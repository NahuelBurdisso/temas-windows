//! Orquestación: aplicar un tema y restaurar el original.

use crate::estado::{archivar_original, Estado, Original, Rutas};
use crate::modulos::Modulo;
use crate::tema::Tema;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Informe {
    pub ok: Vec<String>,
    pub errores: Vec<(String, String)>,
    pub advertencias: Vec<String>,
}

impl Informe {
    pub fn resumen(&self) -> String {
        let mut s = String::new();
        for (id, e) in &self.errores {
            s += &format!("✗ {id}: {e}\n");
        }
        for a in &self.advertencias {
            s += &format!("⚠ {a}\n");
        }
        if !self.ok.is_empty() {
            s += &format!("✓ {}\n", self.ok.join(", "));
        }
        s
    }
}

pub struct Motor {
    pub rutas: Rutas,
    modulos: Vec<Box<dyn Modulo>>,
}

impl Motor {
    pub fn nuevo(rutas: Rutas, modulos: Vec<Box<dyn Modulo>>) -> Motor {
        Motor { rutas, modulos }
    }

    fn modulo(&self, id: &str) -> Option<&dyn Modulo> {
        self.modulos.iter().find(|m| m.id() == id).map(|b| b.as_ref())
    }

    fn dir(&self, id: &str) -> PathBuf {
        self.rutas.dir_original(id)
    }

    pub fn aplicar(&self, tema: &Tema, desmarcados: &[&str]) -> Result<Informe> {
        let mut inf = Informe { advertencias: tema.advertencias.clone(), ..Default::default() };
        let mut original = Original::cargar(&self.rutas)?;
        let mut estado = Estado::cargar(&self.rutas)?;

        // 1. Qué se aplica, capturando el original de lo que todavía no lo tiene.
        let mut nuevos: Vec<&dyn Modulo> = vec![];
        for m in &self.modulos {
            let id = m.id();
            if !tema.define(id) || desmarcados.contains(&id) {
                continue;
            }
            if original.get(id).is_none() {
                let dir = self.dir(id);
                let captura = std::fs::create_dir_all(&dir)
                    .map_err(anyhow::Error::from)
                    .and_then(|_| m.capturar(&dir));
                match captura {
                    Ok(v) => {
                        original.poner(id, v);
                        original.guardar(&self.rutas)?;
                    }
                    Err(e) => {
                        inf.errores.push((id.into(), format!("no se pudo guardar el estado original: {e:#}")));
                        continue;
                    }
                }
            }
            nuevos.push(m.as_ref());
        }
        let ids_nuevos: Vec<String> = nuevos.iter().map(|m| m.id().to_string()).collect();

        // 2. Lo que el tema anterior tocó y este no aplica vuelve al original.
        let mut pendientes = vec![];
        for id in estado.tocados.iter().filter(|id| !ids_nuevos.contains(id)) {
            if let (Some(m), Some(c)) = (self.modulo(id), original.get(id)) {
                if let Err(e) = m.restaurar(c, &self.dir(id)) {
                    inf.errores.push((id.clone(), format!("no se pudo restaurar: {e:#}")));
                    pendientes.push(id.clone());
                }
            }
        }

        // 3. Aplicar.
        for m in &nuevos {
            match m.aplicar(tema) {
                Ok(()) => inf.ok.push(m.id().into()),
                Err(e) => inf.errores.push((m.id().into(), format!("{e:#}"))),
            }
        }

        estado.tema_activo = Some(tema.id.clone());
        estado.tocados = ids_nuevos;
        estado.tocados.extend(pendientes);
        estado.guardar(&self.rutas)?;
        Ok(inf)
    }

    pub fn restaurar_original(&self) -> Result<Informe> {
        let original = Original::cargar(&self.rutas)?;
        let mut inf = Informe::default();
        for (id, c) in &original.0 {
            let Some(m) = self.modulo(id) else { continue };
            match m.restaurar(c, &self.dir(id)) {
                Ok(()) => inf.ok.push(id.clone()),
                Err(e) => inf.errores.push((id.clone(), format!("{e:#}"))),
            }
        }
        archivar_original(&self.rutas, &inf.ok)?;
        Estado::default().guardar(&self.rutas)?;
        Ok(inf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estado::{leer_json, Estado, Original};
    use serde_json::{json, Value};
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    type Log = Arc<Mutex<Vec<String>>>;

    #[derive(Default, Clone, Copy)]
    struct Fallas {
        capturar: bool,
        aplicar: bool,
        restaurar: bool,
    }

    struct Falso {
        id: &'static str,
        log: Log,
        fallas: Fallas,
    }

    impl Modulo for Falso {
        fn id(&self) -> &'static str {
            self.id
        }
        fn capturar(&self, dir: &Path) -> Result<Value> {
            assert!(dir.is_dir(), "la carpeta del módulo existe al capturar");
            self.log.lock().unwrap().push(format!("capturar:{}", self.id));
            if self.fallas.capturar { anyhow::bail!("falla capturar") }
            Ok(json!({ "de": self.id }))
        }
        fn aplicar(&self, tema: &Tema) -> Result<()> {
            self.log.lock().unwrap().push(format!("aplicar:{}:{}", self.id, tema.id));
            if self.fallas.aplicar { anyhow::bail!("falla aplicar") }
            Ok(())
        }
        fn restaurar(&self, captura: &Value, _dir: &Path) -> Result<()> {
            assert_eq!(captura, &json!({ "de": self.id }));
            self.log.lock().unwrap().push(format!("restaurar:{}", self.id));
            if self.fallas.restaurar { anyhow::bail!("falla restaurar") }
            Ok(())
        }
    }

    struct Banco {
        _tmp: tempfile::TempDir,
        temas: PathBuf,
        log: Log,
        motor: Motor,
    }

    fn banco(fallas: &[(&'static str, Fallas)]) -> Banco {
        let tmp = tempfile::tempdir().unwrap();
        let log: Log = Default::default();
        let falla = |id| fallas.iter().find(|(i, _)| *i == id).map(|(_, f)| *f).unwrap_or_default();
        let modulos: Vec<Box<dyn Modulo>> = ["colores", "terminal", "bloqueo"]
            .into_iter()
            .map(|id| Box::new(Falso { id, log: log.clone(), fallas: falla(id) }) as Box<dyn Modulo>)
            .collect();
        let motor = Motor::nuevo(Rutas::nuevo(tmp.path().join("app")), modulos);
        Banco { temas: tmp.path().join("temas"), _tmp: tmp, log, motor }
    }

    impl Banco {
        /// Crea un tema con las secciones pedidas ("colores", "terminal", "bloqueo").
        fn tema(&self, id: &str, secciones: &[&str]) -> Tema {
            let dir = self.temas.join(id);
            std::fs::create_dir_all(&dir).unwrap();
            let mut conf = json!({ "nombre": id });
            for s in secciones {
                match *s {
                    "colores" => conf["colores"] = json!({"acento": "#C9A227", "modo": "oscuro"}),
                    "terminal" => conf["terminal"] = json!({"fondo": "#000000", "texto": "#FFFFFF", "cursor": "#FFFFFF"}),
                    "bloqueo" => {
                        std::fs::write(dir.join("b.jpg"), b"x").unwrap();
                        conf["bloqueo"] = json!("b.jpg");
                    }
                    otro => panic!("sección desconocida {otro}"),
                }
            }
            std::fs::write(dir.join("tema.json"), conf.to_string()).unwrap();
            Tema::cargar(&dir).unwrap()
        }
        fn log(&self) -> Vec<String> {
            std::mem::take(&mut *self.log.lock().unwrap())
        }
        fn estado(&self) -> Estado {
            Estado::cargar(&self.motor.rutas).unwrap()
        }
        fn original(&self) -> Original {
            Original::cargar(&self.motor.rutas).unwrap()
        }
    }

    #[test]
    fn primera_aplicacion_captura_y_aplica() {
        let b = banco(&[]);
        let a = b.tema("a", &["colores", "terminal"]);
        let inf = b.motor.aplicar(&a, &[]).unwrap();
        assert_eq!(b.log(), ["capturar:colores", "capturar:terminal", "aplicar:colores:a", "aplicar:terminal:a"]);
        assert_eq!(inf.ok, ["colores", "terminal"]);
        assert!(inf.errores.is_empty());
        assert!(b.original().get("colores").is_some() && b.original().get("terminal").is_some());
        assert_eq!(b.estado().tema_activo.as_deref(), Some("a"));
        assert_eq!(b.estado().tocados, ["colores", "terminal"]);
    }

    #[test]
    fn no_recaptura_lo_ya_capturado() {
        let b = banco(&[]);
        let a = b.tema("a", &["colores"]);
        b.motor.aplicar(&a, &[]).unwrap();
        b.log();
        b.motor.aplicar(&a, &[]).unwrap();
        assert_eq!(b.log(), ["aplicar:colores:a"]);
    }

    #[test]
    fn al_cambiar_restaura_lo_que_el_nuevo_no_aplica() {
        let b = banco(&[]);
        b.motor.aplicar(&b.tema("a", &["colores", "terminal"]), &[]).unwrap();
        b.log();
        b.motor.aplicar(&b.tema("b", &["colores"]), &[]).unwrap();
        assert_eq!(b.log(), ["restaurar:terminal", "aplicar:colores:b"]);
        assert_eq!(b.estado().tocados, ["colores"]);
    }

    #[test]
    fn un_modulo_nunca_tocado_no_se_restaura() {
        let b = banco(&[]);
        b.motor.aplicar(&b.tema("a", &["colores"]), &[]).unwrap();
        b.log();
        b.motor.aplicar(&b.tema("b", &["colores"]), &[]).unwrap();
        assert!(!b.log().iter().any(|l| l.starts_with("restaurar")));
    }

    #[test]
    fn desmarcado_cuenta_como_no_aplicado() {
        let b = banco(&[]);
        let a = b.tema("a", &["colores", "terminal"]);
        b.motor.aplicar(&a, &[]).unwrap();
        b.log();
        b.motor.aplicar(&a, &["terminal"]).unwrap();
        assert_eq!(b.log(), ["restaurar:terminal", "aplicar:colores:a"]);
    }

    #[test]
    fn falla_al_aplicar_no_frena_a_los_demas_y_queda_tocado() {
        let b = banco(&[("colores", Fallas { aplicar: true, ..Default::default() })]);
        let inf = b.motor.aplicar(&b.tema("a", &["colores", "terminal"]), &[]).unwrap();
        assert_eq!(inf.ok, ["terminal"]);
        assert_eq!(inf.errores.len(), 1);
        assert_eq!(inf.errores[0].0, "colores");
        assert_eq!(b.estado().tocados, ["colores", "terminal"]);
    }

    #[test]
    fn falla_al_capturar_no_aplica_ese_modulo() {
        let b = banco(&[("colores", Fallas { capturar: true, ..Default::default() })]);
        let inf = b.motor.aplicar(&b.tema("a", &["colores", "terminal"]), &[]).unwrap();
        assert!(!b.log().contains(&"aplicar:colores:a".to_string()));
        assert_eq!(inf.errores[0].0, "colores");
        assert_eq!(b.estado().tocados, ["terminal"]);
        assert!(b.original().get("colores").is_none());
    }

    #[test]
    fn falla_al_restaurar_en_un_cambio_queda_pendiente() {
        let b = banco(&[("terminal", Fallas { restaurar: true, ..Default::default() })]);
        b.motor.aplicar(&b.tema("a", &["colores", "terminal"]), &[]).unwrap();
        let inf = b.motor.aplicar(&b.tema("b", &["colores"]), &[]).unwrap();
        assert_eq!(inf.errores[0].0, "terminal");
        assert_eq!(b.estado().tocados, ["colores", "terminal"]);
    }

    #[test]
    fn restaurar_original_restaura_archiva_y_limpia() {
        let b = banco(&[]);
        b.motor.aplicar(&b.tema("a", &["colores", "terminal"]), &[]).unwrap();
        b.log();
        let inf = b.motor.restaurar_original().unwrap();
        assert_eq!(b.log(), ["restaurar:colores", "restaurar:terminal"]);
        assert_eq!(inf.ok, ["colores", "terminal"]);
        assert!(b.original().0.is_empty());
        let prev: Original = leer_json(&b.motor.rutas.original_prev()).unwrap();
        assert_eq!(prev.0.len(), 2);
        assert_eq!(b.estado(), Estado::default());
    }

    #[test]
    fn restaurar_original_con_falla_conserva_esa_captura() {
        let b = banco(&[("terminal", Fallas { restaurar: true, ..Default::default() })]);
        b.motor.aplicar(&b.tema("a", &["colores", "terminal"]), &[]).unwrap();
        let inf = b.motor.restaurar_original().unwrap();
        assert_eq!(inf.ok, ["colores"]);
        assert!(b.original().get("terminal").is_some());
        assert!(b.original().get("colores").is_none());
    }

    #[test]
    fn despues_de_restaurar_se_vuelve_a_capturar() {
        let b = banco(&[]);
        let a = b.tema("a", &["colores"]);
        b.motor.aplicar(&a, &[]).unwrap();
        b.motor.restaurar_original().unwrap();
        b.log();
        b.motor.aplicar(&a, &[]).unwrap();
        assert_eq!(b.log(), ["capturar:colores", "aplicar:colores:a"]);
    }
}
