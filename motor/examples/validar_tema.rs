//! Valida un tema: cargo run -p motor --example validar_tema -- D:\Temas\star-wars
fn main() -> anyhow::Result<()> {
    let dir = std::env::args().nth(1).expect("pasá la carpeta del tema");
    let tema = motor::tema::Tema::cargar(std::path::Path::new(&dir))?;
    println!("OK: {} — advertencias: {:?}", tema.conf.nombre, tema.advertencias);
    Ok(())
}
