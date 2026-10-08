//! Prueba manual: ¿la API de pantalla de bloqueo anda sin admin y desde una app sin empaquetar?
//! Uso: cargo run -p motor --example spike_bloqueo -- "C:\Windows\Web\Wallpaper\Windows\img0.jpg"

use motor::modulos::{bloqueo::Bloqueo, Modulo};
use motor::tema::Tema;
use std::io::stdin;

fn main() -> anyhow::Result<()> {
    let imagen = std::env::args().nth(1).expect("pasá la ruta de una imagen");
    let tmp = tempfile::tempdir()?;
    let dir = tmp.path().join("spike");
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(&imagen, dir.join("bloqueo.jpg"))?;
    std::fs::write(dir.join("tema.json"), r#"{"nombre":"Spike","bloqueo":"bloqueo.jpg"}"#)?;
    let tema = Tema::cargar(&dir)?;
    // Carpeta aparte para la captura: si fuera `dir`, pisaría el bloqueo.jpg del tema.
    let dir_cap = tmp.path().join("original");
    std::fs::create_dir_all(&dir_cap)?;

    let m = Bloqueo;
    let captura = m.capturar(&dir_cap)?;
    println!("Captura: {captura}");
    let r = m.aplicar(&tema);
    let mut espera = Ok(());
    if r.is_ok() {
        println!("Aplicado. Apretá Win+L para ver la pantalla de bloqueo, volvé, y Enter para restaurar.");
        espera = stdin().read_line(&mut String::new()).map(|_| ());
    }
    m.restaurar(&captura, &dir_cap)?;
    r?;
    espera?;
    println!("Restaurado. Win+L de nuevo para confirmar.");
    Ok(())
}
