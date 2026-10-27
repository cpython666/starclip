#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if let Some(code) = starclip_lib::ai_transport::run_from_env() {
        std::process::exit(code);
    }
    starclip_lib::run();
}
