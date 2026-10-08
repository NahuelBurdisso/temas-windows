#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bandeja;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .plugin(tauri_plugin_dialog::init())
        .setup(|a| {
            let (estado, mut avisos) = app::App::nuevo();
            if let Err(e) = estado.retomar() {
                avisos.push(format!("no se pudo retomar la rotación: {e:#}"));
            }
            a.manage(estado);
            bandeja::crear(a.handle())?;
            bandeja::avisar_arranque(a.handle(), &avisos);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al iniciar la app")
        .run(|_, evento| {
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = evento {
                api.prevent_exit();
            }
        });
}
