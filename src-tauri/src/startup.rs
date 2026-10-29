//! Login startup is read from the OS, never restored from a cached preference.

use tauri::{AppHandle, Manager, Runtime};
#[cfg(not(windows))]
use tauri_plugin_autostart::ManagerExt;

pub fn init<R: Runtime>(identifier: &str) -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::Builder::new()
        .app_name(identifier)
        .build()
}

pub fn read_enabled<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    #[cfg(windows)]
    return windows::read_enabled(&app.config().identifier);

    #[cfg(target_os = "macos")]
    return macos::read_enabled(app);

    #[cfg(not(any(windows, target_os = "macos")))]
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

pub fn set_enabled<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<bool, String> {
    #[cfg(windows)]
    windows::set_enabled(&app.config().identifier, enabled)?;

    #[cfg(not(windows))]
    if enabled {
        #[cfg(target_os = "macos")]
        macos::validate_launch_path()?;
        app.autolaunch().enable().map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        macos::enable_service(&app.config().identifier)?;
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())?;
    }
    let actual = read_enabled(app)?;
    if actual != enabled {
        return Err("登录启动设置未生效，请检查系统的登录项或启动应用设置。".into());
    }
    Ok(actual)
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::{path::Path, process::Command};

    pub(super) fn validate_launch_path() -> Result<(), String> {
        // auto-launch 0.6 interpolates XML without escaping. Fail before writing
        // an invalid agent; ordinary app paths (including spaces) are supported.
        let path = std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .map_err(|e| e.to_string())?;
        if path.to_string_lossy().contains(['&', '<', '>']) {
            return Err("请先把星笺移至路径不含 &、<、> 的目录，再开启登录自启动。".into());
        }
        Ok(())
    }

    pub(super) fn read_enabled<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
        if !app.autolaunch().is_enabled().map_err(|e| e.to_string())? {
            return Ok(false);
        }
        let identifier = &app.config().identifier;
        let file = app
            .path()
            .home_dir()
            .map_err(|e| e.to_string())?
            .join("Library/LaunchAgents")
            .join(format!("{identifier}.plist"));
        let plist = plist::Value::from_file(file).map_err(|e| e.to_string())?;
        let executable = std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .map_err(|e| e.to_string())?;
        if !agent_matches(&plist, identifier, &executable) {
            return Ok(false);
        }
        let output = Command::new("/bin/launchctl")
            .args(["print-disabled", &login_domain()?])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("无法读取系统的登录启动状态，请稍后重试。".into());
        }
        Ok(!service_disabled(
            &String::from_utf8_lossy(&output.stdout),
            identifier,
        )?)
    }

    pub(super) fn enable_service(identifier: &str) -> Result<(), String> {
        let output = Command::new("/bin/launchctl")
            .args(["enable", &format!("{}/{identifier}", login_domain()?)])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err("系统未允许登录启动，请检查系统设置中的登录项。".into())
        }
    }

    fn login_domain() -> Result<String, String> {
        let output = Command::new("/usr/bin/id")
            .arg("-u")
            .output()
            .map_err(|e| e.to_string())?;
        let uid = String::from_utf8_lossy(&output.stdout);
        let uid = uid.trim();
        if output.status.success() && !uid.is_empty() && uid.bytes().all(|b| b.is_ascii_digit()) {
            Ok(format!("gui/{uid}"))
        } else {
            Err("无法识别当前登录用户。".into())
        }
    }

    fn agent_matches(plist: &plist::Value, identifier: &str, executable: &Path) -> bool {
        let Some(values) = plist.as_dictionary() else {
            return false;
        };
        let target = values
            .get("ProgramArguments")
            .and_then(plist::Value::as_array)
            .and_then(|args| args.first())
            .and_then(plist::Value::as_string);
        values.get("Label").and_then(plist::Value::as_string) == Some(identifier)
            && values.get("Disabled").and_then(plist::Value::as_boolean) != Some(true)
            && target.is_some_and(|target| {
                Path::new(target) == executable
                    || Path::new(target).canonicalize().ok().as_deref() == Some(executable)
            })
    }

    fn service_disabled(output: &str, identifier: &str) -> Result<bool, String> {
        // launchctl output is not a stable API. Unknown formats are errors, not
        // a guessed enabled state; macOS background-item approval is separate.
        let output = output.trim();
        if !output.starts_with("disabled services = {") || !output.ends_with('}') {
            return Err("无法识别系统的登录启动状态。".into());
        }
        let name = format!("\"{identifier}\"");
        for line in output.lines() {
            if let Some((key, value)) = line.trim().split_once("=>") {
                if key.trim() == name {
                    return match value.trim().trim_end_matches(',') {
                        "true" | "disabled" => Ok(true),
                        "false" | "enabled" => Ok(false),
                        _ => Err("无法识别系统的登录启动状态。".into()),
                    };
                }
            }
        }
        Ok(false)
    }

    #[test]
    fn agent_identity_path_and_system_disable_are_checked() {
        let executable = Path::new("/Applications/Starclip Test.app/Contents/MacOS/starclip");
        let mut values = plist::Dictionary::new();
        values.insert("Label".into(), "com.starclip.test".into());
        values.insert(
            "ProgramArguments".into(),
            plist::Value::Array(vec![executable.to_str().unwrap().into()]),
        );
        let mut plist = plist::Value::Dictionary(values);
        assert!(agent_matches(&plist, "com.starclip.test", executable));
        assert!(!agent_matches(
            &plist,
            "com.starclip.test",
            Path::new("/moved/starclip")
        ));
        assert!(!agent_matches(&plist, "com.starclip.desktop", executable));
        plist
            .as_dictionary_mut()
            .unwrap()
            .insert("Disabled".into(), true.into());
        assert!(!agent_matches(&plist, "com.starclip.test", executable));
        assert!(service_disabled(
            "disabled services = {\n\"com.starclip.test\" => true\n}",
            "com.starclip.test"
        )
        .unwrap());
        assert!(!service_disabled("disabled services = {\n\"com.starclip.test.other\" => true\n\"com.starclip.test\" => false\n}", "com.starclip.test").unwrap());
        assert!(!service_disabled("disabled services = {\n}", "com.starclip.test").unwrap());
        assert!(service_disabled("unknown output", "com.starclip.test").is_err());
        assert!(!service_disabled(
            "disabled services = {\n\"com.starclip.test\" => enabled\n}",
            "com.starclip.test"
        )
        .unwrap());
        assert!(service_disabled(
            "disabled services = {\n\"com.starclip.test\" => disabled\n}",
            "com.starclip.test"
        )
        .unwrap());
    }
}

#[cfg(any(windows, test))]
fn windows_command(executable: &str) -> String {
    format!("\"{executable}\"")
}

#[cfg(any(windows, test))]
fn windows_startup_allowed(value: &[u8]) -> Result<bool, String> {
    match value.get(..4) {
        Some([2 | 6, 0, 0, 0]) => Ok(true),
        Some([3 | 7, 0, 0, 0]) => Ok(false),
        _ => Err("无法识别系统的登录启动状态，请检查任务管理器的启动应用。".into()),
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use windows_registry::{Type, CURRENT_USER};

    const RUN: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED: &str =
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    // Windows' official plugin currently writes an unquoted executable and may
    // choose HKLM when elevated. Keep this one setting strictly current-user.
    pub(super) fn set_enabled(identifier: &str, enabled: bool) -> Result<(), String> {
        if enabled {
            CURRENT_USER
                .create(RUN)
                .and_then(|key| key.set_string(identifier, current_command()?))
                .map_err(|e| e.to_string())?;
            CURRENT_USER
                .create(APPROVED)
                .and_then(|key| {
                    key.set_bytes(
                        identifier,
                        Type::Bytes,
                        &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    )
                })
                .map_err(|e| e.to_string())?;
        } else {
            optional(
                CURRENT_USER
                    .options()
                    .write()
                    .open(RUN)
                    .and_then(|key| key.remove_value(identifier)),
            )?;
        }
        Ok(())
    }

    pub(super) fn read_enabled(identifier: &str) -> Result<bool, String> {
        let command = optional(
            CURRENT_USER
                .open(RUN)
                .and_then(|key| key.get_string(identifier)),
        )?;
        let expected = current_command().map_err(|e| e.to_string())?;
        if command.as_deref().map(str::trim) != Some(expected.as_str()) {
            return Ok(false);
        }
        match optional(
            CURRENT_USER
                .open(APPROVED)
                .and_then(|key| key.get_value(identifier)),
        )? {
            None => Ok(true),
            Some(value) if value.ty() == Type::Bytes => windows_startup_allowed(&value),
            Some(_) => Err("系统登录启动状态的格式不正确，请检查任务管理器的启动应用。".into()),
        }
    }

    fn current_command() -> windows_registry::Result<String> {
        Ok(windows_command(&std::env::current_exe()?.to_string_lossy()))
    }

    fn optional<T>(result: windows_registry::Result<T>) -> Result<Option<T>, String> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.code().0 as u32 == 0x80070002 => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[test]
fn windows_paths_and_external_disabled_states_are_preserved() {
    assert_eq!(
        windows_command(r"C:\Program Files\Starclip\starclip.exe"),
        r#""C:\Program Files\Starclip\starclip.exe""#
    );
    assert_ne!(
        windows_command(r"C:\Old\starclip.exe"),
        windows_command(r"C:\New\starclip.exe")
    );
    assert!(windows_startup_allowed(&[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap());
    assert!(!windows_startup_allowed(&[3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]).unwrap());
    assert!(windows_startup_allowed(&[6, 0, 0, 0]).unwrap());
    assert!(!windows_startup_allowed(&[7, 0, 0, 0]).unwrap());
    assert!(windows_startup_allowed(&[0]).is_err());
}
