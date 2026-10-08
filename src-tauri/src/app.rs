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
use std::sync::{Arc, Mutex};

pub struct App {
    pub motor: Motor,
    pub rotador: Arc<Rotador>,
    pub ajustes: Ajustes,
    /// Evita dos aplicaciones/restauraciones al mismo tiempo.
    ocupado: Mutex<()>,
}

impl App {
    pub fn nuevo() -> Result<App> {
        let rutas = Rutas::por_defecto();
        let ajustes = Ajustes::cargar(&rutas)?;
        let rotador = Arc::new(Rotador::nuevo(win::poner_fondo));
        let modulos: Vec<Box<dyn Modulo>> = vec![
            Box::new(Fondos { rotador: rotador.clone(), raiz: rutas.raiz.clone() }),
            Box::new(Colores),
            Box::new(Bloqueo),
            Box::new(Terminal::por_defecto()),
        ];
        Ok(App { motor: Motor::nuevo(rutas, modulos), rotador, ajustes, ocupado: Mutex::new(()) })
    }

    pub fn temas(&self) -> Vec<(String, Result<Tema>)> {
        listar_temas(&self.ajustes.carpeta_temas).unwrap_or_default()
    }

    pub fn estado(&self) -> Estado {
        Estado::cargar(&self.motor.rutas).unwrap_or_default()
    }

    pub fn nombre_activo(&self) -> Option<String> {
        let id = self.estado().tema_activo?;
        Some(
            Tema::cargar(&self.ajustes.carpeta_temas.join(&id))
                .map(|t| t.conf.nombre)
                .unwrap_or(id),
        )
    }

    pub fn aplicar(&self, id: &str) -> Result<Informe> {
        let _g = self.ocupado.lock().unwrap();
        let tema = Tema::cargar(&self.ajustes.carpeta_temas.join(id))?;
        let inf = self.motor.aplicar(&tema, &[])?;
        self.guardar_pausa(false);
        Ok(inf)
    }

    pub fn restaurar(&self) -> Result<Informe> {
        let _g = self.ocupado.lock().unwrap();
        self.motor.restaurar_original()
    }

    pub fn error_de(&self, id: &str) -> String {
        match Tema::cargar(&self.ajustes.carpeta_temas.join(id)) {
            Ok(_) => "el tema ahora es válido".into(),
            Err(e) => format!("{e:#}"),
        }
    }

    pub fn alternar_pausa(&self) {
        let pausar = !self.rotador.pausado();
        if pausar {
            self.rotador.pausar();
        } else {
            self.rotador.reanudar();
        }
        self.guardar_pausa(pausar);
    }

    fn guardar_pausa(&self, pausado: bool) {
        let mut e = self.estado();
        e.fondos_pausados = pausado;
        let _ = e.guardar(&self.motor.rutas);
    }

    /// Al arrancar: retoma la rotación del tema activo sin reaplicar lo demás.
    pub fn retomar(&self) -> Result<()> {
        let e = self.estado();
        let Some(id) = e.tema_activo else { return Ok(()) };
        if !e.tocados.iter().any(|t| t == "fondos") {
            return Ok(());
        }
        let tema = Tema::cargar(&self.ajustes.carpeta_temas.join(&id))?;
        let (lista, intervalo) = fondos::lista_de(&tema)?;
        self.rotador.iniciar(lista, intervalo);
        if e.fondos_pausados {
            self.rotador.pausar();
        }
        Ok(())
    }
}
