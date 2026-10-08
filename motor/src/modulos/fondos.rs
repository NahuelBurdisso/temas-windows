//! Fondos de escritorio rotativos.

use super::Modulo;
use crate::registro::{self, RegValor};
use crate::rotador::Rotador;
use crate::tema::{listar_imagenes, Orden, Tema};
use crate::win;
use anyhow::{bail, Context, Result};
use rand::seq::SliceRandom;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const DESKTOP: &str = r"Control Panel\Desktop";

pub fn ordenar(mut lista: Vec<PathBuf>, orden: Orden) -> Vec<PathBuf> {
    if orden == Orden::Aleatorio {
        lista.shuffle(&mut rand::thread_rng());
    }
    lista
}

/// Imágenes (ya ordenadas) e intervalo de rotación del tema.
pub fn lista_de(tema: &Tema) -> Result<(Vec<PathBuf>, Duration)> {
    let conf = tema.conf.fondos.as_ref().context("el tema no define fondos")?;
    let lista = ordenar(listar_imagenes(&tema.dir.join("fondos"))?, conf.orden);
    if lista.is_empty() {
        bail!(r"no hay imágenes en fondos\");
    }
    Ok((lista, Duration::from_secs(conf.intervalo_seg)))
}

fn escribir_estilo(estilo: &RegValor) -> Result<()> {
    registro::escribir(DESKTOP, "WallpaperStyle", estilo)?;
    registro::escribir(DESKTOP, "TileWallpaper", &RegValor::Sz("0".into()))
}

/// Copia que Windows guarda del fondo actual; sirve si el archivo original ya no existe.
fn transcoded() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
        .join(r"Microsoft\Windows\Themes\TranscodedWallpaper")
}

pub struct Fondos {
    pub rotador: Arc<Rotador>,
    /// `D:\TemasApp`: ahí va `fondo-actual.<ext>` al restaurar.
    pub raiz: PathBuf,
}

impl Modulo for Fondos {
    fn id(&self) -> &'static str {
        "fondos"
    }

    fn capturar(&self, dir: &Path) -> Result<Value> {
        let actual = win::fondo_actual()?;
        let (fuente, ext) = if actual.is_file() {
            let ext = actual
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg")
                .to_ascii_lowercase();
            (actual, ext)
        } else {
            (transcoded(), "jpg".to_string())
        };
        let archivo = format!("fondo.{ext}");
        std::fs::copy(&fuente, dir.join(&archivo))
            .with_context(|| format!("copiar el fondo actual ({})", fuente.display()))?;
        Ok(json!({ "archivo": archivo, "estilo": registro::leer(DESKTOP, "WallpaperStyle")? }))
    }

    fn aplicar(&self, tema: &Tema) -> Result<()> {
        let conf = tema.conf.fondos.as_ref().context("el tema no define fondos")?;
        let (lista, intervalo) = lista_de(tema)?;
        escribir_estilo(&RegValor::Sz(conf.estilo.wallpaper_style().into()))?;
        self.rotador.iniciar(lista, intervalo);
        Ok(())
    }

    fn restaurar(&self, captura: &Value, dir: &Path) -> Result<()> {
        self.rotador.detener();
        let archivo = captura["archivo"].as_str().context("captura de fondos sin archivo")?;
        let ext = Path::new(archivo).extension().and_then(|e| e.to_str()).unwrap_or("jpg");
        // Copia a una ruta estable: `dir` puede moverse a original.prev después.
        let destino = self.raiz.join(format!("fondo-actual.{ext}"));
        std::fs::copy(dir.join(archivo), &destino)
            .with_context(|| format!("copiar {archivo} a {}", destino.display()))?;
        escribir_estilo(&serde_json::from_value(captura["estilo"].clone())?)?;
        win::poner_fondo(&destino)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ordenar_alfabetico_conserva_y_aleatorio_mezcla_los_mismos() {
        let l: Vec<PathBuf> = (0..20).map(|i| PathBuf::from(format!("{i:02}.jpg"))).collect();
        assert_eq!(ordenar(l.clone(), Orden::Alfabetico), l);
        let mut m = ordenar(l.clone(), Orden::Aleatorio);
        m.sort();
        assert_eq!(m, l);
    }

    #[test]
    fn lista_de_toma_las_imagenes_y_el_intervalo() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("sw");
        std::fs::create_dir_all(dir.join("fondos")).unwrap();
        for f in ["b.jpg", "a.png"] {
            std::fs::write(dir.join("fondos").join(f), b"x").unwrap();
        }
        let conf = json!({"nombre": "SW", "fondos": {"intervaloSeg": 30, "orden": "alfabetico"}});
        std::fs::write(dir.join("tema.json"), conf.to_string()).unwrap();
        let tema = Tema::cargar(&dir).unwrap();
        let (lista, intervalo) = lista_de(&tema).unwrap();
        assert_eq!(intervalo, Duration::from_secs(30));
        assert_eq!(lista, [dir.join("fondos").join("a.png"), dir.join("fondos").join("b.jpg")]);
    }
}
