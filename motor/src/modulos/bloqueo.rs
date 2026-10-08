//! Pantalla de bloqueo.

use super::Modulo;
use crate::registro::{self, RegValor};
use crate::tema::Tema;
use crate::win;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use windows::core::HSTRING;
use windows::Storage::StorageFile;
use windows::Storage::Streams::DataReader;
use windows::System::UserProfile::LockScreen;

pub const CDM: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
pub const CLAVES: [(&str, &str); 2] = [
    (CDM, "RotatingLockScreenEnabled"),
    (CDM, "RotatingLockScreenOverlayEnabled"),
];

pub fn poner_imagen(ruta: &Path) -> Result<()> {
    win::iniciar_winrt();
    let abs = std::path::absolute(ruta)?;
    let archivo = StorageFile::GetFileFromPathAsync(&HSTRING::from(abs.to_string_lossy().as_ref()))?
        .get()
        .with_context(|| format!("abrir {}", abs.display()))?;
    LockScreen::SetImageFileAsync(&archivo)?
        .get()
        .context("LockScreen.SetImageFileAsync")?;
    Ok(())
}

pub fn leer_imagen() -> Result<Vec<u8>> {
    win::iniciar_winrt();
    let stream = LockScreen::GetImageStream()?;
    let tam = stream.Size()? as u32;
    let lector = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    lector.LoadAsync(tam)?.get()?;
    let mut buf = vec![0u8; tam as usize];
    lector.ReadBytes(&mut buf)?;
    Ok(buf)
}

pub fn extension_por_contenido(b: &[u8]) -> &'static str {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if b.starts_with(b"BM") {
        "bmp"
    } else {
        "jpg"
    }
}

fn spotlight_en(captura: &Value) -> bool {
    let clave = format!("{CDM}|RotatingLockScreenEnabled");
    captura["registro"][clave] == json!({"tipo": "dword", "valor": 1})
}

pub struct Bloqueo;

impl Modulo for Bloqueo {
    fn id(&self) -> &'static str {
        "bloqueo"
    }

    fn capturar(&self, dir: &Path) -> Result<Value> {
        let registro = registro::capturar_valores(&CLAVES)?;
        let archivo = match leer_imagen() {
            Ok(b) if !b.is_empty() => {
                let nombre = format!("bloqueo.{}", extension_por_contenido(&b));
                std::fs::write(dir.join(&nombre), &b)?;
                Some(nombre)
            }
            _ => None,
        };
        Ok(json!({ "registro": registro, "archivo": archivo }))
    }

    fn aplicar(&self, tema: &Tema) -> Result<()> {
        let rel = tema.conf.bloqueo.as_ref().context("el tema no define bloqueo")?;
        poner_imagen(&tema.dir.join(rel))?;
        for (ruta, nombre) in CLAVES {
            registro::escribir(ruta, nombre, &RegValor::Dword(0))?;
        }
        Ok(())
    }

    fn restaurar(&self, captura: &Value, dir: &Path) -> Result<()> {
        if !spotlight_en(captura) {
            if let Some(n) = captura["archivo"].as_str() {
                poner_imagen(&dir.join(n))?;
            }
        }
        registro::restaurar_valores(&captura["registro"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_formato_por_contenido() {
        assert_eq!(extension_por_contenido(&[0x89, b'P', b'N', b'G', 0x0D]), "png");
        assert_eq!(extension_por_contenido(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpg");
        assert_eq!(extension_por_contenido(b"BM\x00\x00"), "bmp");
        assert_eq!(extension_por_contenido(b"????"), "jpg");
    }

    #[test]
    fn spotlight_activo_se_detecta_en_la_captura() {
        let captura = |v: u32| {
            let mut reg = serde_json::Map::new();
            reg.insert(
                format!("{CDM}|RotatingLockScreenEnabled"),
                serde_json::json!({"tipo": "dword", "valor": v}),
            );
            serde_json::json!({ "registro": reg })
        };
        assert!(spotlight_en(&captura(1)));
        assert!(!spotlight_en(&captura(0)));
        assert!(!spotlight_en(&serde_json::json!({})));
    }
}
