fn main() {
    #[cfg(feature = "desktop")]
    {
        let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
            && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
        if windows_msvc {
            // tauri-build 2.7.4 links its manifest only into bins. Unit tests
            // also need Common Controls v6 for TaskDialogIndirect (tauri#13419).
            let manifest =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
            println!("cargo:rerun-if-changed=windows-app-manifest.xml");
            println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
            println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
            tauri_build::try_build(
                tauri_build::Attributes::new()
                    .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest()),
            )
            .expect("failed to prepare Tauri Windows resources");
        } else {
            tauri_build::build();
        }
    }
}
