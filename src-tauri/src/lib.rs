pub mod keys;

/// The product name shown in the window title and the UI.
pub const APP_NAME: &str = "Mechanical Key Sounds";

/// Builds the Tauri app and runs it until the last window is closed.
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
