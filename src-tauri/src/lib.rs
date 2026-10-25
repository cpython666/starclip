pub mod capture;
#[cfg(feature = "desktop")]
mod desktop;
pub mod handwriting;
pub mod images;
pub mod storage;
pub mod store;
#[cfg(feature = "desktop")]
pub use desktop::run;

#[cfg(test)]
mod capability_tests {
    #[cfg(feature = "desktop")]
    #[test]
    fn generated_runtime_acl_enforces_window_boundaries() {
        // Use the build script's actual manifests and generate_context!'s resolver,
        // without creating an application/window or touching its store/clipboard.
        let manifests = serde_json::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/acl-manifests.json"
        )))
        .unwrap();
        let capabilities =
            serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/capabilities.json")))
                .unwrap();
        let config = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let capabilities =
            tauri::utils::acl::get_capabilities(&config, capabilities, None).unwrap();
        let resolved = tauri::utils::acl::resolved::Resolved::resolve(
            &manifests,
            capabilities,
            tauri::utils::platform::Target::current(),
        )
        .unwrap();
        assert!(
            resolved.has_app_acl,
            "custom commands must go through the runtime ACL gate"
        );
        let authority = tauri::runtime_authority!(manifests, resolved);
        let permissions: serde_json::Value =
            serde_json::from_str(include_str!("../permissions/windows.json")).unwrap();
        let main_commands = permissions["permission"]
            .as_array()
            .unwrap()
            .iter()
            .find(|permission| permission["identifier"] == "main-commands")
            .unwrap();
        assert!(main_commands["commands"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command == "open_project_link"));
        for permission in permissions["permission"].as_array().unwrap() {
            let pet_allowed = permission["identifier"] == "starpet-commands";
            for command in permission["commands"]["allow"].as_array().unwrap() {
                let command = command.as_str().unwrap();
                assert!(
                    authority
                        .resolve_access(command, "main", "main", &tauri::ipc::Origin::Local)
                        .is_some(),
                    "main must retain {command}"
                );
                assert_eq!(
                    authority
                        .resolve_access(command, "starpet", "starpet", &tauri::ipc::Origin::Local)
                        .is_some(),
                    pet_allowed,
                    "incorrect starpet access to {command}"
                );
            }
        }
    }

    #[test]
    fn starpet_is_not_granted_history_or_general_window_commands() {
        let pet: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/starpet.json")).unwrap();
        assert_eq!(pet["windows"], serde_json::json!(["starpet"]));
        assert_eq!(
            pet["permissions"],
            serde_json::json!([
                "core:event:allow-listen",
                "core:event:allow-unlisten",
                "starpet-commands"
            ])
        );
        let main: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert_eq!(main["windows"], serde_json::json!(["main"]));
        let permissions: serde_json::Value =
            serde_json::from_str(include_str!("../permissions/windows.json")).unwrap();
        let pet = permissions["permission"]
            .as_array()
            .unwrap()
            .iter()
            .find(|permission| permission["identifier"] == "starpet-commands")
            .unwrap();
        assert_eq!(
            pet["commands"]["allow"],
            serde_json::json!([
                "get_starpet_preferences",
                "set_starpet_preferences",
                "show_main_window",
                "starpet_start_dragging"
            ])
        );
        assert!(
            permissions.get("default").is_none(),
            "no globally granted custom commands"
        );
    }
}
