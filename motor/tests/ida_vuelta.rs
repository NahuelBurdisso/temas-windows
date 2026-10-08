//! Pruebas contra Windows real: cambian la configuración y la restauran.
//! Correr SÓLO con permiso del usuario:
//!   cargo test -p motor --test ida_vuelta -- --ignored --test-threads=1

use motor::estado::{Original, Rutas};
use motor::modulos::bloqueo::{self, Bloqueo};
use motor::modulos::colores::{self, Colores};
use motor::modulos::fondos::Fondos;
use motor::modulos::Modulo;
use motor::nucleo::Motor;
use motor::registro::{self, RegValor};
use motor::rotador::Rotador;
use motor::tema::Tema;
use motor::win;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const DESKTOP: &str = r"Control Panel\Desktop";

fn tema_sw() -> Tema {
    Tema::cargar(Path::new(r"D:\Temas\star-wars")).expect("falta el tema Star Wars (Tarea 13)")
}

/// Estado aislado para pruebas, que persiste (el fondo restaurado queda apuntando ahí).
fn rutas_prueba() -> Rutas {
    let r = Rutas::nuevo(r"D:\TemasApp\pruebas");
    let _ = std::fs::remove_file(r.original());
    let _ = std::fs::remove_file(r.original_prev());
    let _ = std::fs::remove_file(r.estado());
    let _ = std::fs::remove_dir_all(r.raiz.join("original"));
    let _ = std::fs::remove_dir_all(r.raiz.join("original.prev"));
    std::fs::create_dir_all(&r.raiz).unwrap();
    r
}

#[test]
#[ignore]
fn colores_ida_y_vuelta() {
    let antes = registro::capturar_valores(&colores::CLAVES).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let m = Colores;
    let captura = m.capturar(dir.path()).unwrap();
    m.aplicar(&tema_sw()).unwrap();
    assert_eq!(
        registro::leer(colores::DWM, "AccentColor").unwrap(),
        RegValor::Dword(motor::color::Rgb::desde_hex("#FFE81F").unwrap().abgr())
    );
    std::thread::sleep(Duration::from_secs(2));
    m.restaurar(&captura, dir.path()).unwrap();
    assert_eq!(registro::capturar_valores(&colores::CLAVES).unwrap(), antes);
}

#[test]
#[ignore]
fn bloqueo_dos_ciclos_con_motor() {
    let tema = tema_sw();
    if !tema.define("bloqueo") {
        eprintln!("el tema no define bloqueo (spike fallido): se saltea");
        return;
    }
    let rutas = rutas_prueba();
    let motor = Motor::nuevo(rutas.clone(), vec![Box::new(Bloqueo)]);
    let antes = registro::capturar_valores(&bloqueo::CLAVES).unwrap();
    for ciclo in 1..=2 {
        let inf = motor.aplicar(&tema, &[]).unwrap();
        assert!(inf.errores.is_empty(), "ciclo {ciclo}: {inf:?}");
        assert_eq!(registro::leer(bloqueo::CDM, "RotatingLockScreenEnabled").unwrap(), RegValor::Dword(0));
        // Imagen que quedó capturada (None si Spotlight estaba activo y no había imagen propia).
        let archivo = Original::cargar(&rutas)
            .unwrap()
            .get("bloqueo")
            .and_then(|c| c["archivo"].as_str().map(String::from));

        let inf = motor.restaurar_original().unwrap();
        assert!(inf.errores.is_empty(), "ciclo {ciclo}: {inf:?}");
        assert!(inf.advertencias.is_empty(), "ciclo {ciclo}: {inf:?}");
        assert_eq!(registro::capturar_valores(&bloqueo::CLAVES).unwrap(), antes, "ciclo {ciclo}");
        // La captura quedó archivada en original.prev con su imagen (se compara existencia, no bytes).
        let prev = rutas.dir_original_prev("bloqueo");
        match archivo {
            Some(nombre) => {
                assert!(nombre.starts_with("bloqueo."), "ciclo {ciclo}: nombre inesperado {nombre}");
                assert!(prev.join(&nombre).is_file(), "ciclo {ciclo}: falta la imagen en {}", prev.display());
            }
            None => eprintln!("ciclo {ciclo}: Spotlight activo sin imagen propia; no hay imagen que archivar"),
        }
    }
}

#[test]
#[ignore]
fn fondos_dos_ciclos_con_motor() {
    let rutas = rutas_prueba();
    let rotador = Arc::new(Rotador::nuevo(win::poner_fondo));
    let fondos = Fondos { rotador: rotador.clone(), raiz: rutas.raiz.clone() };
    let motor = Motor::nuevo(rutas.clone(), vec![Box::new(fondos)]);
    let tema = tema_sw();
    let estilo_antes = registro::leer(DESKTOP, "WallpaperStyle").unwrap();
    let tile_antes = registro::leer(DESKTOP, "TileWallpaper").unwrap();

    for ciclo in 1..=2 {
        let inf = motor.aplicar(&tema, &[]).unwrap();
        assert!(inf.errores.is_empty(), "ciclo {ciclo}: {inf:?}");
        std::thread::sleep(Duration::from_secs(1));
        let actual = win::fondo_actual().unwrap();
        assert!(actual.starts_with(r"D:\Temas\star-wars\fondos"), "ciclo {ciclo}: {}", actual.display());

        let inf = motor.restaurar_original().unwrap();
        assert!(inf.errores.is_empty(), "ciclo {ciclo}: {inf:?}");
        assert!(inf.advertencias.is_empty(), "ciclo {ciclo}: {inf:?}");
        assert!(!rotador.activo());
        let actual = win::fondo_actual().unwrap();
        assert!(actual.starts_with(&rutas.raiz), "ciclo {ciclo}: {}", actual.display());
        assert!(actual.is_file(), "ciclo {ciclo}: el fondo restaurado existe");
        assert_eq!(registro::leer(DESKTOP, "WallpaperStyle").unwrap(), estilo_antes, "ciclo {ciclo}");
        assert_eq!(registro::leer(DESKTOP, "TileWallpaper").unwrap(), tile_antes, "ciclo {ciclo}");
    }
}
