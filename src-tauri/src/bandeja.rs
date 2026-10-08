//! Ícono y menú de la bandeja del sistema.

use crate::app::App;
use motor::nucleo::Informe;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

const ID: &str = "principal";

fn construir_menu(h: &AppHandle, app: &App) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(h)?;
    let activo = app.estado().tema_activo;
    let temas = app.temas();
    if temas.is_empty() {
        menu.append(&MenuItem::with_id(h, "nada", "(no hay temas en la carpeta)", false, None::<&str>)?)?;
    }
    for (id, t) in &temas {
        match t {
            Ok(t) => menu.append(&CheckMenuItem::with_id(
                h,
                format!("tema:{id}"),
                &t.conf.nombre,
                true,
                activo.as_deref() == Some(id.as_str()),
                None::<&str>,
            )?)?,
            Err(_) => menu.append(&MenuItem::with_id(
                h,
                format!("roto:{id}"),
                format!("{id} (tema.json con errores)"),
                true,
                None::<&str>,
            )?)?,
        }
    }
    menu.append(&PredefinedMenuItem::separator(h)?)?;
    let rota = app.rotador.activo();
    let pausa = if app.rotador.pausado() { "Reanudar fondos" } else { "Pausar fondos" };
    menu.append(&MenuItem::with_id(h, "siguiente", "Siguiente fondo", rota, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(h, "pausa", pausa, rota, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(h, "restaurar", "Restaurar original", true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(h)?)?;
    menu.append(&MenuItem::with_id(h, "salir", "Salir", true, None::<&str>)?)?;
    Ok(menu)
}

fn tooltip(app: &App) -> String {
    match app.nombre_activo() {
        Some(n) => format!("Temas — {n}"),
        None => "Temas — original".into(),
    }
}

pub fn crear(h: &AppHandle) -> tauri::Result<()> {
    let app = h.state::<App>();
    let icono = h.default_window_icon().expect("falta el ícono de la app").clone();
    TrayIconBuilder::with_id(ID)
        .icon(icono)
        .tooltip(tooltip(&app))
        .menu(&construir_menu(h, &app)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|h, ev| manejar(h, ev.id().as_ref()))
        .build(h)?;
    Ok(())
}

fn refrescar(h: &AppHandle) {
    let h2 = h.clone();
    let _ = h.run_on_main_thread(move || {
        let app = h2.state::<App>();
        if let (Some(tray), Ok(menu)) = (h2.tray_by_id(ID), construir_menu(&h2, &app)) {
            let _ = tray.set_menu(Some(menu));
            let _ = tray.set_tooltip(Some(tooltip(&app)));
        }
    });
}

fn avisar(h: &AppHandle, titulo: &str, texto: String, tipo: MessageDialogKind) {
    h.dialog().message(texto).title(titulo).kind(tipo).show(|_| {});
}

fn informar(h: &AppHandle, r: anyhow::Result<Informe>) {
    match r {
        Ok(inf) if inf.errores.is_empty() && inf.advertencias.is_empty() => {}
        Ok(inf) => avisar(h, "Temas — hubo problemas", inf.resumen(), MessageDialogKind::Warning),
        Err(e) => avisar(h, "Temas — error", format!("{e:#}"), MessageDialogKind::Error),
    }
}

fn manejar(h: &AppHandle, id: &str) {
    let h = h.clone();
    let id = id.to_string();
    // Aplicar puede tardar (copias, WinRT): fuera del hilo de la interfaz.
    std::thread::spawn(move || {
        let app = h.state::<App>();
        if let Some(t) = id.strip_prefix("tema:") {
            informar(&h, app.aplicar(t));
        } else if let Some(t) = id.strip_prefix("roto:") {
            avisar(&h, "Temas — tema con errores", app.error_de(t), MessageDialogKind::Error);
        } else {
            match id.as_str() {
                "siguiente" => app.rotador.siguiente(),
                "pausa" => app.alternar_pausa(),
                "restaurar" => informar(&h, app.restaurar()),
                "salir" => {
                    h.exit(0);
                    return;
                }
                _ => {}
            }
        }
        refrescar(&h);
    });
}
