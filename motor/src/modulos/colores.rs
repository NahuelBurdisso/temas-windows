//! Colores del sistema: acento, modo oscuro/claro, transparencia, acento en barra.

use super::Modulo;
use crate::color::{paleta_acento, Rgb};
use crate::registro::{self, RegValor};
use crate::tema::{ColoresConf, Modo, Tema};
use crate::win;
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

pub const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
pub const DWM: &str = r"Software\Microsoft\Windows\DWM";
pub const ACCENT: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent";
pub const DESKTOP: &str = r"Control Panel\Desktop";

/// Todo lo que el módulo puede tocar (se captura entero).
pub const CLAVES: [(&str, &str); 11] = [
    (PERSONALIZE, "AppsUseLightTheme"),
    (PERSONALIZE, "SystemUsesLightTheme"),
    (PERSONALIZE, "EnableTransparency"),
    (PERSONALIZE, "ColorPrevalence"),
    (DWM, "AccentColor"),
    (DWM, "ColorizationColor"),
    (DWM, "ColorPrevalence"),
    (ACCENT, "AccentPalette"),
    (ACCENT, "AccentColorMenu"),
    (ACCENT, "StartColorMenu"),
    (DESKTOP, "AutoColorization"),
];

/// Valores de registro para una configuración de colores (sin AutoColorization, que va aparte).
pub fn valores_para(c: &ColoresConf) -> Result<Vec<(&'static str, &'static str, RegValor)>> {
    let base = Rgb::desde_hex(&c.acento)?;
    let claro = (c.modo == Modo::Claro) as u32;
    let barra = (c.acento_en_barra && c.modo == Modo::Oscuro) as u32;
    Ok(vec![
        (PERSONALIZE, "AppsUseLightTheme", RegValor::Dword(claro)),
        (PERSONALIZE, "SystemUsesLightTheme", RegValor::Dword(claro)),
        (PERSONALIZE, "EnableTransparency", RegValor::Dword(c.transparencia as u32)),
        (PERSONALIZE, "ColorPrevalence", RegValor::Dword(barra)),
        (DWM, "AccentColor", RegValor::Dword(base.abgr())),
        (DWM, "ColorizationColor", RegValor::Dword(base.argb(0xC4))),
        (DWM, "ColorPrevalence", RegValor::Dword(barra)),
        (ACCENT, "AccentPalette", RegValor::Binario(paleta_acento(base).to_vec())),
        (ACCENT, "AccentColorMenu", RegValor::Dword(base.abgr())),
        (ACCENT, "StartColorMenu", RegValor::Dword(base.con_luz(-0.20).abgr())),
    ])
}

/// Apaga "elegir el acento automáticamente según el fondo", respetando el tipo de valor existente.
fn apagar_autocolor() -> Result<()> {
    let nuevo = match registro::leer(DESKTOP, "AutoColorization")? {
        RegValor::Sz(_) => RegValor::Sz("0".into()),
        _ => RegValor::Dword(0),
    };
    registro::escribir(DESKTOP, "AutoColorization", &nuevo)
}

pub struct Colores;

impl Modulo for Colores {
    fn id(&self) -> &'static str {
        "colores"
    }

    fn capturar(&self, _dir: &Path) -> Result<Value> {
        registro::capturar_valores(&CLAVES)
    }

    fn aplicar(&self, tema: &Tema) -> Result<()> {
        let c = tema.conf.colores.as_ref().context("el tema no define colores")?;
        apagar_autocolor()?;
        for (ruta, nombre, valor) in valores_para(c)? {
            registro::escribir(ruta, nombre, &valor)?;
        }
        win::avisar_cambio("ImmersiveColorSet");
        Ok(())
    }

    fn restaurar(&self, captura: &Value, _dir: &Path) -> Result<()> {
        registro::restaurar_valores(captura)?;
        win::avisar_cambio("ImmersiveColorSet");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tema::{ColoresConf, Modo};

    fn conf(modo: Modo, barra: bool) -> ColoresConf {
        ColoresConf { acento: "#C9A227".into(), modo, transparencia: true, acento_en_barra: barra }
    }

    fn valor(vs: &[(&str, &str, RegValor)], ruta: &str, nombre: &str) -> RegValor {
        vs.iter().find(|(r, n, _)| *r == ruta && *n == nombre).unwrap().2.clone()
    }

    #[test]
    fn oscuro_con_barra() {
        let vs = valores_para(&conf(Modo::Oscuro, true)).unwrap();
        assert_eq!(valor(&vs, PERSONALIZE, "AppsUseLightTheme"), RegValor::Dword(0));
        assert_eq!(valor(&vs, PERSONALIZE, "SystemUsesLightTheme"), RegValor::Dword(0));
        assert_eq!(valor(&vs, PERSONALIZE, "ColorPrevalence"), RegValor::Dword(1));
        assert_eq!(valor(&vs, DWM, "ColorPrevalence"), RegValor::Dword(1));
        assert_eq!(valor(&vs, DWM, "AccentColor"), RegValor::Dword(0xFF27_A2C9));
        assert_eq!(valor(&vs, DWM, "ColorizationColor"), RegValor::Dword(0xC4C9_A227));
        assert_eq!(valor(&vs, ACCENT, "AccentColorMenu"), RegValor::Dword(0xFF27_A2C9));
        match valor(&vs, ACCENT, "AccentPalette") {
            RegValor::Binario(b) => assert_eq!(b.len(), 32),
            otro => panic!("AccentPalette debería ser binario: {otro:?}"),
        }
    }

    #[test]
    fn claro_ignora_la_barra() {
        let vs = valores_para(&conf(Modo::Claro, true)).unwrap();
        assert_eq!(valor(&vs, PERSONALIZE, "AppsUseLightTheme"), RegValor::Dword(1));
        assert_eq!(valor(&vs, PERSONALIZE, "ColorPrevalence"), RegValor::Dword(0));
    }

    #[test]
    fn las_claves_capturadas_cubren_todo_lo_que_se_escribe() {
        let vs = valores_para(&conf(Modo::Oscuro, true)).unwrap();
        for (r, n, _) in &vs {
            assert!(CLAVES.contains(&(*r, *n)), "{r}|{n} se escribe pero no se captura");
        }
        assert!(CLAVES.contains(&(DESKTOP, "AutoColorization")));
    }
}
