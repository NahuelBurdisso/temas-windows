//! Hilo que cambia el fondo cada cierto intervalo.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

type Poner = dyn Fn(&Path) -> Result<()> + Send + Sync;

#[derive(Default)]
struct Estado {
    lista: Vec<PathBuf>,
    indice: usize,
    intervalo: Duration,
    pausado: bool,
    saltar: bool,
    /// Cambia en cada `iniciar`/`detener`; el hilo viejo lo ve y termina.
    generacion: u64,
}

pub struct Rotador {
    inner: Arc<(Mutex<Estado>, Condvar)>,
    poner: Arc<Poner>,
}

impl Rotador {
    pub fn nuevo(poner: impl Fn(&Path) -> Result<()> + Send + Sync + 'static) -> Rotador {
        Rotador { inner: Default::default(), poner: Arc::new(poner) }
    }

    pub fn iniciar(&self, lista: Vec<PathBuf>, intervalo: Duration) {
        let (m, cv) = &*self.inner;
        let vacia = lista.is_empty();
        let gen = {
            let mut e = m.lock().unwrap();
            e.generacion += 1;
            e.lista = lista;
            e.indice = 0;
            e.intervalo = intervalo;
            e.pausado = false;
            e.saltar = false;
            e.generacion
        };
        cv.notify_all();
        if vacia {
            return;
        }
        let inner = self.inner.clone();
        let poner = self.poner.clone();
        thread::spawn(move || bucle(inner, poner, gen));
    }

    pub fn siguiente(&self) {
        self.cambiar(|e| e.saltar = true);
    }
    pub fn pausar(&self) {
        self.cambiar(|e| e.pausado = true);
    }
    pub fn reanudar(&self) {
        self.cambiar(|e| e.pausado = false);
    }
    pub fn detener(&self) {
        self.cambiar(|e| {
            e.generacion += 1;
            e.lista.clear();
        });
    }
    pub fn activo(&self) -> bool {
        !self.inner.0.lock().unwrap().lista.is_empty()
    }
    pub fn pausado(&self) -> bool {
        self.inner.0.lock().unwrap().pausado
    }

    fn cambiar(&self, f: impl FnOnce(&mut Estado)) {
        let (m, cv) = &*self.inner;
        f(&mut m.lock().unwrap());
        cv.notify_all();
    }
}

fn bucle(inner: Arc<(Mutex<Estado>, Condvar)>, poner: Arc<Poner>, gen: u64) {
    let (m, cv) = &*inner;
    loop {
        let ruta = {
            let e = m.lock().unwrap();
            if e.generacion != gen {
                return;
            }
            e.lista[e.indice].clone()
        };
        if let Err(err) = poner(&ruta) {
            eprintln!("no se pudo poner {}: {err:#}", ruta.display());
        }
        let mut e = m.lock().unwrap();
        let mut limite = Instant::now() + e.intervalo;
        loop {
            if e.generacion != gen {
                return;
            }
            if e.saltar {
                e.saltar = false;
                break;
            }
            if e.pausado {
                e = cv.wait(e).unwrap();
                limite = Instant::now() + e.intervalo;
                continue;
            }
            let ahora = Instant::now();
            if ahora >= limite {
                break;
            }
            e = cv.wait_timeout(e, limite - ahora).unwrap().0;
        }
        e.indice = (e.indice + 1) % e.lista.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    type Log = Arc<Mutex<Vec<PathBuf>>>;

    fn grabador() -> (Log, Rotador) {
        let log: Log = Default::default();
        let l = log.clone();
        let r = Rotador::nuevo(move |p: &Path| {
            l.lock().unwrap().push(p.to_path_buf());
            Ok(())
        });
        (log, r)
    }

    fn lista(prefijo: &str, n: usize) -> Vec<PathBuf> {
        (0..n).map(|i| PathBuf::from(format!("{prefijo}{i}.jpg"))).collect()
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn rota_en_orden_y_vuelve_al_principio() {
        let (log, r) = grabador();
        r.iniciar(lista("a", 3), ms(40));
        sleep(ms(150));
        r.detener();
        let l = log.lock().unwrap().clone();
        assert!(l.len() >= 4, "esperaba ≥ 4 cambios, hubo {}", l.len());
        assert_eq!(&l[..4], &[lista("a", 3), lista("a", 1)].concat()[..]);
    }

    #[test]
    fn siguiente_avanza_en_el_momento() {
        let (log, r) = grabador();
        r.iniciar(lista("a", 3), Duration::from_secs(10));
        sleep(ms(30));
        r.siguiente();
        sleep(ms(30));
        assert_eq!(*log.lock().unwrap(), lista("a", 2));
        r.detener();
    }

    #[test]
    fn pausar_frena_y_reanudar_sigue() {
        let (log, r) = grabador();
        r.iniciar(lista("a", 3), ms(30));
        sleep(ms(10));
        r.pausar();
        assert!(r.pausado());
        let n = log.lock().unwrap().len();
        sleep(ms(100));
        assert_eq!(log.lock().unwrap().len(), n, "no cambia mientras está en pausa");
        r.reanudar();
        sleep(ms(80));
        assert!(log.lock().unwrap().len() > n);
        r.detener();
    }

    #[test]
    fn detener_frena_y_queda_inactivo() {
        let (log, r) = grabador();
        r.iniciar(lista("a", 3), ms(20));
        sleep(ms(10));
        r.detener();
        assert!(!r.activo());
        let n = log.lock().unwrap().len();
        sleep(ms(80));
        assert_eq!(log.lock().unwrap().len(), n);
    }

    #[test]
    fn iniciar_de_nuevo_reemplaza_la_rotacion_anterior() {
        let (log, r) = grabador();
        r.iniciar(lista("a", 3), Duration::from_secs(10));
        sleep(ms(20));
        r.iniciar(lista("b", 3), Duration::from_secs(10));
        sleep(ms(40));
        assert_eq!(*log.lock().unwrap(), [lista("a", 1), lista("b", 1)].concat());
        assert!(r.activo());
        r.detener();
    }

    #[test]
    fn lista_vacia_no_arranca() {
        let (log, r) = grabador();
        r.iniciar(vec![], ms(10));
        sleep(ms(30));
        assert!(log.lock().unwrap().is_empty());
        assert!(!r.activo());
    }
}
