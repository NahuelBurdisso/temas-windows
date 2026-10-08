//! Dónde vive el estado de la app y cómo se lee/escribe.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Write};
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
    let texto = match fs::read_to_string(ruta) {
        Ok(t) => t,
        // Sólo "no existe" significa "usar el valor por defecto"; cualquier otro error se informa.
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(T::default()),
        Err(e) => return Err(e).with_context(|| format!("leer {}", ruta.display())),
    };
    serde_json::from_str(texto.trim_start_matches('\u{feff}'))
        .with_context(|| format!("{} mal formado", ruta.display()))
}

/// Escribe en un temporal (sincronizado a disco) y renombra, para no dejar archivos a medias.
pub(crate) fn escribir_json<T: Serialize>(ruta: &Path, v: &T) -> Result<()> {
    if let Some(p) = ruta.parent() {
        fs::create_dir_all(p)?;
    }
    let tmp = ruta.with_extension("json.tmp");
    let mut f = fs::File::create(&tmp).with_context(|| format!("crear {}", tmp.display()))?;
    f.write_all(serde_json::to_string_pretty(v)?.as_bytes())?;
    f.sync_all()?;
    drop(f);
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
///
/// El punto de confirmación es `original.json`: se escribe primero sin los ids archivados. Después se
/// copian las capturas a `original.prev.json` y por último se mueven las carpetas, sin abortar ante
/// errores: los problemas de carpetas se devuelven como avisos.
pub fn archivar_original(r: &Rutas, ids: &[String]) -> Result<Vec<String>> {
    let mut orig = Original::cargar(r)?;
    let mut archivadas = vec![];
    for id in ids {
        if let Some(v) = orig.0.remove(id) {
            archivadas.push((id.clone(), v));
        }
    }
    orig.guardar(r)?;

    let mut prev: Original = leer_json(&r.original_prev())?;
    for (id, v) in archivadas {
        prev.0.insert(id, v);
    }
    escribir_json(&r.original_prev(), &prev)?;

    let mut avisos = vec![];
    for id in ids {
        let src = r.dir_original(id);
        let dst = r.dir_original_prev(id);
        if dst.exists() {
            if let Err(e) = fs::remove_dir_all(&dst) {
                avisos.push(format!("no se pudo borrar {}: {e}", dst.display()));
                continue;
            }
        }
        if src.exists() {
            let movido = match dst.parent() {
                Some(p) => fs::create_dir_all(p).and_then(|_| fs::rename(&src, &dst)),
                None => fs::rename(&src, &dst),
            };
            if let Err(e) = movido {
                avisos.push(format!("no se pudo archivar {}: {e}", src.display()));
            }
        }
    }
    Ok(avisos)
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

        let avisos = archivar_original(&r, &["fondos".to_string()]).unwrap();
        assert!(avisos.is_empty(), "{avisos:?}");

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
    fn archivar_sin_carpeta_de_origen_no_avisa_nada() {
        let (_t, r) = rutas();
        let mut o = Original::default();
        o.poner("colores", json!({"a": 1}));
        o.guardar(&r).unwrap();
        let avisos = archivar_original(&r, &["colores".to_string()]).unwrap();
        assert!(avisos.is_empty(), "{avisos:?}");
        assert!(Original::cargar(&r).unwrap().get("colores").is_none());
        let prev: Original = leer_json(&r.original_prev()).unwrap();
        assert!(prev.get("colores").is_some());
    }

    #[test]
    fn error_de_io_que_no_es_not_found_se_propaga() {
        let (_t, r) = rutas();
        // Un directorio donde se espera el archivo: leerlo da un error que no es NotFound.
        std::fs::create_dir_all(r.estado()).unwrap();
        assert!(Estado::cargar(&r).is_err());
    }

    #[test]
    fn tolera_bom() {
        let (_t, r) = rutas();
        std::fs::create_dir_all(&r.raiz).unwrap();
        std::fs::write(r.original(), "\u{feff}{\"fondos\": {\"archivo\": \"fondo.jpg\"}}").unwrap();
        assert!(Original::cargar(&r).unwrap().get("fondos").is_some());
    }
}
