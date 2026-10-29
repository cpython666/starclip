//! Coarse product metadata only: no host name, serial, account or hardware ID.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceInfo {
    /// Architecture of this app binary, including when running under translation.
    pub architecture: String,
    pub os_major: String,
    pub model: String,
}

impl DeviceInfo {
    pub fn detect() -> Self {
        static DEVICE: OnceLock<DeviceInfo> = OnceLock::new();
        DEVICE
            .get_or_init(|| {
                let (os_major, model) = platform_info();
                Self {
                    architecture: architecture(std::env::consts::ARCH).into(),
                    os_major,
                    model,
                }
            })
            .clone()
    }
}

fn architecture(value: &str) -> &'static str {
    match value {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        "x86" => "x86",
        "arm" => "arm",
        _ => "unknown",
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", test))]
fn os_major(value: &str) -> String {
    let major = value.trim().split('.').next().unwrap_or_default();
    if major.len() <= 2
        && !major.starts_with('0')
        && !major.is_empty()
        && major.bytes().all(|b| b.is_ascii_digit())
    {
        major.to_owned()
    } else {
        "unknown".into()
    }
}

#[cfg(any(target_os = "macos", test))]
fn mac_model(value: &str) -> String {
    // Public Apple product identifiers have two short numeric components.
    // Do not let malformed firmware strings become arbitrary telemetry text.
    let value = value.trim();
    let accepted = [
        "MacBookPro",
        "MacBookAir",
        "MacBook",
        "Macmini",
        "MacPro",
        "iMacPro",
        "iMac",
        "Mac",
    ]
    .iter()
    .any(|prefix| {
        let Some(suffix) = value.strip_prefix(prefix) else {
            return false;
        };
        let Some((generation, variant)) = suffix.split_once(',') else {
            return false;
        };
        [generation, variant]
            .iter()
            .all(|part| (1..=2).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit()))
    });
    if accepted {
        value.into()
    } else {
        "Mac".into()
    }
}

#[cfg(any(target_os = "windows", test))]
fn windows_model(value: &str) -> String {
    const SERIES: [&str; 18] = [
        "Surface",
        "ThinkPad",
        "ThinkCentre",
        "Latitude",
        "OptiPlex",
        "Precision",
        "XPS",
        "Inspiron",
        "Pavilion",
        "EliteBook",
        "ProBook",
        "ZBook",
        "Vivobook",
        "Zenbook",
        "ROG",
        "Swift",
        "Aspire",
        "WindowsPC",
    ];
    SERIES
        .iter()
        .find(|series| {
            value
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| word.eq_ignore_ascii_case(series))
        })
        .copied()
        .unwrap_or("WindowsPC")
        .into()
}

#[cfg(any(target_os = "windows", test))]
fn windows_major(build: &str) -> String {
    match build.trim().parse::<u32>() {
        Ok(22_000..) => "11",
        Ok(10_240..=21_999) => "10",
        _ => "unknown",
    }
    .into()
}

#[cfg(target_os = "macos")]
fn platform_info() -> (String, String) {
    use std::ffi::{c_char, c_int, c_void, CStr};
    unsafe extern "C" {
        fn sysctlbyname(
            name: *const c_char,
            oldp: *mut c_void,
            oldlenp: *mut usize,
            newp: *mut c_void,
            newlen: usize,
        ) -> c_int;
    }
    fn read(name: &CStr) -> Option<String> {
        let mut bytes = [0_u8; 128];
        let mut size = bytes.len();
        // Fixed NUL-terminated names and bounded writable buffer; newp=NULL means read only.
        let status = unsafe {
            sysctlbyname(
                name.as_ptr(),
                bytes.as_mut_ptr().cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if status != 0 || size > bytes.len() || size == 0 {
            return None;
        }
        let value = CStr::from_bytes_with_nul(&bytes[..size]).ok()?;
        value.to_str().ok().map(str::to_owned)
    }
    (
        os_major(&read(c"kern.osproductversion").unwrap_or_default()),
        mac_model(&read(c"hw.model").unwrap_or_default()),
    )
}

#[cfg(target_os = "windows")]
fn platform_info() -> (String, String) {
    use std::ffi::c_void;
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            key: *mut c_void,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut c_void,
            size: *mut u32,
        ) -> i32;
    }
    fn read(subkey: &str, value: &str) -> Option<String> {
        let subkey: Vec<_> = subkey.encode_utf16().chain(Some(0)).collect();
        let value: Vec<_> = value.encode_utf16().chain(Some(0)).collect();
        let mut buffer = [0_u16; 128];
        let mut size = std::mem::size_of_val(&buffer) as u32;
        // HKLM + REG_SZ only, 64-bit registry view. Never inspect user or serial keys.
        let status = unsafe {
            RegGetValueW(
                (-2_147_483_646_isize) as *mut c_void,
                subkey.as_ptr(),
                value.as_ptr(),
                0x0000_0002 | 0x0001_0000,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != 0
            || size < 2
            || size as usize > std::mem::size_of_val(&buffer)
            || size % 2 != 0
        {
            return None;
        }
        let end = size as usize / 2 - 1;
        if buffer[end] != 0 || buffer[..end].contains(&0) {
            return None;
        }
        String::from_utf16(&buffer[..end]).ok()
    }
    (
        windows_major(
            &read(
                "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
                "CurrentBuildNumber",
            )
            .unwrap_or_default(),
        ),
        windows_model(
            &read("HARDWARE\\DESCRIPTION\\System\\BIOS", "SystemProductName").unwrap_or_default(),
        ),
    )
}

#[cfg(target_os = "linux")]
fn platform_info() -> (String, String) {
    use std::io::Read;
    let mut release = String::new();
    let read = std::fs::File::open("/proc/sys/kernel/osrelease")
        .and_then(|file| file.take(128).read_to_string(&mut release));
    (
        if read.is_ok() {
            os_major(&release)
        } else {
            "unknown".into()
        },
        "LinuxPC".into(),
    )
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_info() -> (String, String) {
    ("unknown".into(), "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_coarse_allowlisted_values_survive_normalization() {
        for name in ["Mac14,2", "MacBookPro18,3", "MacBookAir10,1", "iMac19,2"] {
            assert_eq!(mac_model(name), name);
        }
        for name in [
            "private-host",
            "Mac14,2 serial123",
            "Mac123,4",
            "Mac1,2,3",
            "Mac1,x",
        ] {
            assert_eq!(mac_model(name), "Mac");
        }
        assert_eq!(windows_model("HP EliteBook 840 G9"), "EliteBook");
        assert_eq!(windows_model("Microsoft Surface Laptop 6"), "Surface");
        assert_eq!(windows_model("Lenovo THINKPAD X1"), "ThinkPad");
        assert_eq!(windows_model("custom-private-model-SN-123456"), "WindowsPC");
        assert_eq!(windows_model("mysurfaceaccount"), "WindowsPC");
        assert_eq!(os_major("26.0.1"), "26");
        assert_eq!(os_major("6.8.0-31-generic\n"), "6");
        for value in ["", "26-private", "001.2", "100.0", "private-host", "0.1"] {
            assert_eq!(os_major(value), "unknown");
        }
        assert_eq!(windows_major("22631"), "11");
        assert_eq!(windows_major("19045"), "10");
        assert_eq!(windows_major("9600"), "unknown");
        assert_eq!(windows_major("private-host"), "unknown");
        assert_eq!(architecture("aarch64"), "aarch64");
        assert_eq!(architecture("private-model"), "unknown");
    }

    #[test]
    fn serialization_has_no_room_for_identity_or_arbitrary_extra_fields() {
        let device = DeviceInfo {
            architecture: "aarch64".into(),
            os_major: "26".into(),
            model: "Mac14,2".into(),
        };
        let mut value = serde_json::to_value(&device).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"architecture":"aarch64", "osMajor":"26", "model":"Mac14,2"})
        );
        value["serial"] = "fixed-forbidden-fixture".into();
        assert!(serde_json::from_value::<DeviceInfo>(value).is_err());
    }
}
