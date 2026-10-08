//! Temas: lectura y validación de `tema.json`.

use crate::color::Rgb;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const EXTENSIONES_IMAGEN: [&str; 4] = ["jpg", "jpeg", "png", "bmp"];

pub const NOMBRES_ANSI: [&str; 16] = [
    "black", "red", "green", "yellow", "blue", "purple", "cyan", "white",
    "brightBlack", "brightRed", "brightGreen", "brightYellow",
    "brightBlue", "brightPurple", "brightCyan", "brightWhite",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modo {
    Oscuro,
    Claro,
}

fn si() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColoresConf {
    pub acento: String,
    pub modo: Modo,
    #[serde(default = "si")]
    pub transparencia: bool,
    #[serde(default)]
    pub acento_en_barra: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orden {
    #[default]
    Aleatorio,
    Alfabetico,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Estilo {
    #[default]
    Rellenar,
    Ajustar,
    Expandir,
    Centrar,
}

impl Estilo {
    /// Valor de `HKCU\Control Panel\Desktop\WallpaperStyle`.
    pub fn wallpaper_style(&self) -> &'static str {
        match self {
            Estilo::Rellenar => "10",
            Estilo::Ajustar => "6",
            Estilo::Expandir => "2",
            Estilo::Centrar => "0",
        }
    }
}

fn veinte() -> u64 {
    20
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FondosConf {
    #[serde(default = "veinte")]
    pub intervalo_seg: u64,
    #[serde(default)]
    pub orden: Orden,
    #[serde(default)]
    pub estilo: Estilo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerminalConf {
    pub fondo: String,
    pub texto: String,
    pub cursor: String,
    #[serde(default)]
    pub ansi: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TemaJson {
    pub nombre: String,
    pub colores: Option<ColoresConf>,
    pub fondos: Option<FondosConf>,
    pub bloqueo: Option<String>,
    pub terminal: Option<TerminalConf>,
    // Secciones de etapas posteriores: se aceptan pero todavía no se validan.
    pub sonidos: Option<Value>,
    pub intro: Option<Value>,
    pub protector: Option<Value>,
    pub iconos: Option<Value>,
    pub widget: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct Tema {
    /// Nombre de la carpeta.
    pub id: String,
    pub dir: PathBuf,
    pub conf: TemaJson,
    pub advertencias: Vec<String>,
}

impl Tema {
    pub fn cargar(dir: &Path) -> Result<Tema> {
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .context("carpeta de tema sin nombre")?
            .to_string();
        let ruta = dir.join("tema.json");
        let texto = std::fs::read_to_string(&ruta)
            .with_context(|| format!("leer {}", ruta.display()))?;
        let conf: TemaJson = serde_json::from_str(texto.trim_start_matches('\u{feff}'))
            .context("tema.json mal formado")?;
        let mut tema = Tema { id, dir: dir.to_path_buf(), conf, advertencias: vec![] };
        tema.validar()?;
        Ok(tema)
    }

    fn validar(&mut self) -> Result<()> {
        if self.conf.nombre.trim().is_empty() {
            bail!("\"nombre\" está vacío");
        }
        if let Some(c) = &self.conf.colores {
            Rgb::desde_hex(&c.acento).context("colores.acento")?;
            if c.modo == Modo::Claro && c.acento_en_barra {
                self.advertencias
                    .push("acentoEnBarra no tiene efecto en modo claro; se ignora".into());
            }
        }
        if let Some(f) = &self.conf.fondos {
            if f.intervalo_seg < 5 {
                bail!("fondos.intervaloSeg debe ser ≥ 5 (es {})", f.intervalo_seg);
            }
            if listar_imagenes(&self.dir.join("fondos"))?.is_empty() {
                bail!(r"la carpeta fondos\ no tiene imágenes");
            }
        }
        if let Some(b) = &self.conf.bloqueo {
            if !self.dir.join(b).is_file() {
                bail!("no existe la imagen de bloqueo {b}");
            }
        }
        if let Some(t) = &self.conf.terminal {
            for (campo, v) in [("fondo", &t.fondo), ("texto", &t.texto), ("cursor", &t.cursor)] {
                Rgb::desde_hex(v).with_context(|| format!("terminal.{campo}"))?;
            }
            for (k, v) in &t.ansi {
                if !NOMBRES_ANSI.contains(&k.as_str()) {
                    bail!("terminal.ansi: color desconocido {k:?}");
                }
                Rgb::desde_hex(v).with_context(|| format!("terminal.ansi.{k}"))?;
            }
        }
        Ok(())
    }

    /// ¿El tema configura este módulo?
    pub fn define(&self, modulo: &str) -> bool {
        let c = &self.conf;
        match modulo {
            "colores" => c.colores.is_some(),
            "fondos" => c.fondos.is_some(),
            "bloqueo" => c.bloqueo.is_some(),
            "terminal" => c.terminal.is_some(),
            "sonidos" => c.sonidos.is_some(),
            "intro" => c.intro.is_some(),
            "protector" => c.protector.is_some(),
            "iconos" => c.iconos.is_some(),
            "widget" => c.widget.is_some(),
            _ => false,
        }
    }
}

/// Imágenes de una carpeta (no recursivo), ordenadas por nombre. Carpeta inexistente → vacío.
pub fn listar_imagenes(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| EXTENSIONES_IMAGEN.contains(&e.to_ascii_lowercase().as_str()))
                    .unwrap_or(false)
        })
        .collect();
    v.sort();
    Ok(v)
}

/// Temas de la carpeta, ordenados por id. Ignora archivos y carpetas que empiezan con `_` o `.`.
/// Los inválidos se devuelven como `Err` para mostrarlos marcados.
pub fn listar_temas(carpeta: &Path) -> Result<Vec<(String, Result<Tema>)>> {
    let mut out = vec![];
    if !carpeta.is_dir() {
        return Ok(out);
    }
    for e in std::fs::read_dir(carpeta)? {
        let p = e?.path();
        let nombre = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if !p.is_dir() || nombre.starts_with('_') || nombre.starts_with('.') {
            continue;
        }
        out.push((nombre, Tema::cargar(&p)));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    /// Crea `<base>/<id>/tema.json` y las imágenes indicadas en `fondos/`.
    fn crear(base: &Path, id: &str, conf: Value, fondos: &[&str]) -> PathBuf {
        let dir = base.join(id);
        fs::create_dir_all(dir.join("fondos")).unwrap();
        fs::write(dir.join("tema.json"), conf.to_string()).unwrap();
        for f in fondos {
            fs::write(dir.join("fondos").join(f), b"x").unwrap();
        }
        dir
    }

    #[test]
    fn minimo_solo_nombre() {
        let t = tempfile::tempdir().unwrap();
        let tema = Tema::cargar(&crear(t.path(), "min", json!({"nombre": "Mínimo"}), &[])).unwrap();
        assert_eq!(tema.id, "min");
        assert_eq!(tema.conf.nombre, "Mínimo");
        for m in ["colores", "fondos", "bloqueo", "terminal", "sonidos"] {
            assert!(!tema.define(m), "{m} no debería estar definido");
        }
    }

    #[test]
    fn completo_con_valores_por_defecto() {
        let t = tempfile::tempdir().unwrap();
        let dir = crear(
            t.path(),
            "got",
            json!({
                "nombre": "Game of Thrones",
                "colores": {"acento": "#C9A227", "modo": "oscuro", "acentoEnBarra": true},
                "fondos": {},
                "bloqueo": "fondos/a.jpg",
                "terminal": {"fondo": "#141414", "texto": "#E8D9B0", "cursor": "#C9A227", "ansi": {"red": "#AA0000"}}
            }),
            &["a.jpg", "b.PNG"],
        );
        let tema = Tema::cargar(&dir).unwrap();
        let f = tema.conf.fondos.as_ref().unwrap();
        assert_eq!(f.intervalo_seg, 20);
        assert_eq!(f.orden, Orden::Aleatorio);
        assert_eq!(f.estilo, Estilo::Rellenar);
        assert!(tema.conf.colores.as_ref().unwrap().transparencia);
        assert!(tema.define("colores") && tema.define("fondos") && tema.define("bloqueo") && tema.define("terminal"));
        assert!(tema.advertencias.is_empty());
    }

    #[test]
    fn errores_de_validacion() {
        let t = tempfile::tempdir().unwrap();
        let casos = [
            json!({"nombre": "  "}),
            json!({"nombre": "X", "colores": {"acento": "amarillo", "modo": "oscuro"}}),
            json!({"nombre": "X", "colores": {"acento": "#FFFFFF", "modo": "gris"}}),
            json!({"nombre": "X", "fondos": {"intervaloSeg": 3}}),
            json!({"nombre": "X", "fondos": {}}),
            json!({"nombre": "X", "bloqueo": "no-existe.jpg"}),
            json!({"nombre": "X", "terminal": {"fondo": "#000000", "texto": "#FFFFFF", "cursor": "#FFFFFF", "ansi": {"rosa": "#FF00FF"}}}),
        ];
        for (i, c) in casos.into_iter().enumerate() {
            let dir = crear(t.path(), &format!("caso{i}"), c.clone(), &[]);
            assert!(Tema::cargar(&dir).is_err(), "el caso {i} debería fallar: {c}");
        }
    }

    #[test]
    fn modo_claro_con_acento_en_barra_advierte() {
        let t = tempfile::tempdir().unwrap();
        let dir = crear(
            t.path(),
            "claro",
            json!({"nombre": "C", "colores": {"acento": "#FFFFFF", "modo": "claro", "acentoEnBarra": true}}),
            &[],
        );
        assert_eq!(Tema::cargar(&dir).unwrap().advertencias.len(), 1);
    }

    #[test]
    fn tolera_bom_utf8() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("bom");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("tema.json"), "\u{feff}{\"nombre\": \"Con BOM\"}").unwrap();
        assert_eq!(Tema::cargar(&dir).unwrap().conf.nombre, "Con BOM");
    }

    #[test]
    fn listar_imagenes_filtra_y_ordena() {
        let t = tempfile::tempdir().unwrap();
        for f in ["b.JPG", "a.png", "c.txt", "d.bmp", "e.jpeg"] {
            fs::write(t.path().join(f), b"x").unwrap();
        }
        let nombres: Vec<String> = listar_imagenes(t.path())
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(nombres, ["a.png", "b.JPG", "d.bmp", "e.jpeg"]);
        assert!(listar_imagenes(&t.path().join("no-existe")).unwrap().is_empty());
    }

    #[test]
    fn listar_temas_ignora_ocultos_y_reporta_invalidos() {
        let t = tempfile::tempdir().unwrap();
        crear(t.path(), "zeta", json!({"nombre": "Z"}), &[]);
        crear(t.path(), "alfa", json!({"nombre": ""}), &[]);
        crear(t.path(), "_bin", json!({"nombre": "no"}), &[]);
        fs::write(t.path().join("suelto.txt"), b"x").unwrap();
        let temas = listar_temas(t.path()).unwrap();
        let ids: Vec<&str> = temas.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["alfa", "zeta"]);
        assert!(temas[0].1.is_err());
        assert!(temas[1].1.is_ok());
    }
}
