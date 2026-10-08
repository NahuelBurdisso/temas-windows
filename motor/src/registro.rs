//! Acceso a HKCU con valores serializables, para capturar y restaurar.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::io::ErrorKind;
use winreg::enums::*;
use winreg::{RegKey, RegValue};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", content = "valor", rename_all = "lowercase")]
pub enum RegValor {
    Ausente,
    Dword(u32),
    Sz(String),
    Binario(Vec<u8>),
}

pub(crate) fn hkcu() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
}

pub fn leer(ruta: &str, nombre: &str) -> Result<RegValor> {
    let clave = match hkcu().open_subkey(ruta) {
        Ok(k) => k,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(RegValor::Ausente),
        Err(e) => return Err(e).with_context(|| format!(r"abrir HKCU\{ruta}")),
    };
    let crudo = match clave.get_raw_value(nombre) {
        Ok(v) => v,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(RegValor::Ausente),
        Err(e) => return Err(e).with_context(|| format!(r"leer HKCU\{ruta}\{nombre}")),
    };
    let bytes = crudo.bytes.to_vec();
    Ok(match crudo.vtype {
        REG_DWORD if bytes.len() >= 4 => {
            RegValor::Dword(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        REG_SZ => RegValor::Sz(clave.get_value::<String, _>(nombre)?),
        REG_BINARY => RegValor::Binario(bytes),
        otro => bail!(r"HKCU\{ruta}\{nombre}: tipo de registro no soportado ({otro:?})"),
    })
}

pub fn escribir(ruta: &str, nombre: &str, valor: &RegValor) -> Result<()> {
    if let RegValor::Ausente = valor {
        if let Ok(k) = hkcu().open_subkey_with_flags(ruta, KEY_SET_VALUE) {
            match k.delete_value(nombre) {
                Ok(()) => {}
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(e).with_context(|| format!(r"borrar HKCU\{ruta}\{nombre}")),
            }
        }
        return Ok(());
    }
    let (clave, _) = hkcu()
        .create_subkey(ruta)
        .with_context(|| format!(r"crear HKCU\{ruta}"))?;
    match valor {
        RegValor::Dword(n) => clave.set_value(nombre, n),
        RegValor::Sz(s) => clave.set_value(nombre, s),
        RegValor::Binario(b) => clave.set_raw_value(
            nombre,
            &RegValue { bytes: b.clone().into(), vtype: REG_BINARY },
        ),
        RegValor::Ausente => unreachable!(),
    }
    .with_context(|| format!(r"escribir HKCU\{ruta}\{nombre}"))
}

/// (ruta, nombre) de un valor de HKCU.
pub type Clave<'a> = (&'a str, &'a str);

/// Lee varios valores y los devuelve como `{"ruta|nombre": RegValor}`.
pub fn capturar_valores(claves: &[Clave]) -> Result<Value> {
    let mut m = Map::new();
    for (ruta, nombre) in claves {
        m.insert(format!("{ruta}|{nombre}"), serde_json::to_value(leer(ruta, nombre)?)?);
    }
    Ok(Value::Object(m))
}

/// Escribe de vuelta lo que devolvió `capturar_valores` (los `Ausente` se borran).
pub fn restaurar_valores(captura: &Value) -> Result<()> {
    let m = captura.as_object().context("captura de registro inválida")?;
    for (k, v) in m {
        let (ruta, nombre) = k.split_once('|').context("clave de captura inválida")?;
        escribir(ruta, nombre, &serde_json::from_value(v.clone())?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clave de prueba única por test; se borra al soltarse.
    struct ClavePrueba(String);
    impl ClavePrueba {
        fn nueva(nombre: &str) -> Self {
            let ruta = format!(r"Software\TemasWindowsTest\{nombre}");
            let _ = hkcu().delete_subkey_all(&ruta);
            ClavePrueba(ruta)
        }
    }
    impl Drop for ClavePrueba {
        fn drop(&mut self) {
            let _ = hkcu().delete_subkey_all(&self.0);
        }
    }

    #[test]
    fn clave_inexistente_es_ausente() {
        let k = ClavePrueba::nueva("inexistente");
        assert_eq!(leer(&k.0, "X").unwrap(), RegValor::Ausente);
    }

    #[test]
    fn ida_y_vuelta_de_cada_tipo() {
        let k = ClavePrueba::nueva("tipos");
        for (nombre, v) in [
            ("d", RegValor::Dword(0xFF27_A2C9)),
            ("s", RegValor::Sz("10".into())),
            ("b", RegValor::Binario(vec![1, 2, 3, 0, 255])),
        ] {
            escribir(&k.0, nombre, &v).unwrap();
            assert_eq!(leer(&k.0, nombre).unwrap(), v);
        }
    }

    #[test]
    fn escribir_ausente_borra_el_valor() {
        let k = ClavePrueba::nueva("borrar");
        escribir(&k.0, "x", &RegValor::Dword(1)).unwrap();
        escribir(&k.0, "x", &RegValor::Ausente).unwrap();
        assert_eq!(leer(&k.0, "x").unwrap(), RegValor::Ausente);
        // Borrar algo que no existe no es error.
        escribir(&k.0, "x", &RegValor::Ausente).unwrap();
    }

    #[test]
    fn capturar_y_restaurar_vuelve_al_estado_previo() {
        let k = ClavePrueba::nueva("captura");
        escribir(&k.0, "a", &RegValor::Dword(1)).unwrap();
        let claves = [(k.0.as_str(), "a"), (k.0.as_str(), "b")];
        let captura = capturar_valores(&claves).unwrap();

        escribir(&k.0, "a", &RegValor::Dword(2)).unwrap();
        escribir(&k.0, "b", &RegValor::Sz("nuevo".into())).unwrap();
        restaurar_valores(&captura).unwrap();

        assert_eq!(leer(&k.0, "a").unwrap(), RegValor::Dword(1));
        assert_eq!(leer(&k.0, "b").unwrap(), RegValor::Ausente);
    }

    #[test]
    fn regvalor_se_serializa_con_tipo_y_valor() {
        let j = serde_json::to_value(RegValor::Dword(1)).unwrap();
        assert_eq!(j, serde_json::json!({"tipo": "dword", "valor": 1}));
        let j = serde_json::to_value(RegValor::Ausente).unwrap();
        assert_eq!(j, serde_json::json!({"tipo": "ausente"}));
    }
}
