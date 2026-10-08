//! Prueba manual: ¿el acento cambia en vivo en barra, Inicio y títulos sin cerrar sesión?
//! Uso: cargo run -p motor --example spike_colores -- "#C9A227"

use motor::modulos::{colores::Colores, Modulo};
use motor::tema::Tema;
use std::io::stdin;

fn main() -> anyhow::Result<()> {
    let acento = std::env::args().nth(1).unwrap_or_else(|| "#C9A227".into());
    let tmp = tempfile::tempdir()?;
    let dir = tmp.path().join("spike");
    std::fs::create_dir_all(&dir)?;
    let conf = serde_json::json!({
        "nombre": "Spike",
        "colores": {"acento": acento, "modo": "oscuro", "acentoEnBarra": true}
    });
    std::fs::write(dir.join("tema.json"), conf.to_string())?;
    let tema = Tema::cargar(&dir)?;

    let m = Colores;
    let captura = m.capturar(&dir)?;
    println!("Captura: {captura}");
    println!("Estado actual guardado. Aplicando {acento}…");
    let r = m.aplicar(&tema);
    if r.is_ok() {
        println!("Mirá la barra de tareas, el menú Inicio y la barra de título de una ventana.");
        println!("Enter para restaurar.");
        stdin().read_line(&mut String::new())?;
    }
    m.restaurar(&captura, &dir)?;
    r?;
    println!("Restaurado. ¿Quedó todo como antes?");
    Ok(())
}
