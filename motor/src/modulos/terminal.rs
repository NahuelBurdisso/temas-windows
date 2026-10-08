//! Esquema de colores de Windows Terminal.

use super::Modulo;
use crate::tema::{Tema, TerminalConf};
use anyhow::{bail, Context, Result};
use jsonc_parser::cst::{CstInputValue, CstNode, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub const NOMBRE_ESQUEMA: &str = "Tema activo";

/// Esquema Campbell (el de fábrica) para completar lo que el tema no define.
pub const CAMPBELL: [(&str, &str); 16] = [
    ("black", "#0C0C0C"), ("red", "#C50F1F"), ("green", "#13A10E"), ("yellow", "#C19C00"),
    ("blue", "#0037DA"), ("purple", "#881798"), ("cyan", "#3A96DD"), ("white", "#CCCCCC"),
    ("brightBlack", "#767676"), ("brightRed", "#E74856"), ("brightGreen", "#16C60C"),
    ("brightYellow", "#F9F1A5"), ("brightBlue", "#3B78FF"), ("brightPurple", "#B4009E"),
    ("brightCyan", "#61D6D6"), ("brightWhite", "#F2F2F2"),
];

pub fn esquema_para(t: &TerminalConf) -> Value {
    let mut m = Map::new();
    m.insert("name".into(), NOMBRE_ESQUEMA.into());
    for (k, v) in CAMPBELL {
        m.insert(k.into(), t.ansi.get(k).cloned().unwrap_or_else(|| v.to_string()).into());
    }
    m.insert("background".into(), t.fondo.clone().into());
    m.insert("foreground".into(), t.texto.clone().into());
    m.insert("cursorColor".into(), t.cursor.clone().into());
    m.insert("selectionBackground".into(), t.cursor.clone().into());
    Value::Object(m)
}

fn opciones() -> ParseOptions {
    ParseOptions::default()
}

/// Valor actual de `profiles.defaults.colorScheme` (texto u objeto dark/light), si existe.
pub fn esquema_defecto(texto: &str) -> Result<Option<Value>> {
    let v = jsonc_parser::parse_to_serde_value(texto, &opciones())?.unwrap_or(Value::Null);
    Ok(v.pointer("/profiles/defaults/colorScheme").cloned())
}

fn a_cst(v: &Value) -> CstInputValue {
    match v {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => CstInputValue::Number(n.to_string()),
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(a) => CstInputValue::Array(a.iter().map(a_cst).collect()),
        Value::Object(o) => {
            CstInputValue::Object(o.iter().map(|(k, v)| (k.clone(), a_cst(v))).collect())
        }
    }
}

fn es_nuestro(nodo: &CstNode) -> bool {
    nodo.as_object()
        .and_then(|o| o.get("name"))
        .and_then(|p| p.value())
        .and_then(|v| v.as_string_lit())
        .and_then(|s| s.decoded_value().ok())
        .map(|n| n == NOMBRE_ESQUEMA)
        .unwrap_or(false)
}

fn quitar_nuestro(raiz: &CstRootNode) {
    if let Some(esquemas) = raiz.object_value().and_then(|o| o.array_value("schemes")) {
        for el in esquemas.elements() {
            if es_nuestro(&el) {
                el.remove();
            }
        }
    }
}

pub fn aplicar_texto(texto: &str, esquema: &Value) -> Result<String> {
    let raiz = CstRootNode::parse(texto, &opciones())?;
    let obj = raiz.object_value_or_set();
    if let Some(p) = obj.get("profiles") {
        if p.value().and_then(|v| v.as_object()).is_none() {
            bail!("\"profiles\" tiene el formato antiguo (lista); no soportado");
        }
    }
    quitar_nuestro(&raiz);
    obj.array_value_or_set("schemes").append(a_cst(esquema));
    let defaults = obj.object_value_or_set("profiles").object_value_or_set("defaults");
    let valor = CstInputValue::String(NOMBRE_ESQUEMA.into());
    match defaults.get("colorScheme") {
        Some(p) => p.set_value(valor),
        None => {
            defaults.append("colorScheme", valor);
        }
    }
    Ok(raiz.to_string())
}

pub fn restaurar_texto(texto: &str, previo: Option<&Value>) -> Result<String> {
    let raiz = CstRootNode::parse(texto, &opciones())?;
    quitar_nuestro(&raiz);
    let defaults = raiz
        .object_value()
        .and_then(|o| o.object_value("profiles"))
        .and_then(|p| p.object_value("defaults"));
    if let Some(d) = defaults {
        match (previo, d.get("colorScheme")) {
            (Some(v), Some(p)) => p.set_value(a_cst(v)),
            (Some(v), None) => {
                d.append("colorScheme", a_cst(v));
            }
            (None, Some(p)) => p.remove(),
            (None, None) => {}
        }
    }
    Ok(raiz.to_string())
}

fn leer(ruta: &Path) -> Result<String> {
    let t = std::fs::read_to_string(ruta).with_context(|| format!("leer {}", ruta.display()))?;
    Ok(t.trim_start_matches('\u{feff}').to_string())
}

pub struct Terminal {
    /// Rutas posibles de settings.json; se usan las que existan.
    pub candidatos: Vec<PathBuf>,
}

impl Terminal {
    pub fn por_defecto() -> Terminal {
        let la = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default());
        Terminal {
            candidatos: vec![
                la.join(r"Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json"),
                la.join(r"Packages\Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe\LocalState\settings.json"),
                la.join(r"Microsoft\Windows Terminal\settings.json"),
            ],
        }
    }

    fn existentes(&self) -> Vec<&PathBuf> {
        self.candidatos.iter().filter(|p| p.is_file()).collect()
    }
}

impl Modulo for Terminal {
    fn id(&self) -> &'static str {
        "terminal"
    }

    /// Sin Windows Terminal instalado devuelve una lista vacía: el módulo no hace nada.
    fn capturar(&self, _dir: &Path) -> Result<Value> {
        let mut lista = vec![];
        for p in self.existentes() {
            lista.push(json!({ "ruta": p, "colorScheme": esquema_defecto(&leer(p)?)? }));
        }
        Ok(Value::Array(lista))
    }

    fn aplicar(&self, tema: &Tema) -> Result<()> {
        let conf = tema.conf.terminal.as_ref().context("el tema no define terminal")?;
        let esquema = esquema_para(conf);
        let mut errores = vec![];
        for p in self.existentes() {
            let r = (|| -> Result<()> {
                let bak = p.with_extension("json.bak");
                if !bak.exists() {
                    std::fs::copy(p, &bak)?;
                }
                std::fs::write(p, aplicar_texto(&leer(p)?, &esquema)?)?;
                Ok(())
            })();
            if let Err(e) = r {
                errores.push(format!("{}: {e:#}", p.display()));
            }
        }
        if !errores.is_empty() {
            bail!(errores.join("; "));
        }
        Ok(())
    }

    fn restaurar(&self, captura: &Value, _dir: &Path) -> Result<()> {
        let lista = captura.as_array().context("captura de terminal inválida")?;
        for item in lista {
            let ruta = PathBuf::from(item["ruta"].as_str().context("captura sin ruta")?);
            if !ruta.is_file() {
                continue;
            }
            let previo = item.get("colorScheme").filter(|v| !v.is_null());
            std::fs::write(&ruta, restaurar_texto(&leer(&ruta)?, previo)?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tema::TerminalConf;
    use serde_json::json;

    const SETTINGS: &str = r#"{
    // comentario del usuario
    "defaultProfile": "{abc}",
    "profiles": {
        "defaults": {
            "colorScheme": "One Half Dark",
        },
        "list": [ { "name": "PowerShell", "guid": "{abc}" } ]
    },
    "schemes": [ { "name": "Mio", "background": "black" } ],
}"#;

    fn conf() -> TerminalConf {
        let mut ansi = std::collections::BTreeMap::new();
        ansi.insert("red".to_string(), "#AA0000".to_string());
        TerminalConf { fondo: "#141414".into(), texto: "#E8D9B0".into(), cursor: "#C9A227".into(), ansi }
    }

    fn parsear(t: &str) -> Value {
        jsonc_parser::parse_to_serde_value(t, &Default::default()).unwrap().unwrap()
    }

    fn nuestros(v: &Value) -> usize {
        v["schemes"].as_array().unwrap().iter().filter(|s| s["name"] == NOMBRE_ESQUEMA).count()
    }

    #[test]
    fn esquema_completa_desde_campbell_y_pisa_lo_del_tema() {
        let e = esquema_para(&conf());
        assert_eq!(e["name"], NOMBRE_ESQUEMA);
        assert_eq!(e["background"], "#141414");
        assert_eq!(e["foreground"], "#E8D9B0");
        assert_eq!(e["cursorColor"], "#C9A227");
        assert_eq!(e["selectionBackground"], "#C9A227");
        assert_eq!(e["red"], "#AA0000");
        assert_eq!(e["green"], "#13A10E", "lo que falta sale de Campbell");
        assert_eq!(e.as_object().unwrap().len(), 21);
    }

    #[test]
    fn lee_el_esquema_por_defecto() {
        assert_eq!(esquema_defecto(SETTINGS).unwrap(), Some(json!("One Half Dark")));
        assert_eq!(esquema_defecto("{}").unwrap(), None);
    }

    #[test]
    fn aplicar_agrega_esquema_fija_defecto_y_conserva_comentarios() {
        let nuevo = aplicar_texto(SETTINGS, &esquema_para(&conf())).unwrap();
        assert!(nuevo.contains("// comentario del usuario"));
        let v = parsear(&nuevo);
        assert_eq!(nuestros(&v), 1);
        assert_eq!(v["profiles"]["defaults"]["colorScheme"], NOMBRE_ESQUEMA);
        assert!(v["schemes"].as_array().unwrap().iter().any(|s| s["name"] == "Mio"));
    }

    #[test]
    fn aplicar_dos_veces_no_duplica() {
        let e = esquema_para(&conf());
        let dos = aplicar_texto(&aplicar_texto(SETTINGS, &e).unwrap(), &e).unwrap();
        assert_eq!(nuestros(&parsear(&dos)), 1);
    }

    #[test]
    fn aplicar_crea_lo_que_falta() {
        let v = parsear(&aplicar_texto("{}", &esquema_para(&conf())).unwrap());
        assert_eq!(nuestros(&v), 1);
        assert_eq!(v["profiles"]["defaults"]["colorScheme"], NOMBRE_ESQUEMA);
    }

    #[test]
    fn perfiles_en_formato_lista_es_error() {
        assert!(aplicar_texto(r#"{"profiles": []}"#, &esquema_para(&conf())).is_err());
    }

    #[test]
    fn restaurar_repone_o_borra_el_defecto_y_quita_el_esquema() {
        let aplicado = aplicar_texto(SETTINGS, &esquema_para(&conf())).unwrap();

        let v = parsear(&restaurar_texto(&aplicado, Some(&json!("One Half Dark"))).unwrap());
        assert_eq!(nuestros(&v), 0);
        assert_eq!(v["profiles"]["defaults"]["colorScheme"], "One Half Dark");

        let r = restaurar_texto(&aplicado, None).unwrap();
        assert!(r.contains("// comentario del usuario"));
        assert!(parsear(&r)["profiles"]["defaults"].get("colorScheme").is_none());
    }

    #[test]
    fn modulo_ida_y_vuelta_sobre_archivo_temporal() {
        let t = tempfile::tempdir().unwrap();
        let archivo = t.path().join("settings.json");
        std::fs::write(&archivo, SETTINGS).unwrap();
        let m = Terminal { candidatos: vec![archivo.clone(), t.path().join("no-existe.json")] };

        let tema_dir = t.path().join("tema");
        std::fs::create_dir_all(&tema_dir).unwrap();
        let tema_json = json!({"nombre": "T", "terminal": {"fondo": "#000000", "texto": "#FFFFFF", "cursor": "#FFFFFF"}});
        std::fs::write(tema_dir.join("tema.json"), tema_json.to_string()).unwrap();
        let tema = Tema::cargar(&tema_dir).unwrap();

        let captura = m.capturar(t.path()).unwrap();
        m.aplicar(&tema).unwrap();
        assert!(t.path().join("settings.json.bak").exists());
        assert_eq!(parsear(&std::fs::read_to_string(&archivo).unwrap())["profiles"]["defaults"]["colorScheme"], NOMBRE_ESQUEMA);
        m.restaurar(&captura, t.path()).unwrap();
        assert_eq!(parsear(&std::fs::read_to_string(&archivo).unwrap()), parsear(SETTINGS));
    }

    #[test]
    fn sin_windows_terminal_se_saltea_sin_error() {
        let m = Terminal { candidatos: vec![PathBuf::from(r"Z:\no\existe.json")] };
        let captura = m.capturar(Path::new(".")).unwrap();
        assert_eq!(captura, json!([]));
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("t");
        std::fs::create_dir_all(&dir).unwrap();
        let conf = json!({"nombre": "T", "terminal": {"fondo": "#000000", "texto": "#FFFFFF", "cursor": "#FFFFFF"}});
        std::fs::write(dir.join("tema.json"), conf.to_string()).unwrap();
        m.aplicar(&Tema::cargar(&dir).unwrap()).unwrap();
        m.restaurar(&captura, Path::new(".")).unwrap();
    }
}
