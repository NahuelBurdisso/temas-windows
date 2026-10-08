//! Esquema de colores de Windows Terminal.

use super::Modulo;
use crate::tema::{Tema, TerminalConf};
use anyhow::{bail, Context, Result};
use jsonc_parser::cst::{CstInputValue, CstNode, CstObject, CstRootNode};
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

/// ¿El valor es el texto `NOMBRE_ESQUEMA`?
fn valor_es_nuestro(valor: Option<CstNode>) -> bool {
    valor
        .and_then(|v| v.as_string_lit())
        .and_then(|s| s.decoded_value().ok())
        .map(|n| n == NOMBRE_ESQUEMA)
        .unwrap_or(false)
}

fn es_nuestro(nodo: &CstNode) -> bool {
    nodo.as_object()
        .and_then(|o| o.get("name"))
        .map(|p| valor_es_nuestro(p.value()))
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

/// Falla si la propiedad existe pero su valor no es un objeto.
fn exigir_objeto(obj: &CstObject, nombre: &str, donde: &str) -> Result<()> {
    if let Some(p) = obj.get(nombre) {
        if p.value().and_then(|v| v.as_object()).is_none() {
            bail!("\"{nombre}\" en {donde} no es un objeto; no se modifica");
        }
    }
    Ok(())
}

pub fn aplicar_texto(texto: &str, esquema: &Value) -> Result<String> {
    let raiz = CstRootNode::parse(texto, &opciones())?;
    if let Some(v) = raiz.value() {
        if v.as_object().is_none() {
            bail!("la raíz de settings.json no es un objeto; no se modifica");
        }
    }
    let obj = raiz.object_value_or_set();
    if let Some(p) = obj.get("schemes") {
        if p.value().and_then(|v| v.as_array()).is_none() {
            bail!("\"schemes\" no es una lista; no se modifica");
        }
    }
    exigir_objeto(&obj, "profiles", "la raíz")?;
    if let Some(perfiles) = obj.object_value("profiles") {
        exigir_objeto(&perfiles, "defaults", "\"profiles\"")?;
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

/// Quita nuestro esquema y devuelve `colorScheme` a `previo`, pero sólo si sigue siendo el
/// nuestro (o falta). Si el usuario eligió otro mientras tanto, se lo respeta.
pub fn restaurar_texto(texto: &str, previo: Option<&Value>) -> Result<String> {
    let raiz = CstRootNode::parse(texto, &opciones())?;
    quitar_nuestro(&raiz);
    let defaults = raiz
        .object_value()
        .and_then(|o| o.object_value("profiles"))
        .and_then(|p| p.object_value("defaults"));
    if let Some(d) = defaults {
        match d.get("colorScheme") {
            Some(p) => {
                if valor_es_nuestro(p.value()) {
                    match previo {
                        Some(v) => p.set_value(a_cst(v)),
                        None => p.remove(),
                    }
                }
            }
            None => {
                if let Some(v) = previo {
                    d.append("colorScheme", a_cst(v));
                }
            }
        }
    }
    Ok(raiz.to_string())
}

/// Para archivos que no estaban al capturar: quita lo nuestro, sin valor previo que reponer.
pub fn limpiar_texto(texto: &str) -> Result<String> {
    restaurar_texto(texto, None)
}

/// Escribe en `<nombre>.tmp` junto al destino y lo renombra encima, así un corte a mitad de
/// escritura nunca deja el archivo truncado. Si `ruta` es un enlace simbólico se escribe sobre
/// el destino real, no se reemplaza el enlace.
fn escribir_atomico(ruta: &Path, texto: &str) -> Result<()> {
    let destino = std::fs::canonicalize(ruta).unwrap_or_else(|_| ruta.to_path_buf());
    let nombre = destino.file_name().context("ruta sin nombre de archivo")?;
    let mut nombre_tmp = nombre.to_os_string();
    nombre_tmp.push(".tmp");
    let tmp = destino.with_file_name(nombre_tmp);
    let r = std::fs::write(&tmp, texto)
        .and_then(|_| std::fs::rename(&tmp, &destino))
        .with_context(|| format!("escribir {}", ruta.display()));
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

/// Lee el archivo y devuelve el texto sin BOM y si lo tenía.
fn leer(ruta: &Path) -> Result<(String, bool)> {
    let t = std::fs::read_to_string(ruta).with_context(|| format!("leer {}", ruta.display()))?;
    let bom = t.starts_with('\u{feff}');
    Ok((t.trim_start_matches('\u{feff}').to_string(), bom))
}

/// Lee `ruta`, la transforma y la reescribe de forma atómica conservando el BOM.
/// No toca el archivo si no hay cambios.
fn transformar(ruta: &Path, f: impl FnOnce(&str) -> Result<String>) -> Result<()> {
    let (texto, bom) = leer(ruta)?;
    let nuevo = f(&texto)?;
    if nuevo == texto {
        return Ok(());
    }
    let salida = if bom { format!("\u{feff}{nuevo}") } else { nuevo };
    escribir_atomico(ruta, &salida)
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
            let (texto, _) = leer(p)?;
            let previo = esquema_defecto(&texto).with_context(|| format!("{}", p.display()))?;
            lista.push(json!({ "ruta": p, "colorScheme": previo }));
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
                transformar(p, |t| aplicar_texto(t, &esquema))
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

    /// Sigue con los demás archivos aunque uno falle, y devuelve todos los errores juntos.
    /// También limpia los archivos que aparecieron después de capturar.
    fn restaurar(&self, captura: &Value, _dir: &Path) -> Result<()> {
        let lista = captura.as_array().context("captura de terminal inválida")?;
        let mut errores = vec![];
        let mut capturadas: Vec<PathBuf> = vec![];
        for item in lista {
            let Some(ruta) = item["ruta"].as_str().map(PathBuf::from) else {
                errores.push("captura sin ruta".to_string());
                continue;
            };
            capturadas.push(ruta.clone());
            if !ruta.is_file() {
                continue;
            }
            let previo = item.get("colorScheme").filter(|v| !v.is_null());
            if let Err(e) = transformar(&ruta, |t| restaurar_texto(t, previo)) {
                errores.push(format!("{}: {e:#}", ruta.display()));
            }
        }
        for p in self.existentes() {
            if capturadas.contains(p) {
                continue;
            }
            if let Err(e) = transformar(p, limpiar_texto) {
                errores.push(format!("{}: {e:#}", p.display()));
            }
        }
        if !errores.is_empty() {
            bail!(errores.join("; "));
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

    fn tema_simple(base: &Path) -> Tema {
        let dir = base.join("tema");
        std::fs::create_dir_all(&dir).unwrap();
        let conf = json!({"nombre": "T", "terminal": {"fondo": "#000000", "texto": "#FFFFFF", "cursor": "#FFFFFF"}});
        std::fs::write(dir.join("tema.json"), conf.to_string()).unwrap();
        Tema::cargar(&dir).unwrap()
    }

    fn leer_json(ruta: &Path) -> Value {
        parsear(std::fs::read_to_string(ruta).unwrap().trim_start_matches('\u{feff}'))
    }

    #[test]
    fn restaurar_junta_errores_y_restaura_los_otros_archivos() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.json");
        let b = t.path().join("b.json");
        std::fs::write(&a, SETTINGS).unwrap();
        std::fs::write(&b, SETTINGS).unwrap();
        let m = Terminal { candidatos: vec![a.clone(), b.clone()] };
        let tema = tema_simple(t.path());

        let captura = m.capturar(t.path()).unwrap();
        m.aplicar(&tema).unwrap();
        std::fs::write(&a, "{{{ esto no es json").unwrap();

        let err = m.restaurar(&captura, t.path()).unwrap_err();
        assert!(format!("{err:#}").contains(&a.display().to_string()), "el error nombra el archivo roto: {err:#}");
        let vb = leer_json(&b);
        assert_eq!(nuestros(&vb), 0);
        assert_eq!(vb["profiles"]["defaults"]["colorScheme"], "One Half Dark");
    }

    #[test]
    fn restaurar_limpia_archivos_que_aparecieron_despues_de_capturar() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.json");
        let b = t.path().join("b.json");
        std::fs::write(&a, SETTINGS).unwrap();
        let m = Terminal { candidatos: vec![a.clone(), b.clone()] };
        let tema = tema_simple(t.path());

        let captura = m.capturar(t.path()).unwrap();
        assert_eq!(captura.as_array().unwrap().len(), 1);
        std::fs::write(&b, SETTINGS).unwrap(); // aparece después de capturar
        m.aplicar(&tema).unwrap();
        assert_eq!(leer_json(&b)["profiles"]["defaults"]["colorScheme"], NOMBRE_ESQUEMA);

        m.restaurar(&captura, t.path()).unwrap();
        let vb = leer_json(&b);
        assert_eq!(nuestros(&vb), 0);
        assert!(vb["profiles"]["defaults"].get("colorScheme").is_none());
        assert_eq!(leer_json(&a), parsear(SETTINGS));
    }

    #[test]
    fn restaurar_respeta_el_esquema_que_el_usuario_eligio_despues() {
        let aplicado = aplicar_texto(SETTINGS, &esquema_para(&conf())).unwrap();
        let usuario = aplicado.replace(&format!("\"colorScheme\": \"{NOMBRE_ESQUEMA}\""), "\"colorScheme\": \"Solarized\"");
        assert_ne!(usuario, aplicado);

        for previo in [Some(json!("One Half Dark")), None] {
            let v = parsear(&restaurar_texto(&usuario, previo.as_ref()).unwrap());
            assert_eq!(nuestros(&v), 0, "nuestro esquema se quita siempre");
            assert_eq!(v["profiles"]["defaults"]["colorScheme"], "Solarized");
        }
    }

    #[test]
    fn restaurar_repone_el_previo_si_el_defecto_desaparecio() {
        let sin = r#"{"profiles": {"defaults": {}}, "schemes": []}"#;
        let v = parsear(&restaurar_texto(sin, Some(&json!("X"))).unwrap());
        assert_eq!(v["profiles"]["defaults"]["colorScheme"], "X");
    }

    #[test]
    fn tipos_incorrectos_son_error_y_no_se_pisan() {
        let e = esquema_para(&conf());
        for malo in [
            "[]",
            "3",
            r#"{"schemes": {}}"#,
            r#"{"schemes": "x"}"#,
            r#"{"profiles": []}"#,
            r#"{"profiles": {"defaults": []}}"#,
            r#"{"profiles": {"defaults": 5}}"#,
        ] {
            assert!(aplicar_texto(malo, &e).is_err(), "debería fallar con {malo}");
        }
        let msg = format!("{:#}", aplicar_texto(r#"{"profiles": []}"#, &e).unwrap_err());
        assert!(msg.contains("profiles") && msg.contains("objeto"), "{msg}");
    }

    #[test]
    fn esquemas_duplicados_se_quitan_todos() {
        let dup = r#"{"schemes": [{"name": "Tema activo"}, {"name": "Mio"}, {"name": "Tema activo"}]}"#;
        let v = parsear(&restaurar_texto(dup, None).unwrap());
        assert_eq!(nuestros(&v), 0);
        assert_eq!(v["schemes"].as_array().unwrap().len(), 1);
        let v = parsear(&aplicar_texto(dup, &esquema_para(&conf())).unwrap());
        assert_eq!(nuestros(&v), 1);
        assert_eq!(v["schemes"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn esquema_defecto_en_forma_de_objeto_hace_ida_y_vuelta() {
        let original = r#"{
    "profiles": { "defaults": { "colorScheme": { "dark": "Campbell", "light": "One Half Light" } } },
    "schemes": [],
}"#;
        let t = tempfile::tempdir().unwrap();
        let archivo = t.path().join("settings.json");
        std::fs::write(&archivo, original).unwrap();
        let m = Terminal { candidatos: vec![archivo.clone()] };
        let tema = tema_simple(t.path());

        let captura = m.capturar(t.path()).unwrap();
        m.aplicar(&tema).unwrap();
        assert_eq!(leer_json(&archivo)["profiles"]["defaults"]["colorScheme"], NOMBRE_ESQUEMA);
        m.restaurar(&captura, t.path()).unwrap();
        assert_eq!(leer_json(&archivo), parsear(original));
    }

    #[test]
    fn archivo_con_bom_conserva_el_bom() {
        let t = tempfile::tempdir().unwrap();
        let archivo = t.path().join("settings.json");
        std::fs::write(&archivo, format!("\u{feff}{SETTINGS}")).unwrap();
        let m = Terminal { candidatos: vec![archivo.clone()] };
        let tema = tema_simple(t.path());

        let captura = m.capturar(t.path()).unwrap();
        m.aplicar(&tema).unwrap();
        assert!(std::fs::read_to_string(&archivo).unwrap().starts_with('\u{feff}'));
        m.restaurar(&captura, t.path()).unwrap();
        let crudo = std::fs::read_to_string(&archivo).unwrap();
        assert!(crudo.starts_with('\u{feff}'));
        assert!(!crudo.starts_with("\u{feff}\u{feff}"));
        assert_eq!(leer_json(&archivo), parsear(SETTINGS));
    }

    #[test]
    fn un_archivo_sin_bom_no_gana_bom() {
        let t = tempfile::tempdir().unwrap();
        let archivo = t.path().join("settings.json");
        std::fs::write(&archivo, SETTINGS).unwrap();
        let m = Terminal { candidatos: vec![archivo.clone()] };
        m.aplicar(&tema_simple(t.path())).unwrap();
        assert!(!std::fs::read_to_string(&archivo).unwrap().starts_with('\u{feff}'));
    }

    #[test]
    fn el_bak_se_crea_una_vez_y_no_se_pisa() {
        let t = tempfile::tempdir().unwrap();
        let archivo = t.path().join("settings.json");
        std::fs::write(&archivo, SETTINGS).unwrap();
        let m = Terminal { candidatos: vec![archivo.clone()] };
        let tema = tema_simple(t.path());

        m.aplicar(&tema).unwrap();
        m.aplicar(&tema).unwrap();
        assert_eq!(std::fs::read_to_string(t.path().join("settings.json.bak")).unwrap(), SETTINGS);
    }

    #[test]
    fn escribir_atomico_reemplaza_y_no_deja_temporales() {
        let t = tempfile::tempdir().unwrap();
        let archivo = t.path().join("settings.json");
        std::fs::write(&archivo, "viejo").unwrap();
        escribir_atomico(&archivo, "nuevo").unwrap();
        assert_eq!(std::fs::read_to_string(&archivo).unwrap(), "nuevo");
        let nombres: Vec<_> = std::fs::read_dir(t.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(nombres.len(), 1, "sólo queda el archivo final: {nombres:?}");
    }
}
