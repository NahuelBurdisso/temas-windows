//! Ícono y menú de la bandeja del sistema.

use crate::app::App;
use motor::nucleo::Informe;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

const ID: &str = "principal";

/// Todo lo que el menú necesita, ya leído del disco. Se arma fuera del hilo de la interfaz.
struct DatosMenu {
    /// (id, etiqueta, tema válido)
    temas: Vec<(String, String, bool)>,
    activo: Option<String>,
    rota: bool,
    pausado: bool,
    tooltip: String,
}

fn datos_menu(app: &App) -> DatosMenu {
    let activo = app.estado().tema_activo;
    let temas: Vec<(String, String, bool)> = app
        .temas()
        .into_iter()
        .map(|(id, t)| match t {
            Ok(t) => (id, t.conf.nombre, true),
            Err(_) => {
                let etiqueta = format!("{id} (tema.json con errores)");
                (id, etiqueta, false)
            }
        })
        .collect();
    let tooltip = match &activo {
        Some(id) => {
            let nombre = temas.iter().find(|(i, _, ok)| i == id && *ok).map(|(_, n, _)| n.as_str());
            format!("Temas — {}", nombre.unwrap_or(id))
        }
        None => "Temas — original".into(),
    };
    DatosMenu { temas, activo, rota: app.rotador.activo(), pausado: app.rotador.pausado(), tooltip }
}

fn construir_menu(h: &AppHandle, d: &DatosMenu) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(h)?;
    if d.temas.is_empty() {
        menu.append(&MenuItem::with_id(h, "nada", "(no hay temas en la carpeta)", false, None::<&str>)?)?;
    }
    for (id, etiqueta, ok) in &d.temas {
        if *ok {
            menu.append(&CheckMenuItem::with_id(
                h,
                format!("tema:{id}"),
                etiqueta,
                true,
                d.activo.as_deref() == Some(id.as_str()),
                None::<&str>,
            )?)?;
        } else {
            menu.append(&MenuItem::with_id(h, format!("roto:{id}"), etiqueta, true, None::<&str>)?)?;
        }
    }
    menu.append(&PredefinedMenuItem::separator(h)?)?;
    let pausa = if d.pausado { "Reanudar fondos" } else { "Pausar fondos" };
    menu.append(&MenuItem::with_id(h, "siguiente", "Siguiente fondo", d.rota, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(h, "pausa", pausa, d.rota, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(h, "restaurar", "Restaurar original", true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(h)?)?;
    menu.append(&MenuItem::with_id(h, "salir", "Salir", true, None::<&str>)?)?;
    Ok(menu)
}

pub fn crear(h: &AppHandle) -> tauri::Result<()> {
    let app = h.state::<App>();
    let datos = datos_menu(&app);
    let icono = h.default_window_icon().expect("falta el ícono de la app").clone();
    TrayIconBuilder::with_id(ID)
        .icon(icono)
        .tooltip(&datos.tooltip)
        .menu(&construir_menu(h, &datos)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|h, ev| manejar(h, ev.id().as_ref()))
        .build(h)?;
    Ok(())
}

/// Vuelve a leer el estado en el hilo que llama; el hilo de la interfaz sólo arma los widgets.
fn refrescar(h: &AppHandle) {
    let datos = {
        // Con el candado tomado, una lectura vieja no puede publicarse después de una nueva
        // (las acciones también lo toman). Se suelta antes de pasar a la interfaz.
        let app = h.state::<App>();
        let _g = app.bloquear();
        datos_menu(&app)
    };
    let h2 = h.clone();
    let _ = h.run_on_main_thread(move || {
        if let (Some(tray), Ok(menu)) = (h2.tray_by_id(ID), construir_menu(&h2, &datos)) {
            let _ = tray.set_menu(Some(menu));
            let _ = tray.set_tooltip(Some(&datos.tooltip));
        }
    });
}

fn avisar(h: &AppHandle, titulo: &str, texto: String, tipo: MessageDialogKind) {
    h.dialog().message(texto).title(titulo).kind(tipo).show(|_| {});
}

/// Avisos juntados durante el arranque (ajustes ilegibles, rotación que no se pudo retomar…).
pub fn avisar_arranque(h: &AppHandle, avisos: &[String]) {
    if !avisos.is_empty() {
        avisar(h, "Temas — avisos al arrancar", avisos.join("\n\n"), MessageDialogKind::Warning);
    }
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
                "pausa" => {
                    if let Err(e) = app.alternar_pausa() {
                        avisar(&h, "Temas — error", format!("no se pudo guardar la pausa: {e:#}"), MessageDialogKind::Error);
                    }
                }
                "restaurar" => informar(&h, app.restaurar()),
                "salir" => {
                    // Si hay una aplicación o restauración en curso, se espera a que termine,
                    // y el candado queda tomado para siempre: ninguna acción nueva arranca.
                    std::mem::forget(app.bloquear());
                    // Espera a que termine el cambio de fondo en vuelo (si hay) y frena la rotación.
                    app.rotador.pausar();
                    h.exit(0);
                    return;
                }
                _ => {}
            }
        }
        refrescar(&h);
    });
}
