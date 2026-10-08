//! Dónde vive el estado de la app y cómo se lee/escribe.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Rutas {
    pub raiz: PathBuf,
}

impl Rutas {
    pub fn nuevo(raiz: impl Into<PathBuf>) -> Rutas {
        Rutas { raiz: raiz.into() }
    }
    pub fn por_defecto() -> Rutas {
        Rutas::nuevo(r"D:\TemasApp")
    }
    pub fn estado(&self) -> PathBuf {
        self.raiz.join("estado.json")
    }
    pub fn ajustes(&self) -> PathBuf {
        self.raiz.join("ajustes.json")
    }
    pub fn original(&self) -> PathBuf {
        self.raiz.join("original.json")
    }
    pub fn original_prev(&self) -> PathBuf {
        self.raiz.join("original.prev.json")
    }
    pub fn dir_original(&self, modulo: &str) -> PathBuf {
        self.raiz.join("original").join(modulo)
    }
    pub fn dir_original_prev(&self, modulo: &str) -> PathBuf {
        self.raiz.join("original.prev").join(modulo)
    }
}

pub(crate) fn leer_json<T: DeserializeOwned + Default>(ruta: &Path) -> Result<T> {
    if !ruta.exists() {
        return Ok(T::default());
    }
    let texto = fs::read_to_string(ruta).with_context(|| format!("leer {}", ruta.display()))?;
    serde_json::from_str(texto.trim_start_matches('\u{feff}'))
        .with_context(|| format!("{} mal formado", ruta.display()))
}

/// Escribe en un temporal y renombra, para no dejar archivos a medias.
pub(crate) fn escribir_json<T: Serialize>(ruta: &Path, v: &T) -> Result<()> {
    if let Some(p) = ruta.parent() {
        fs::create_dir_all(p)?;
    }
    let tmp = ruta.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(v)?)?;
    fs::rename(&tmp, ruta).with_context(|| format!("guardar {}", ruta.display()))?;
    Ok(())
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Estado {
    pub tema_activo: Option<String>,
    /// Módulos que el tema activo intentó aplicar (bien o mal) y que hay que restaurar al cambiar.
    pub tocados: Vec<String>,
    pub fondos_pausados: bool,
}

impl Estado {
    pub fn cargar(r: &Rutas) -> Result<Estado> {
        leer_json(&r.estado())
    }
    pub fn guardar(&self, r: &Rutas) -> Result<()> {
        escribir_json(&r.estado(), self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Ajustes {
    pub carpeta_temas: PathBuf,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes { carpeta_temas: PathBuf::from(r"D:\Temas") }
    }
}

impl Ajustes {
    pub fn cargar(r: &Rutas) -> Result<Ajustes> {
        leer_json(&r.ajustes())
    }
    pub fn guardar(&self, r: &Rutas) -> Result<()> {
        escribir_json(&r.ajustes(), self)
    }
}

/// Capturas del estado original: `módulo → captura`.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Original(pub BTreeMap<String, Value>);

impl Original {
    pub fn cargar(r: &Rutas) -> Result<Original> {
        leer_json(&r.original())
    }
    pub fn guardar(&self, r: &Rutas) -> Result<()> {
        escribir_json(&r.original(), self)
    }
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.0.get(id)
    }
    pub fn poner(&mut self, id: &str, v: Value) {
        self.0.insert(id.to_string(), v);
    }
}

/// Pasa las capturas de `ids` (y sus carpetas) de `original` a `original.prev`.
pub fn archivar_original(r: &Rutas, ids: &[String]) -> Result<()> {
    let mut orig = Original::cargar(r)?;
    let mut prev: Original = leer_json(&r.original_prev())?;
    for id in ids {
        if let Some(v) = orig.0.remove(id) {
            prev.0.insert(id.clone(), v);
        }
        let src = r.dir_original(id);
        let dst = r.dir_original_prev(id);
        if dst.exists() {
            fs::remove_dir_all(&dst)?;
        }
        if src.exists() {
            fs::create_dir_all(dst.parent().expect("dir_original_prev tiene padre"))?;
            fs::rename(&src, &dst)?;
        }
    }
    escribir_json(&r.original_prev(), &prev)?;
    orig.guardar(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rutas() -> (tempfile::TempDir, Rutas) {
        let t = tempfile::tempdir().unwrap();
        let r = Rutas::nuevo(t.path());
        (t, r)
    }

    #[test]
    fn sin_archivos_hay_valores_por_defecto() {
        let (_t, r) = rutas();
        assert_eq!(Estado::cargar(&r).unwrap(), Estado::default());
        assert_eq!(Ajustes::cargar(&r).unwrap().carpeta_temas, PathBuf::from(r"D:\Temas"));
        assert!(Original::cargar(&r).unwrap().0.is_empty());
    }

    #[test]
    fn estado_ida_y_vuelta() {
        let (_t, r) = rutas();
        let e = Estado { tema_activo: Some("star-wars".into()), tocados: vec!["fondos".into()], fondos_pausados: true };
        e.guardar(&r).unwrap();
        assert_eq!(Estado::cargar(&r).unwrap(), e);
        let texto = std::fs::read_to_string(r.estado()).unwrap();
        assert!(texto.contains("temaActivo"), "se guarda en camelCase: {texto}");
    }

    #[test]
    fn original_ida_y_vuelta() {
        let (_t, r) = rutas();
        let mut o = Original::default();
        o.poner("colores", json!({"a": 1}));
        o.guardar(&r).unwrap();
        assert_eq!(Original::cargar(&r).unwrap().get("colores"), Some(&json!({"a": 1})));
    }

    #[test]
    fn archivar_mueve_entradas_y_carpetas_elegidas() {
        let (_t, r) = rutas();
        let mut o = Original::default();
        o.poner("fondos", json!({"archivo": "fondo.jpg"}));
        o.poner("colores", json!({"a": 1}));
        o.guardar(&r).unwrap();
        std::fs::create_dir_all(r.dir_original("fondos")).unwrap();
        std::fs::write(r.dir_original("fondos").join("fondo.jpg"), b"nuevo").unwrap();
        // Un .prev viejo de fondos tiene que ser reemplazado.
        std::fs::create_dir_all(r.dir_original_prev("fondos")).unwrap();
        std::fs::write(r.dir_original_prev("fondos").join("viejo.jpg"), b"viejo").unwrap();

        archivar_original(&r, &["fondos".to_string()]).unwrap();

        let o = Original::cargar(&r).unwrap();
        assert!(o.get("fondos").is_none());
        assert!(o.get("colores").is_some(), "lo no archivado queda");
        let prev: Original = leer_json(&r.original_prev()).unwrap();
        assert_eq!(prev.get("fondos"), Some(&json!({"archivo": "fondo.jpg"})));
        assert!(!r.dir_original("fondos").exists());
        assert!(r.dir_original_prev("fondos").join("fondo.jpg").exists());
        assert!(!r.dir_original_prev("fondos").join("viejo.jpg").exists());
    }

    #[test]
    fn tolera_bom() {
        let (_t, r) = rutas();
        std::fs::create_dir_all(&r.raiz).unwrap();
        std::fs::write(r.original(), "\u{feff}{\"fondos\": {\"archivo\": \"fondo.jpg\"}}").unwrap();
        assert!(Original::cargar(&r).unwrap().get("fondos").is_some());
    }
}
