/// Builds and runs the Tauri app. No window is created yet.
#[allow(
    clippy::expect_used,
    reason = "a failed Tauri startup is unrecoverable"
)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running the Quark desktop app");
}
