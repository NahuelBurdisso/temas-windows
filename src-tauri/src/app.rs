//! Estado de la app y acciones que dispara la bandeja.

use anyhow::Result;
use motor::estado::{Ajustes, Estado, Rutas};
use motor::modulos::bloqueo::Bloqueo;
use motor::modulos::colores::Colores;
use motor::modulos::fondos::{self, Fondos};
use motor::modulos::terminal::Terminal;
use motor::modulos::Modulo;
use motor::nucleo::{Informe, Motor};
use motor::rotador::Rotador;
use motor::tema::{listar_temas, Tema};
use motor::win;
use std::sync::{Arc, Mutex, MutexGuard};

pub struct App {
    pub motor: Motor,
    pub rotador: Arc<Rotador>,
    pub ajustes: Ajustes,
    /// Evita dos aplicaciones/restauraciones al mismo tiempo.
    ocupado: Mutex<()>,
}

impl App {
    /// Nunca falla: si `ajustes.json` no se puede leer usa los ajustes por defecto y lo avisa
    /// en la lista de advertencias de arranque.
    pub fn nuevo() -> (App, Vec<String>) {
        let rutas = Rutas::por_defecto();
        let mut avisos = vec![];
        let ajustes = match Ajustes::cargar(&rutas) {
            Ok(a) => a,
            Err(e) => {
                avisos.push(format!(
                    "no se pudo leer ajustes.json; se usan los ajustes por defecto ({}): {e:#}",
                    rutas.ajustes().display()
                ));
                Ajustes::default()
            }
        };
        let rotador = Arc::new(Rotador::nuevo(win::poner_fondo));
        let modulos: Vec<Box<dyn Modulo>> = vec![
            Box::new(Fondos { rotador: rotador.clone(), raiz: rutas.raiz.clone() }),
            Box::new(Colores),
            Box::new(Bloqueo),
            Box::new(Terminal::por_defecto()),
        ];
        let app = App { motor: Motor::nuevo(rutas, modulos), rotador, ajustes, ocupado: Mutex::new(()) };
        (app, avisos)
    }

    /// Toma el candado de operaciones (tolera que haya quedado envenenado). Orden de locks:
    /// siempre `ocupado` primero y después el rotador.
    pub fn bloquear(&self) -> MutexGuard<'_, ()> {
        self.ocupado.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn temas(&self) -> Vec<(String, Result<Tema>)> {
        listar_temas(&self.ajustes.carpeta_temas).unwrap_or_default()
    }

    pub fn estado(&self) -> Estado {
        Estado::cargar(&self.motor.rutas).unwrap_or_default()
    }

    pub fn aplicar(&self, id: &str) -> Result<Informe> {
        let _g = self.bloquear();
        let tema = Tema::cargar(&self.ajustes.carpeta_temas.join(id))?;
        let mut inf = self.motor.aplicar(&tema, &[])?;
        if let Err(e) = self.guardar_pausa(false) {
            inf.advertencias.push(format!("no se pudo guardar el estado de pausa: {e:#}"));
        }
        Ok(inf)
    }

    pub fn restaurar(&self) -> Result<Informe> {
        let _g = self.bloquear();
        self.motor.restaurar_original()
    }

    pub fn error_de(&self, id: &str) -> String {
        match Tema::cargar(&self.ajustes.carpeta_temas.join(id)) {
            Ok(_) => "el tema ahora es válido".into(),
            Err(e) => format!("{e:#}"),
        }
    }

    pub fn alternar_pausa(&self) -> Result<()> {
        let _g = self.bloquear();
        // Se lee el estado antes de tocar el rotador: si no se puede leer, no se cambia nada.
        let estado = Estado::cargar(&self.motor.rutas)?;
        let pausar = !self.rotador.pausado();
        if pausar {
            self.rotador.pausar();
        } else {
            self.rotador.reanudar();
        }
        self.escribir_pausa(estado, pausar)
    }

    /// Supone que `ocupado` está tomado. No escribe nada si no puede leer `estado.json`.
    fn guardar_pausa(&self, pausado: bool) -> Result<()> {
        let estado = Estado::cargar(&self.motor.rutas)?;
        self.escribir_pausa(estado, pausado)
    }

    fn escribir_pausa(&self, mut estado: Estado, pausado: bool) -> Result<()> {
        estado.fondos_pausados = pausado;
        estado.guardar(&self.motor.rutas)
    }

    /// Al arrancar: retoma la rotación del tema activo sin reaplicar lo demás.
    pub fn retomar(&self) -> Result<()> {
        let e = Estado::cargar(&self.motor.rutas)?;
        let Some(id) = e.tema_activo else { return Ok(()) };
        if !e.tocados.iter().any(|t| t == "fondos") {
            return Ok(());
        }
        let tema = Tema::cargar(&self.ajustes.carpeta_temas.join(&id))?;
        let (lista, intervalo) = fondos::lista_de(&tema)?;
        if e.fondos_pausados {
            self.rotador.iniciar_en_pausa(lista, intervalo);
        } else {
            self.rotador.iniciar(lista, intervalo);
        }
        Ok(())
    }
}
