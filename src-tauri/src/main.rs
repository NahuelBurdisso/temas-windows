#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .plugin(tauri_plugin_dialog::init())
        .build(tauri::generate_context!())
        .expect("error al iniciar la app")
        .run(|_, evento| {
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = evento {
                api.prevent_exit();
            }
        });
}
