// Tauri entry. `main.rs` only calls `run()` — all setup happens here.
// Managers will be registered in `setup()` as P0 slices land.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
