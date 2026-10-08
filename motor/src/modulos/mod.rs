//! Cada parte de Windows que un tema puede cambiar es un módulo.

use crate::tema::Tema;
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub trait Modulo: Send + Sync {
    /// Igual a la sección de `tema.json` ("colores", "fondos", …).
    fn id(&self) -> &'static str;
    /// Estado actual de Windows. Si hay que guardar archivos, van en `dir` (carpeta propia del módulo).
    fn capturar(&self, dir: &Path) -> Result<Value>;
    fn aplicar(&self, tema: &Tema) -> Result<()>;
    /// Vuelve a lo capturado. `dir` es la misma carpeta que recibió `capturar`.
    fn restaurar(&self, captura: &Value, dir: &Path) -> Result<()>;
}
