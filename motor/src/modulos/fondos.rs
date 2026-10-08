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

fn escribir_estilo(estilo: &RegValor, tile: &RegValor) -> Result<()> {
    registro::escribir(DESKTOP, "WallpaperStyle", estilo)?;
    registro::escribir(DESKTOP, "TileWallpaper", tile)
}

/// `TileWallpaper` a restaurar: lo capturado, o "0" si la captura no lo trae
/// (captura vieja u `original.json` sembrado a mano).
fn tile_de(captura: &Value) -> Result<RegValor> {
    match captura.get("tile") {
        None | Some(Value::Null) => Ok(RegValor::Sz("0".into())),
        Some(v) => serde_json::from_value(v.clone()).context("captura de fondos: tile inválido"),
    }
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

    /// Si el fondo es un color sólido, Windows no tiene ruta de imagen: se cae a
    /// `TranscodedWallpaper`, que puede contener una imagen anterior y no el color actual.
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
        Ok(json!({
            "archivo": archivo,
            "estilo": registro::leer(DESKTOP, "WallpaperStyle")?,
            "tile": registro::leer(DESKTOP, "TileWallpaper")?,
        }))
    }

    fn aplicar(&self, tema: &Tema) -> Result<()> {
        // Antes de cualquier `?`: un aplicar fallido nunca deja rotando el tema anterior.
        self.rotador.detener();
        let conf = tema.conf.fondos.as_ref().context("el tema no define fondos")?;
        let (lista, intervalo) = lista_de(tema)?;
        escribir_estilo(&RegValor::Sz(conf.estilo.wallpaper_style().into()), &RegValor::Sz("0".into()))?;
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
        escribir_estilo(&serde_json::from_value(captura["estilo"].clone())?, &tile_de(captura)?)?;
        win::poner_fondo(&destino)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn aplicar_fallido_no_deja_rotando_el_tema_anterior() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("sin-fondos");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("tema.json"), json!({"nombre": "Sin fondos"}).to_string()).unwrap();
        let tema = Tema::cargar(&dir).unwrap();

        let rotador = Arc::new(Rotador::nuevo(|_: &Path| Ok(())));
        rotador.iniciar(vec![PathBuf::from("a.jpg")], Duration::from_secs(60));
        assert!(rotador.activo());
        let fondos = Fondos { rotador: rotador.clone(), raiz: t.path().into() };

        assert!(fondos.aplicar(&tema).is_err());
        assert!(!rotador.activo(), "un aplicar fallido no debe dejar rotando el tema anterior");
    }

    #[test]
    fn ordenar_alfabetico_conserva_y_aleatorio_mezcla_los_mismos() {
        let l: Vec<PathBuf> = (0..20).map(|i| PathBuf::from(format!("{i:02}.jpg"))).collect();
        assert_eq!(ordenar(l.clone(), Orden::Alfabetico), l);
        let mut m = ordenar(l.clone(), Orden::Aleatorio);
        m.sort();
        assert_eq!(m, l);
    }

    #[test]
    fn tile_de_usa_lo_capturado_o_cero_si_falta() {
        assert_eq!(tile_de(&json!({"archivo": "fondo.jpg"})).unwrap(), RegValor::Sz("0".into()));
        assert_eq!(tile_de(&json!({"tile": null})).unwrap(), RegValor::Sz("0".into()));
        let capturado = serde_json::to_value(RegValor::Sz("1".into())).unwrap();
        assert_eq!(tile_de(&json!({ "tile": capturado })).unwrap(), RegValor::Sz("1".into()));
        let ausente = serde_json::to_value(RegValor::Ausente).unwrap();
        assert_eq!(tile_de(&json!({ "tile": ausente })).unwrap(), RegValor::Ausente);
        assert!(tile_de(&json!({"tile": 5})).is_err());
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
