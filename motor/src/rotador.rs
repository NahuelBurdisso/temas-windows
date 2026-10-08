//! Hilo que cambia el fondo cada cierto intervalo.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

type Poner = dyn Fn(&Path) -> Result<()> + Send + Sync;
type Compartido = Arc<(Mutex<Estado>, Condvar)>;

/// Tope de espera si el intervalo es tan grande que `Instant` se desborda.
const UN_DIA: Duration = Duration::from_secs(86_400);

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

/// Rota los fondos en un hilo propio.
///
/// Orden de locks: siempre `poniendo` primero y después el mutex de estado, nunca al revés.
/// `poniendo` se sostiene únicamente durante la llamada a `poner` (y su chequeo previo de
/// generación), jamás mientras se espera en la condvar. Por eso `detener`/`pausar` pueden
/// esperar a que termine un `poner` en curso. Consecuencia: `poner` no debe llamar a
/// `detener`/`pausar` del mismo rotador (se bloquearía a sí mismo).
pub struct Rotador {
    inner: Compartido,
    poner: Arc<Poner>,
    /// Se sostiene mientras corre `poner`.
    poniendo: Arc<Mutex<()>>,
}

fn tomar<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn limite_desde(intervalo: Duration) -> Instant {
    let ahora = Instant::now();
    ahora.checked_add(intervalo).unwrap_or_else(|| ahora + UN_DIA)
}

impl Rotador {
    pub fn nuevo(poner: impl Fn(&Path) -> Result<()> + Send + Sync + 'static) -> Rotador {
        Rotador { inner: Default::default(), poner: Arc::new(poner), poniendo: Default::default() }
    }

    pub fn iniciar(&self, lista: Vec<PathBuf>, intervalo: Duration) {
        self.arrancar(lista, intervalo, false);
    }

    /// Igual que `iniciar`, pero arranca en pausa: no se pone ninguna imagen hasta `reanudar`
    /// (un `siguiente` sí pone una, y se queda en pausa).
    pub fn iniciar_en_pausa(&self, lista: Vec<PathBuf>, intervalo: Duration) {
        self.arrancar(lista, intervalo, true);
    }

    fn arrancar(&self, lista: Vec<PathBuf>, intervalo: Duration, pausado: bool) {
        let (m, cv) = &*self.inner;
        let vacia = lista.is_empty();
        let gen = {
            let mut e = m.lock().unwrap();
            e.generacion += 1;
            e.lista = lista;
            e.indice = 0;
            e.intervalo = intervalo;
            e.pausado = pausado;
            e.saltar = false;
            e.generacion
        };
        cv.notify_all();
        if vacia {
            return;
        }
        let inner = self.inner.clone();
        let poner = self.poner.clone();
        let poniendo = self.poniendo.clone();
        // El hilo nuevo espera en `poniendo` a que termine el `poner` del viejo: no se pisan.
        thread::spawn(move || bucle(inner, poner, poniendo, gen));
    }

    pub fn siguiente(&self) {
        self.cambiar(|e| e.saltar = true);
    }
    /// Al volver, ningún `poner` está en curso ni va a empezar hasta `reanudar`.
    pub fn pausar(&self) {
        self.cambiar(|e| e.pausado = true);
        self.esperar_poner();
    }
    pub fn reanudar(&self) {
        self.cambiar(|e| e.pausado = false);
    }
    /// Al volver, ningún `poner` está en curso ni va a empezar.
    pub fn detener(&self) {
        self.cambiar(|e| {
            e.generacion += 1;
            e.lista.clear();
        });
        self.esperar_poner();
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

    /// Espera a que termine el `poner` en curso (si hay). El hilo ya no empieza otro:
    /// lo impide el estado que `cambiar` dejó antes de llamar acá.
    fn esperar_poner(&self) {
        drop(tomar(&self.poniendo));
    }
}

fn bucle(inner: Compartido, poner: Arc<Poner>, poniendo: Arc<Mutex<()>>, gen: u64) {
    let (m, cv) = &*inner;
    // Tras `siguiente`, se pone la imagen aunque esté en pausa.
    let mut forzar = false;
    loop {
        let puesto = {
            let _p = tomar(&poniendo);
            let e = m.lock().unwrap();
            if e.generacion != gen {
                return;
            }
            if e.pausado && !forzar {
                false
            } else {
                let ruta = e.lista[e.indice].clone();
                drop(e);
                if let Err(err) = poner(&ruta) {
                    eprintln!("no se pudo poner {}: {err:#}", ruta.display());
                }
                true
            }
        };
        forzar = false;
        let mut e = m.lock().unwrap();
        if !puesto {
            // En pausa antes de poner: esperar a reanudar (o a que nos reemplacen) y reintentar.
            while e.generacion == gen && e.pausado {
                if e.saltar {
                    // `siguiente` en pausa: poner ya la imagen que toca.
                    e.saltar = false;
                    forzar = true;
                    break;
                }
                e = cv.wait(e).unwrap();
            }
            continue;
        }
        let mut limite = limite_desde(e.intervalo);
        loop {
            if e.generacion != gen {
                return;
            }
            if e.saltar {
                e.saltar = false;
                forzar = true;
                break;
            }
            if e.pausado {
                e = cv.wait(e).unwrap();
                limite = limite_desde(e.intervalo);
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
    use std::sync::mpsc::{channel, Receiver};
    use std::thread::sleep;

    /// Margen generoso para esperar un cambio que SÍ tiene que ocurrir.
    const ESPERA: Duration = Duration::from_secs(2);

    /// Rotador cuyo `poner` manda cada ruta por un canal en vez de tocar Windows.
    fn grabador() -> (Receiver<PathBuf>, Rotador) {
        let (rx, _empezo, r) = grabador_lento(Duration::ZERO);
        (rx, r)
    }

    /// Igual, pero `poner` tarda `demora` y recién después avisa (simula un `poner` en curso).
    /// El segundo receptor avisa cuando un `poner` ARRANCÓ (antes de la demora): sirve de
    /// handshake para saber, sin adivinar con `sleep`, que hay uno en vuelo.
    fn grabador_lento(demora: Duration) -> (Receiver<PathBuf>, Receiver<()>, Rotador) {
        let (tx, rx) = channel();
        let (tx_empezo, rx_empezo) = channel();
        let tx = Mutex::new(tx);
        let tx_empezo = Mutex::new(tx_empezo);
        let r = Rotador::nuevo(move |p: &Path| {
            let _ = tx_empezo.lock().unwrap().send(()); // si nadie escucha el handshake, da igual
            sleep(demora);
            tx.lock().unwrap().send(p.to_path_buf()).unwrap();
            Ok(())
        });
        (rx, rx_empezo, r)
    }

    fn lista(prefijo: &str, n: usize) -> Vec<PathBuf> {
        (0..n).map(|i| PathBuf::from(format!("{prefijo}{i}.jpg"))).collect()
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn esperar(rx: &Receiver<PathBuf>) -> PathBuf {
        rx.recv_timeout(ESPERA).expect("el cambio de fondo no llegó a tiempo")
    }

    #[test]
    fn rota_en_orden_y_vuelve_al_principio() {
        let (rx, r) = grabador();
        r.iniciar(lista("a", 3), ms(40));
        let vistos: Vec<PathBuf> = (0..4).map(|_| esperar(&rx)).collect();
        r.detener();
        assert_eq!(vistos, [lista("a", 3), lista("a", 1)].concat());
    }

    #[test]
    fn siguiente_avanza_en_el_momento() {
        let (rx, r) = grabador();
        r.iniciar(lista("a", 3), Duration::from_secs(10));
        assert_eq!(esperar(&rx), lista("a", 1)[0]);
        r.siguiente();
        assert_eq!(esperar(&rx), lista("a", 2)[1]);
        sleep(ms(50));
        assert!(rx.try_recv().is_err(), "no debería haber más cambios");
        r.detener();
    }

    #[test]
    fn siguiente_en_pausa_avanza_una_imagen() {
        let (rx, r) = grabador();
        r.iniciar(lista("a", 3), Duration::from_secs(10));
        assert_eq!(esperar(&rx), lista("a", 1)[0]);
        r.pausar();
        r.siguiente();
        assert_eq!(esperar(&rx), lista("a", 2)[1]);
        sleep(ms(50));
        assert!(rx.try_recv().is_err(), "sigue en pausa después de saltar");
        assert!(r.pausado());
        r.detener();
    }

    #[test]
    fn pausar_frena_y_reanudar_sigue() {
        let (rx, r) = grabador();
        r.iniciar(lista("a", 3), ms(30));
        esperar(&rx);
        r.pausar();
        assert!(r.pausado());
        // Lo que ya se había puesto antes de pausar no cuenta.
        let _ = rx.try_iter().count();
        sleep(ms(150));
        assert!(rx.try_recv().is_err(), "no cambia mientras está en pausa");
        r.reanudar();
        assert!(!r.pausado());
        esperar(&rx);
        r.detener();
    }

    #[test]
    fn detener_frena_y_queda_inactivo() {
        let (rx, r) = grabador();
        r.iniciar(lista("a", 3), ms(20));
        esperar(&rx);
        r.detener();
        assert!(!r.activo());
        let _ = rx.try_iter().count();
        sleep(ms(100));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn iniciar_de_nuevo_reemplaza_la_rotacion_anterior() {
        let (rx, r) = grabador();
        r.iniciar(lista("a", 3), Duration::from_secs(10));
        assert_eq!(esperar(&rx), lista("a", 1)[0]);
        r.iniciar(lista("b", 3), Duration::from_secs(10));
        assert_eq!(esperar(&rx), lista("b", 1)[0]);
        sleep(ms(50));
        assert!(rx.try_recv().is_err(), "la rotación vieja no debe seguir");
        assert!(r.activo());
        r.detener();
    }

    #[test]
    fn iniciar_en_pausa_no_pone_nada_hasta_reanudar() {
        let (rx, r) = grabador();
        r.iniciar_en_pausa(lista("a", 3), Duration::from_secs(10));
        assert!(r.pausado());
        assert!(r.activo());
        sleep(ms(100));
        assert!(rx.try_recv().is_err(), "en pausa no se pone nada");
        r.reanudar();
        assert_eq!(esperar(&rx), lista("a", 1)[0]);
        r.detener();
    }

    #[test]
    fn siguiente_en_pausa_desde_el_arranque_pone_una_imagen() {
        let (rx, r) = grabador();
        r.iniciar_en_pausa(lista("a", 3), Duration::from_secs(10));
        sleep(ms(50));
        assert!(rx.try_recv().is_err());
        r.siguiente();
        esperar(&rx);
        sleep(ms(50));
        assert!(rx.try_recv().is_err(), "sigue en pausa después de saltar");
        assert!(r.pausado());
        r.detener();
    }

    #[test]
    fn lista_vacia_no_arranca() {
        let (rx, r) = grabador();
        r.iniciar(vec![], ms(10));
        sleep(ms(50));
        assert!(rx.try_recv().is_err());
        assert!(!r.activo());
    }

    #[test]
    fn detener_espera_al_poner_en_curso() {
        let (rx, empezo, r) = grabador_lento(ms(100));
        r.iniciar(lista("a", 3), ms(5));
        empezo.recv_timeout(ESPERA).unwrap(); // el primer `poner` está en vuelo
        r.detener();
        // Al volver `detener` el `poner` en curso ya terminó y no vendrá otro.
        assert_eq!(rx.try_iter().count(), 1);
        sleep(ms(300));
        assert!(rx.try_recv().is_err(), "nada después de detener");
    }

    #[test]
    fn pausar_espera_al_poner_en_curso() {
        let (rx, empezo, r) = grabador_lento(ms(100));
        r.iniciar(lista("a", 3), ms(5));
        empezo.recv_timeout(ESPERA).unwrap(); // el primer `poner` está en vuelo
        r.pausar();
        assert_eq!(rx.try_iter().count(), 1);
        sleep(ms(300));
        assert!(rx.try_recv().is_err(), "nada después de pausar");
        r.detener();
    }
}
