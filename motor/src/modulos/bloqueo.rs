//! Pantalla de bloqueo.

use super::Modulo;
use crate::registro::{self, RegValor};
use crate::tema::Tema;
use crate::win;
use anyhow::{bail, Context, Result};
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
    let tam = u32::try_from(stream.Size()?).context("la imagen de bloqueo es demasiado grande")?;
    let lector = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    let leidos = lector.LoadAsync(tam)?.get()?;
    if leidos != tam {
        bail!("lectura incompleta de la imagen de bloqueo: {leidos} de {tam} bytes");
    }
    let mut buf = vec![0u8; leidos as usize];
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
    let v = &captura["registro"][clave];
    v["tipo"] == "dword" && v["valor"].as_u64().is_some_and(|n| n != 0)
}

pub struct Bloqueo;

impl Modulo for Bloqueo {
    fn id(&self) -> &'static str {
        "bloqueo"
    }

    fn capturar(&self, dir: &Path) -> Result<Value> {
        let registro = registro::capturar_valores(&CLAVES)?;
        // Con Spotlight activo puede no haber imagen propia; sin Spotlight, no tenerla es un error.
        let spot = spotlight_en(&json!({ "registro": &registro }));
        let archivo = match leer_imagen() {
            Ok(b) if !b.is_empty() => {
                let nombre = format!("bloqueo.{}", extension_por_contenido(&b));
                std::fs::write(dir.join(&nombre), &b)?;
                Some(nombre)
            }
            Ok(_) if spot => None,
            Ok(_) => bail!("la imagen de bloqueo actual vino vacía"),
            Err(_) if spot => None,
            Err(e) => return Err(e).context("leer la imagen de bloqueo actual"),
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
        // Se intenta todo aunque algo falle; los errores se devuelven juntos.
        let mut errores: Vec<String> = Vec::new();
        if let Some(n) = captura["archivo"].as_str() {
            if let Err(e) = poner_imagen(&dir.join(n)) {
                errores.push(format!("restaurar imagen de bloqueo: {e:#}"));
            }
        }
        if let Err(e) = registro::restaurar_valores(&captura["registro"]) {
            errores.push(format!("restaurar registro de bloqueo: {e:#}"));
        }
        if !errores.is_empty() {
            bail!("{}", errores.join("; "));
        }
        Ok(())
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
        assert!(spotlight_en(&captura(2)));
        assert!(!spotlight_en(&captura(0)));
        let ausente = serde_json::json!({
            "registro": { format!("{CDM}|RotatingLockScreenEnabled"): {"tipo": "ausente"} }
        });
        assert!(!spotlight_en(&ausente));
        assert!(!spotlight_en(&serde_json::json!({})));
    }
}
