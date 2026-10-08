# Etapa 1 — Núcleo + bandeja: plan de implementación

> **Para agentes:** SUB-SKILL REQUERIDO: usar superpowers:subagent-driven-development (recomendado) o superpowers:executing-plans para implementar este plan tarea por tarea. Los pasos usan checkboxes (`- [ ]`).
>
> **🛑 CHECKPOINT (usuario)** marca pasos que cambian la configuración real de Windows, mueven archivos del usuario o descargan cosas. El agente que ejecuta **se detiene y pide confirmación explícita al usuario** (Nahuel) antes de seguir. Nunca se saltean.

**Objetivo:** una app de bandeja (Tauri 2) que aplica temas a Windows 11 —fondos rotativos, colores, pantalla de bloqueo, colores de Windows Terminal— y los revierte con "Restaurar original". Incluye los temas Star Wars (migrado desde `rotar.ps1`) y Game of Thrones.

**Arquitectura:** workspace de Cargo con dos crates. `motor` es una librería Rust pura (sin Tauri) con los módulos que tocan Windows, la captura del estado original y la orquestación; se testea con `cargo test -p motor`. `src-tauri` es la app Tauri: sólo bandeja del sistema (sin ventanas en esta etapa), que usa `motor`.

**Stack:** Rust stable MSVC, Tauri 2, crates `windows` 0.61, `winreg` 0.55, `jsonc-parser` 0.26 (`cst`, `serde`), `serde`/`serde_json`, `anyhow`, `rand`, `tempfile` (tests). Node 22 sólo para el CLI de Tauri.

**Spec:** `docs/superpowers/specs/2026-10-08-selector-temas-design.md`

---

## Mapa de archivos

```
D:\proyects\temas-windows\
├── Cargo.toml                       workspace
├── package.json                     CLI de Tauri (devDependency)
├── .gitignore
├── ui\index.html                    placeholder (Tauri exige frontendDist; la galería llega en la etapa 2)
├── motor\
│   ├── Cargo.toml
│   ├── src\
│   │   ├── lib.rs                   declara los módulos
│   │   ├── color.rs                 #RRGGBB, HSL, ABGR/ARGB, AccentPalette
│   │   ├── registro.rs              leer/escribir HKCU, capturar/restaurar listas de valores
│   │   ├── tema.rs                  tema.json: tipos, validación, listar temas e imágenes
│   │   ├── win.rs                   llamadas Win32 sueltas (fondo, broadcast, COM)
│   │   ├── estado.rs                Rutas (D:\TemasApp), estado.json, ajustes.json, original.json, archivar a .prev
│   │   ├── nucleo.rs                Motor: aplicar tema / restaurar original
│   │   ├── rotador.rs               hilo que rota los fondos
│   │   └── modulos\
│   │       ├── mod.rs               trait Modulo
│   │       ├── colores.rs
│   │       ├── bloqueo.rs
│   │       ├── fondos.rs
│   │       └── terminal.rs
│   ├── examples\
│   │   ├── spike_colores.rs         prueba manual: ¿el acento cambia en vivo?
│   │   └── spike_bloqueo.rs         prueba manual: ¿la API de bloqueo anda sin admin?
│   └── tests\ida_vuelta.rs          integración contra Windows real (#[ignore])
└── src-tauri\
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json
    ├── icons\                       generados con `npx tauri icon`
    └── src\
        ├── main.rs                  arranque, plugins, setup
        ├── app.rs                   estado de la app (Motor + Rotador) y acciones
        └── bandeja.rs               menú de bandeja y eventos
```

Datos fuera del repo (no se versionan): `D:\Temas\<id>\` (temas) y `D:\TemasApp\` (estado).

Todos los comandos se corren desde `D:\proyects\temas-windows` salvo que se indique otra cosa. Shell: Git Bash (los comandos PowerShell se marcan como tales).

Todos los commits terminan con la línea:
```
Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
```

---

### Tarea 1: Esqueleto del workspace y crate `motor`

**Archivos:**
- Crear: `Cargo.toml`, `.gitignore`, `motor/Cargo.toml`, `motor/src/lib.rs`

- [ ] **Paso 1: Crear `Cargo.toml` del workspace**

Por ahora sólo `motor`; `src-tauri` se agrega en la Tarea 16.

```toml
[workspace]
members = ["motor"]
resolver = "2"
```

- [ ] **Paso 2: Crear `.gitignore`**

```
target/
node_modules/
src-tauri/gen/
```

- [ ] **Paso 3: Crear `motor/Cargo.toml`**

```toml
[package]
name = "motor"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
winreg = "0.55"
jsonc-parser = { version = "0.26", features = ["cst", "serde"] }
rand = "0.8"

[dependencies.windows]
version = "0.61"
features = [
  "Win32_Foundation",
  "Win32_UI_WindowsAndMessaging",
  "Win32_System_WinRT",
  "Foundation",
  "Storage",
  "Storage_Streams",
  "System_UserProfile",
]

[dev-dependencies]
tempfile = "3"
```

- [ ] **Paso 4: Crear `motor/src/lib.rs` vacío**

```rust
//! Motor del selector de temas: todo lo que toca Windows, sin interfaz.
```

- [ ] **Paso 5: Compilar**

Run: `cargo build -p motor`
Expected: compila (descarga dependencias la primera vez; puede tardar varios minutos).

Si alguna versión de crate no existe, usar la última publicada de esa línea (`cargo search <crate>`) y anotarlo en el commit.

- [ ] **Paso 6: Commit**

```bash
git add Cargo.toml .gitignore motor
git commit -m "chore: esqueleto del workspace y crate motor"
```

---

### Tarea 2: `color.rs` — colores, HSL y paleta de acento

**Archivos:**
- Crear: `motor/src/color.rs`
- Modificar: `motor/src/lib.rs`

- [ ] **Paso 1: Declarar el módulo en `lib.rs`**

Agregar al final:
```rust
pub mod color;
```

- [ ] **Paso 2: Escribir los tests (fallan porque no hay implementación)**

Crear `motor/src/color.rs` sólo con los tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn cerca(a: Rgb, b: Rgb) -> bool {
        (a.r as i16 - b.r as i16).abs() <= 1
            && (a.g as i16 - b.g as i16).abs() <= 1
            && (a.b as i16 - b.b as i16).abs() <= 1
    }

    #[test]
    fn parsea_hex_con_y_sin_numeral() {
        assert_eq!(Rgb::desde_hex("#C9A227").unwrap(), Rgb { r: 0xC9, g: 0xA2, b: 0x27 });
        assert_eq!(Rgb::desde_hex("c9a227").unwrap(), Rgb { r: 0xC9, g: 0xA2, b: 0x27 });
    }

    #[test]
    fn rechaza_hex_invalido() {
        assert!(Rgb::desde_hex("#12345").is_err());
        assert!(Rgb::desde_hex("#GGGGGG").is_err());
        assert!(Rgb::desde_hex("").is_err());
    }

    #[test]
    fn a_hex_en_mayusculas() {
        assert_eq!(Rgb { r: 0xc9, g: 0xa2, b: 0x27 }.a_hex(), "#C9A227");
    }

    #[test]
    fn abgr_y_argb() {
        let c = Rgb { r: 0x11, g: 0x22, b: 0x33 };
        assert_eq!(c.abgr(), 0xFF33_2211);
        assert_eq!(c.argb(0xC4), 0xC411_2233);
    }

    #[test]
    fn con_luz_cero_no_cambia() {
        let c = Rgb::desde_hex("#C9A227").unwrap();
        assert!(cerca(c.con_luz(0.0), c));
    }

    #[test]
    fn con_luz_extremos() {
        let c = Rgb::desde_hex("#C9A227").unwrap();
        assert_eq!(c.con_luz(1.0), Rgb { r: 255, g: 255, b: 255 });
        assert_eq!(c.con_luz(-1.0), Rgb { r: 0, g: 0, b: 0 });
    }

    #[test]
    fn paleta_tiene_8_tonos_con_la_base_en_el_medio() {
        let base = Rgb::desde_hex("#C9A227").unwrap();
        let p = paleta_acento(base);
        assert_eq!(p.len(), 32);
        assert!(cerca(Rgb { r: p[12], g: p[13], b: p[14] }, base));
        assert_eq!(&p[28..32], &[0x69, 0x69, 0x69, 0x00]);
        let suma = |i: usize| p[i] as u32 + p[i + 1] as u32 + p[i + 2] as u32;
        assert!(suma(0) > suma(12), "el primer tono es más claro que la base");
        assert!(suma(24) < suma(12), "el séptimo tono es más oscuro que la base");
        for i in 0..8 {
            assert_eq!(p[i * 4 + 3], 0, "el cuarto byte de cada tono es 0");
        }
    }
}
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor color`
Expected: FALLA de compilación (`cannot find type Rgb`).

- [ ] **Paso 4: Implementar (arriba del bloque de tests)**

```rust
//! Colores: parseo #RRGGBB, ajustes de luminosidad HSL y formatos de registro de Windows.

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn desde_hex(s: &str) -> Result<Rgb> {
        let h = s.strip_prefix('#').unwrap_or(s);
        if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("color inválido: {s:?} (se espera #RRGGBB)");
        }
        let n = u32::from_str_radix(h, 16)?;
        Ok(Rgb { r: (n >> 16) as u8, g: (n >> 8) as u8, b: n as u8 })
    }

    pub fn a_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Formato de `AccentColor`, `AccentColorMenu`, `StartColorMenu`.
    pub fn abgr(&self) -> u32 {
        0xFF00_0000 | (self.b as u32) << 16 | (self.g as u32) << 8 | self.r as u32
    }

    /// Formato de `DWM\ColorizationColor`.
    pub fn argb(&self, alfa: u8) -> u32 {
        (alfa as u32) << 24 | (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }

    /// Suma `delta` (−1.0..=1.0) a la luminosidad HSL.
    pub fn con_luz(&self, delta: f32) -> Rgb {
        let (h, s, l) = rgb_a_hsl(*self);
        hsl_a_rgb(h, s, (l + delta).clamp(0.0, 1.0))
    }
}

fn rgb_a_hsl(c: Rgb) -> (f32, f32, f32) {
    let r = c.r as f32 / 255.0;
    let g = c.g as f32 / 255.0;
    let b = c.b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn hsl_a_rgb(h: f32, s: f32, l: f32) -> Rgb {
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return Rgb { r: v, g: v, b: v };
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let canal = |mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round() as u8
    };
    Rgb { r: canal(h + 1.0 / 3.0), g: canal(h), b: canal(h - 1.0 / 3.0) }
}

/// Variaciones de luminosidad de los 7 primeros tonos de `AccentPalette`.
pub const PASOS_PALETA: [f32; 7] = [0.30, 0.20, 0.10, 0.0, -0.10, -0.20, -0.30];

/// `AccentPalette`: 8 tonos de 4 bytes (R, G, B, 0). El 8.º es gris fijo.
pub fn paleta_acento(base: Rgb) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, d) in PASOS_PALETA.iter().enumerate() {
        let c = base.con_luz(*d);
        out[i * 4..i * 4 + 4].copy_from_slice(&[c.r, c.g, c.b, 0]);
    }
    out[28..32].copy_from_slice(&[0x69, 0x69, 0x69, 0]);
    out
}
```

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor color`
Expected: 7 tests PASS.

- [ ] **Paso 6: Commit**

```bash
git add motor/src
git commit -m "feat(motor): colores, HSL y paleta de acento"
```

---

### Tarea 3: `registro.rs` — leer y escribir HKCU

**Archivos:**
- Crear: `motor/src/registro.rs`
- Modificar: `motor/src/lib.rs`

Los tests usan claves propias bajo `HKCU\Software\TemasWindowsTest\<test>` y las borran al final: no tocan configuración real.

- [ ] **Paso 1: Declarar el módulo**

En `lib.rs` agregar: `pub mod registro;`

- [ ] **Paso 2: Escribir los tests**

```rust
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
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor registro`
Expected: FALLA de compilación.

- [ ] **Paso 4: Implementar**

```rust
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
```

Nota: si `winreg` 0.55 define `RegValue.bytes` como `Vec<u8>` o como `Cow<[u8]>`, `.to_vec()` y `.into()` funcionan en ambos casos.

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor registro`
Expected: 5 tests PASS.

- [ ] **Paso 6: Commit**

```bash
git add motor/src
git commit -m "feat(motor): acceso a HKCU con captura y restauración"
```

---

### Tarea 4: `tema.rs` — tema.json, validación y listados

**Archivos:**
- Crear: `motor/src/tema.rs`
- Modificar: `motor/src/lib.rs`

- [ ] **Paso 1: Declarar el módulo**

En `lib.rs`: `pub mod tema;`

- [ ] **Paso 2: Escribir los tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    /// Crea `<base>/<id>/tema.json` y las imágenes indicadas en `fondos/`.
    fn crear(base: &Path, id: &str, conf: Value, fondos: &[&str]) -> PathBuf {
        let dir = base.join(id);
        fs::create_dir_all(dir.join("fondos")).unwrap();
        fs::write(dir.join("tema.json"), conf.to_string()).unwrap();
        for f in fondos {
            fs::write(dir.join("fondos").join(f), b"x").unwrap();
        }
        dir
    }

    #[test]
    fn minimo_solo_nombre() {
        let t = tempfile::tempdir().unwrap();
        let tema = Tema::cargar(&crear(t.path(), "min", json!({"nombre": "Mínimo"}), &[])).unwrap();
        assert_eq!(tema.id, "min");
        assert_eq!(tema.conf.nombre, "Mínimo");
        for m in ["colores", "fondos", "bloqueo", "terminal", "sonidos"] {
            assert!(!tema.define(m), "{m} no debería estar definido");
        }
    }

    #[test]
    fn completo_con_valores_por_defecto() {
        let t = tempfile::tempdir().unwrap();
        let dir = crear(
            t.path(),
            "got",
            json!({
                "nombre": "Game of Thrones",
                "colores": {"acento": "#C9A227", "modo": "oscuro", "acentoEnBarra": true},
                "fondos": {},
                "bloqueo": "fondos/a.jpg",
                "terminal": {"fondo": "#141414", "texto": "#E8D9B0", "cursor": "#C9A227", "ansi": {"red": "#AA0000"}}
            }),
            &["a.jpg", "b.PNG"],
        );
        let tema = Tema::cargar(&dir).unwrap();
        let f = tema.conf.fondos.as_ref().unwrap();
        assert_eq!(f.intervalo_seg, 20);
        assert_eq!(f.orden, Orden::Aleatorio);
        assert_eq!(f.estilo, Estilo::Rellenar);
        assert!(tema.conf.colores.as_ref().unwrap().transparencia);
        assert!(tema.define("colores") && tema.define("fondos") && tema.define("bloqueo") && tema.define("terminal"));
        assert!(tema.advertencias.is_empty());
    }

    #[test]
    fn errores_de_validacion() {
        let t = tempfile::tempdir().unwrap();
        let casos = [
            json!({"nombre": "  "}),
            json!({"nombre": "X", "colores": {"acento": "amarillo", "modo": "oscuro"}}),
            json!({"nombre": "X", "colores": {"acento": "#FFFFFF", "modo": "gris"}}),
            json!({"nombre": "X", "fondos": {"intervaloSeg": 3}}),
            json!({"nombre": "X", "fondos": {}}),
            json!({"nombre": "X", "bloqueo": "no-existe.jpg"}),
            json!({"nombre": "X", "terminal": {"fondo": "#000000", "texto": "#FFFFFF", "cursor": "#FFFFFF", "ansi": {"rosa": "#FF00FF"}}}),
        ];
        for (i, c) in casos.into_iter().enumerate() {
            let dir = crear(t.path(), &format!("caso{i}"), c.clone(), &[]);
            assert!(Tema::cargar(&dir).is_err(), "el caso {i} debería fallar: {c}");
        }
    }

    #[test]
    fn modo_claro_con_acento_en_barra_advierte() {
        let t = tempfile::tempdir().unwrap();
        let dir = crear(
            t.path(),
            "claro",
            json!({"nombre": "C", "colores": {"acento": "#FFFFFF", "modo": "claro", "acentoEnBarra": true}}),
            &[],
        );
        assert_eq!(Tema::cargar(&dir).unwrap().advertencias.len(), 1);
    }

    #[test]
    fn tolera_bom_utf8() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("bom");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("tema.json"), "\u{feff}{\"nombre\": \"Con BOM\"}").unwrap();
        assert_eq!(Tema::cargar(&dir).unwrap().conf.nombre, "Con BOM");
    }

    #[test]
    fn listar_imagenes_filtra_y_ordena() {
        let t = tempfile::tempdir().unwrap();
        for f in ["b.JPG", "a.png", "c.txt", "d.bmp", "e.jpeg"] {
            fs::write(t.path().join(f), b"x").unwrap();
        }
        let nombres: Vec<String> = listar_imagenes(t.path())
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(nombres, ["a.png", "b.JPG", "d.bmp", "e.jpeg"]);
        assert!(listar_imagenes(&t.path().join("no-existe")).unwrap().is_empty());
    }

    #[test]
    fn listar_temas_ignora_ocultos_y_reporta_invalidos() {
        let t = tempfile::tempdir().unwrap();
        crear(t.path(), "zeta", json!({"nombre": "Z"}), &[]);
        crear(t.path(), "alfa", json!({"nombre": ""}), &[]);
        crear(t.path(), "_bin", json!({"nombre": "no"}), &[]);
        fs::write(t.path().join("suelto.txt"), b"x").unwrap();
        let temas = listar_temas(t.path()).unwrap();
        let ids: Vec<&str> = temas.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["alfa", "zeta"]);
        assert!(temas[0].1.is_err());
        assert!(temas[1].1.is_ok());
    }
}
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor tema`
Expected: FALLA de compilación.

- [ ] **Paso 4: Implementar**

```rust
//! Temas: lectura y validación de `tema.json`.

use crate::color::Rgb;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const EXTENSIONES_IMAGEN: [&str; 4] = ["jpg", "jpeg", "png", "bmp"];

pub const NOMBRES_ANSI: [&str; 16] = [
    "black", "red", "green", "yellow", "blue", "purple", "cyan", "white",
    "brightBlack", "brightRed", "brightGreen", "brightYellow",
    "brightBlue", "brightPurple", "brightCyan", "brightWhite",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modo {
    Oscuro,
    Claro,
}

fn si() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColoresConf {
    pub acento: String,
    pub modo: Modo,
    #[serde(default = "si")]
    pub transparencia: bool,
    #[serde(default)]
    pub acento_en_barra: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orden {
    #[default]
    Aleatorio,
    Alfabetico,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Estilo {
    #[default]
    Rellenar,
    Ajustar,
    Expandir,
    Centrar,
}

impl Estilo {
    /// Valor de `HKCU\Control Panel\Desktop\WallpaperStyle`.
    pub fn wallpaper_style(&self) -> &'static str {
        match self {
            Estilo::Rellenar => "10",
            Estilo::Ajustar => "6",
            Estilo::Expandir => "2",
            Estilo::Centrar => "0",
        }
    }
}

fn veinte() -> u64 {
    20
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FondosConf {
    #[serde(default = "veinte")]
    pub intervalo_seg: u64,
    #[serde(default)]
    pub orden: Orden,
    #[serde(default)]
    pub estilo: Estilo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerminalConf {
    pub fondo: String,
    pub texto: String,
    pub cursor: String,
    #[serde(default)]
    pub ansi: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TemaJson {
    pub nombre: String,
    pub colores: Option<ColoresConf>,
    pub fondos: Option<FondosConf>,
    pub bloqueo: Option<String>,
    pub terminal: Option<TerminalConf>,
    // Secciones de etapas posteriores: se aceptan pero todavía no se validan.
    pub sonidos: Option<Value>,
    pub intro: Option<Value>,
    pub protector: Option<Value>,
    pub iconos: Option<Value>,
    pub widget: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct Tema {
    /// Nombre de la carpeta.
    pub id: String,
    pub dir: PathBuf,
    pub conf: TemaJson,
    pub advertencias: Vec<String>,
}

impl Tema {
    pub fn cargar(dir: &Path) -> Result<Tema> {
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .context("carpeta de tema sin nombre")?
            .to_string();
        let ruta = dir.join("tema.json");
        let texto = std::fs::read_to_string(&ruta)
            .with_context(|| format!("leer {}", ruta.display()))?;
        let conf: TemaJson = serde_json::from_str(texto.trim_start_matches('\u{feff}'))
            .context("tema.json mal formado")?;
        let mut tema = Tema { id, dir: dir.to_path_buf(), conf, advertencias: vec![] };
        tema.validar()?;
        Ok(tema)
    }

    fn validar(&mut self) -> Result<()> {
        if self.conf.nombre.trim().is_empty() {
            bail!("\"nombre\" está vacío");
        }
        if let Some(c) = &self.conf.colores {
            Rgb::desde_hex(&c.acento).context("colores.acento")?;
            if c.modo == Modo::Claro && c.acento_en_barra {
                self.advertencias
                    .push("acentoEnBarra no tiene efecto en modo claro; se ignora".into());
            }
        }
        if let Some(f) = &self.conf.fondos {
            if f.intervalo_seg < 5 {
                bail!("fondos.intervaloSeg debe ser ≥ 5 (es {})", f.intervalo_seg);
            }
            if listar_imagenes(&self.dir.join("fondos"))?.is_empty() {
                bail!(r"la carpeta fondos\ no tiene imágenes");
            }
        }
        if let Some(b) = &self.conf.bloqueo {
            if !self.dir.join(b).is_file() {
                bail!("no existe la imagen de bloqueo {b}");
            }
        }
        if let Some(t) = &self.conf.terminal {
            for (campo, v) in [("fondo", &t.fondo), ("texto", &t.texto), ("cursor", &t.cursor)] {
                Rgb::desde_hex(v).with_context(|| format!("terminal.{campo}"))?;
            }
            for (k, v) in &t.ansi {
                if !NOMBRES_ANSI.contains(&k.as_str()) {
                    bail!("terminal.ansi: color desconocido {k:?}");
                }
                Rgb::desde_hex(v).with_context(|| format!("terminal.ansi.{k}"))?;
            }
        }
        Ok(())
    }

    /// ¿El tema configura este módulo?
    pub fn define(&self, modulo: &str) -> bool {
        let c = &self.conf;
        match modulo {
            "colores" => c.colores.is_some(),
            "fondos" => c.fondos.is_some(),
            "bloqueo" => c.bloqueo.is_some(),
            "terminal" => c.terminal.is_some(),
            "sonidos" => c.sonidos.is_some(),
            "intro" => c.intro.is_some(),
            "protector" => c.protector.is_some(),
            "iconos" => c.iconos.is_some(),
            "widget" => c.widget.is_some(),
            _ => false,
        }
    }
}

/// Imágenes de una carpeta (no recursivo), ordenadas por nombre. Carpeta inexistente → vacío.
pub fn listar_imagenes(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| EXTENSIONES_IMAGEN.contains(&e.to_ascii_lowercase().as_str()))
                    .unwrap_or(false)
        })
        .collect();
    v.sort();
    Ok(v)
}

/// Temas de la carpeta, ordenados por id. Ignora archivos y carpetas que empiezan con `_` o `.`.
/// Los inválidos se devuelven como `Err` para mostrarlos marcados.
pub fn listar_temas(carpeta: &Path) -> Result<Vec<(String, Result<Tema>)>> {
    let mut out = vec![];
    if !carpeta.is_dir() {
        return Ok(out);
    }
    for e in std::fs::read_dir(carpeta)? {
        let p = e?.path();
        let nombre = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if !p.is_dir() || nombre.starts_with('_') || nombre.starts_with('.') {
            continue;
        }
        out.push((nombre, Tema::cargar(&p)));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}
```

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor tema`
Expected: 7 tests PASS.

- [ ] **Paso 6: Commit**

```bash
git add motor/src
git commit -m "feat(motor): lectura y validación de tema.json"
```

---

### Tarea 5: trait `Modulo` y helpers Win32

**Archivos:**
- Crear: `motor/src/modulos/mod.rs`, `motor/src/win.rs`
- Modificar: `motor/src/lib.rs`

Sin tests unitarios: son declaraciones y llamadas directas a Win32 que se prueban en las tareas 6, 7, 11 y 13.

- [ ] **Paso 1: Crear `motor/src/win.rs`**

```rust
//! Llamadas Win32 sueltas.

use anyhow::Result;
use std::path::{Path, PathBuf};
use windows::core::HSTRING;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, SystemParametersInfoW, HWND_BROADCAST, SMTO_ABORTIFHUNG,
    SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETDESKWALLPAPER, SPI_SETDESKWALLPAPER,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WM_SETTINGCHANGE,
};

/// Avisa a todas las ventanas que cambió una configuración (p. ej. "ImmersiveColorSet").
pub fn avisar_cambio(area: &str) {
    let h = HSTRING::from(area);
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(h.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5000,
            None,
        );
    }
}

pub fn poner_fondo(ruta: &Path) -> Result<()> {
    let h = HSTRING::from(ruta.to_string_lossy().as_ref());
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(h.as_ptr() as *mut core::ffi::c_void),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )?;
    }
    Ok(())
}

pub fn fondo_actual() -> Result<PathBuf> {
    let mut buf = [0u16; 1024];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Ok(PathBuf::from(String::from_utf16_lossy(&buf[..n])))
}

/// Inicializa WinRT en el hilo actual (necesario antes de usar APIs WinRT desde hilos propios).
/// Si ya estaba inicializado, no pasa nada.
pub fn iniciar_winrt() {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}
```

Si alguna firma no coincide con `windows` 0.61 (por ejemplo `HWND_BROADCAST` o el tipo de `pvparam`), ajustarla según https://microsoft.github.io/windows-docs-rs/ sin cambiar el comportamiento.

- [ ] **Paso 2: Crear `motor/src/modulos/mod.rs`**

```rust
//! Cada parte de Windows que un tema puede cambiar es un módulo.

use crate::tema::Tema;
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub trait Modulo: Send + Sync {
    /// Igual a la sección de `tema.json` ("colores", "fondos", …).
    fn id(&self) -> &'static str;
    /// Estado actual de Windows. Si hay que guardar archivos, van en `dir` (carpeta propia del módulo).
    fn capturar(&self, dir: &Path) -> Result<Value>;
    fn aplicar(&self, tema: &Tema) -> Result<()>;
    /// Vuelve a lo capturado. `dir` es la misma carpeta que recibió `capturar`.
    fn restaurar(&self, captura: &Value, dir: &Path) -> Result<()>;
}
```

- [ ] **Paso 3: Declarar en `lib.rs`**

```rust
pub mod modulos;
pub mod win;
```

- [ ] **Paso 4: Compilar**

Run: `cargo build -p motor`
Expected: compila.

- [ ] **Paso 5: Commit**

```bash
git add motor/src
git commit -m "feat(motor): trait Modulo y helpers Win32"
```

---

### Tarea 6: módulo `colores` + spike visual

**Archivos:**
- Crear: `motor/src/modulos/colores.rs`, `motor/examples/spike_colores.rs`
- Modificar: `motor/src/modulos/mod.rs`

- [ ] **Paso 1: Declarar el submódulo**

En `modulos/mod.rs`, arriba: `pub mod colores;`

- [ ] **Paso 2: Escribir los tests (de la parte pura)**

```rust
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
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor colores`
Expected: FALLA de compilación.

- [ ] **Paso 4: Implementar**

```rust
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
```

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor colores`
Expected: 3 tests PASS.

- [ ] **Paso 6: Escribir el spike `motor/examples/spike_colores.rs`**

```rust
//! Prueba manual: ¿el acento cambia en vivo en barra, Inicio y títulos sin cerrar sesión?
//! Uso: cargo run -p motor --example spike_colores -- "#C9A227"

use motor::modulos::{colores::Colores, Modulo};
use motor::tema::Tema;
use std::io::stdin;

fn main() -> anyhow::Result<()> {
    let acento = std::env::args().nth(1).unwrap_or_else(|| "#C9A227".into());
    let tmp = tempfile::tempdir()?;
    let dir = tmp.path().join("spike");
    std::fs::create_dir_all(&dir)?;
    let conf = serde_json::json!({
        "nombre": "Spike",
        "colores": {"acento": acento, "modo": "oscuro", "acentoEnBarra": true}
    });
    std::fs::write(dir.join("tema.json"), conf.to_string())?;
    let tema = Tema::cargar(&dir)?;

    let m = Colores;
    let captura = m.capturar(&dir)?;
    println!("Estado actual guardado. Aplicando {acento}…");
    m.aplicar(&tema)?;
    println!("Mirá la barra de tareas, el menú Inicio y la barra de título de una ventana.");
    println!("Enter para restaurar.");
    stdin().read_line(&mut String::new())?;
    m.restaurar(&captura, &dir)?;
    println!("Restaurado. ¿Quedó todo como antes?");
    Ok(())
}
```

- [ ] **Paso 7: 🛑 CHECKPOINT (usuario) — correr el spike**

Pedir confirmación al usuario: "Voy a cambiar temporalmente el color de acento y el modo oscuro de Windows para probar; al apretar Enter vuelve todo como estaba. ¿Dale?". Con el sí:

Run: `cargo run -p motor --example spike_colores -- "#C9A227"` (en una terminal visible para el usuario, por ejemplo con `mcp__terminal__run_in_terminal`).

Preguntar al usuario qué vio y anotar el resultado:
- **Cambia en vivo:** seguir.
- **No cambia la barra/Inicio:** agregar en `Colores::aplicar`, después del broadcast existente, `win::avisar_cambio("WindowsThemeElement");` y repetir el spike. Si sigue sin cambiar, frenar y consultar al usuario (la alternativa es reiniciar el Explorador, que es invasivo).
- **Los tonos de Inicio/barra se ven raros:** ajustar `AccentColorMenu`/`StartColorMenu` (por ejemplo usar `base.con_luz(-0.10)`), actualizar el test `oscuro_con_barra` y repetir.

- [ ] **Paso 8: Anotar el resultado en el spec**

En `docs/superpowers/specs/2026-10-08-selector-temas-design.md`, sección "Etapas", debajo de la etapa 1, agregar una línea: `Resultado del spike de colores (AAAA-MM-DD): <qué se vio y qué se ajustó>.`

- [ ] **Paso 9: Commit**

```bash
git add motor docs
git commit -m "feat(motor): módulo colores y spike de acento en vivo"
```

---

### Tarea 7: módulo `bloqueo` + spike

**Archivos:**
- Crear: `motor/src/modulos/bloqueo.rs`, `motor/examples/spike_bloqueo.rs`
- Modificar: `motor/src/modulos/mod.rs`

- [ ] **Paso 1: Declarar el submódulo**

En `modulos/mod.rs`: `pub mod bloqueo;`

- [ ] **Paso 2: Escribir los tests (parte pura)**

```rust
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
        assert!(!spotlight_en(&captura(0)));
        assert!(!spotlight_en(&serde_json::json!({})));
    }
}
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor bloqueo`
Expected: FALLA de compilación.

- [ ] **Paso 4: Implementar**

```rust
//! Pantalla de bloqueo.

use super::Modulo;
use crate::registro::{self, RegValor};
use crate::tema::Tema;
use crate::win;
use anyhow::{Context, Result};
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
    let tam = stream.Size()? as u32;
    let lector = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    lector.LoadAsync(tam)?.get()?;
    let mut buf = vec![0u8; tam as usize];
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
    captura["registro"][clave] == json!({"tipo": "dword", "valor": 1})
}

pub struct Bloqueo;

impl Modulo for Bloqueo {
    fn id(&self) -> &'static str {
        "bloqueo"
    }

    fn capturar(&self, dir: &Path) -> Result<Value> {
        let registro = registro::capturar_valores(&CLAVES)?;
        let archivo = match leer_imagen() {
            Ok(b) if !b.is_empty() => {
                let nombre = format!("bloqueo.{}", extension_por_contenido(&b));
                std::fs::write(dir.join(&nombre), &b)?;
                Some(nombre)
            }
            _ => None,
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
        if !spotlight_en(captura) {
            if let Some(n) = captura["archivo"].as_str() {
                poner_imagen(&dir.join(n))?;
            }
        }
        registro::restaurar_valores(&captura["registro"])
    }
}
```

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor bloqueo`
Expected: 2 tests PASS.

- [ ] **Paso 6: Escribir el spike `motor/examples/spike_bloqueo.rs`**

```rust
//! Prueba manual: ¿la API de pantalla de bloqueo anda sin admin y desde una app sin empaquetar?
//! Uso: cargo run -p motor --example spike_bloqueo -- "C:\Windows\Web\Wallpaper\Windows\img0.jpg"

use motor::modulos::{bloqueo::Bloqueo, Modulo};
use motor::tema::Tema;
use std::io::stdin;

fn main() -> anyhow::Result<()> {
    let imagen = std::env::args().nth(1).expect("pasá la ruta de una imagen");
    let tmp = tempfile::tempdir()?;
    let dir = tmp.path().join("spike");
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(&imagen, dir.join("bloqueo.jpg"))?;
    std::fs::write(dir.join("tema.json"), r#"{"nombre":"Spike","bloqueo":"bloqueo.jpg"}"#)?;
    let tema = Tema::cargar(&dir)?;
    // Carpeta aparte para la captura: si fuera `dir`, pisaría el bloqueo.jpg del tema.
    let dir_cap = tmp.path().join("original");
    std::fs::create_dir_all(&dir_cap)?;

    let m = Bloqueo;
    let captura = m.capturar(&dir_cap)?;
    println!("Captura: {captura}");
    m.aplicar(&tema)?;
    println!("Aplicado. Apretá Win+L para ver la pantalla de bloqueo, volvé, y Enter para restaurar.");
    stdin().read_line(&mut String::new())?;
    m.restaurar(&captura, &dir_cap)?;
    println!("Restaurado. Win+L de nuevo para confirmar.");
    Ok(())
}
```

- [ ] **Paso 7: 🛑 CHECKPOINT (usuario) — correr el spike**

Pedir confirmación: "Voy a cambiar temporalmente la imagen de la pantalla de bloqueo para probar y después la restauro. Te voy a pedir que aprietes Win+L dos veces. ¿Dale?". Con el sí:

Run: `cargo run -p motor --example spike_bloqueo -- "C:\Windows\Web\Wallpaper\Windows\img0.jpg"` en una terminal visible.

Resultados posibles:
- **Funciona y restaura:** seguir.
- **Error de acceso o "requiere identidad de paquete":** frenar y avisar al usuario. Anotar en el spec que el módulo `bloqueo` se pospone a la etapa 2 (con el instalador la app tiene identidad), quitar `bloqueo` de los temas de esta etapa y seguir con el resto.
- **Spotlight no vuelve al restaurar:** anotar en el spec y consultar al usuario.

- [ ] **Paso 8: Anotar el resultado en el spec** (misma sección que en la Tarea 6).

- [ ] **Paso 9: Commit**

```bash
git add motor docs
git commit -m "feat(motor): módulo pantalla de bloqueo y spike"
```

---

### Tarea 8: `estado.rs` — rutas y archivos de estado

**Archivos:**
- Crear: `motor/src/estado.rs`
- Modificar: `motor/src/lib.rs`

- [ ] **Paso 1: Declarar el módulo**

En `lib.rs`: `pub mod estado;`

- [ ] **Paso 2: Escribir los tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rutas() -> (tempfile::TempDir, Rutas) {
        let t = tempfile::tempdir().unwrap();
        let r = Rutas::nuevo(t.path());
        (t, r)
    }

    #[test]
    fn sin_archivos_hay_valores_por_defecto() {
        let (_t, r) = rutas();
        assert_eq!(Estado::cargar(&r).unwrap(), Estado::default());
        assert_eq!(Ajustes::cargar(&r).unwrap().carpeta_temas, PathBuf::from(r"D:\Temas"));
        assert!(Original::cargar(&r).unwrap().0.is_empty());
    }

    #[test]
    fn estado_ida_y_vuelta() {
        let (_t, r) = rutas();
        let e = Estado { tema_activo: Some("star-wars".into()), tocados: vec!["fondos".into()], fondos_pausados: true };
        e.guardar(&r).unwrap();
        assert_eq!(Estado::cargar(&r).unwrap(), e);
        let texto = std::fs::read_to_string(r.estado()).unwrap();
        assert!(texto.contains("temaActivo"), "se guarda en camelCase: {texto}");
    }

    #[test]
    fn original_ida_y_vuelta() {
        let (_t, r) = rutas();
        let mut o = Original::default();
        o.poner("colores", json!({"a": 1}));
        o.guardar(&r).unwrap();
        assert_eq!(Original::cargar(&r).unwrap().get("colores"), Some(&json!({"a": 1})));
    }

    #[test]
    fn archivar_mueve_entradas_y_carpetas_elegidas() {
        let (_t, r) = rutas();
        let mut o = Original::default();
        o.poner("fondos", json!({"archivo": "fondo.jpg"}));
        o.poner("colores", json!({"a": 1}));
        o.guardar(&r).unwrap();
        std::fs::create_dir_all(r.dir_original("fondos")).unwrap();
        std::fs::write(r.dir_original("fondos").join("fondo.jpg"), b"nuevo").unwrap();
        // Un .prev viejo de fondos tiene que ser reemplazado.
        std::fs::create_dir_all(r.dir_original_prev("fondos")).unwrap();
        std::fs::write(r.dir_original_prev("fondos").join("viejo.jpg"), b"viejo").unwrap();

        archivar_original(&r, &["fondos".to_string()]).unwrap();

        let o = Original::cargar(&r).unwrap();
        assert!(o.get("fondos").is_none());
        assert!(o.get("colores").is_some(), "lo no archivado queda");
        let prev: Original = leer_json(&r.original_prev()).unwrap();
        assert_eq!(prev.get("fondos"), Some(&json!({"archivo": "fondo.jpg"})));
        assert!(!r.dir_original("fondos").exists());
        assert!(r.dir_original_prev("fondos").join("fondo.jpg").exists());
        assert!(!r.dir_original_prev("fondos").join("viejo.jpg").exists());
    }

    #[test]
    fn tolera_bom() {
        let (_t, r) = rutas();
        std::fs::create_dir_all(&r.raiz).unwrap();
        std::fs::write(r.original(), "\u{feff}{\"fondos\": {\"archivo\": \"fondo.jpg\"}}").unwrap();
        assert!(Original::cargar(&r).unwrap().get("fondos").is_some());
    }
}
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor estado`
Expected: FALLA de compilación.

- [ ] **Paso 4: Implementar**

```rust
//! Dónde vive el estado de la app y cómo se lee/escribe.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Rutas {
    pub raiz: PathBuf,
}

impl Rutas {
    pub fn nuevo(raiz: impl Into<PathBuf>) -> Rutas {
        Rutas { raiz: raiz.into() }
    }
    pub fn por_defecto() -> Rutas {
        Rutas::nuevo(r"D:\TemasApp")
    }
    pub fn estado(&self) -> PathBuf {
        self.raiz.join("estado.json")
    }
    pub fn ajustes(&self) -> PathBuf {
        self.raiz.join("ajustes.json")
    }
    pub fn original(&self) -> PathBuf {
        self.raiz.join("original.json")
    }
    pub fn original_prev(&self) -> PathBuf {
        self.raiz.join("original.prev.json")
    }
    pub fn dir_original(&self, modulo: &str) -> PathBuf {
        self.raiz.join("original").join(modulo)
    }
    pub fn dir_original_prev(&self, modulo: &str) -> PathBuf {
        self.raiz.join("original.prev").join(modulo)
    }
}

pub(crate) fn leer_json<T: DeserializeOwned + Default>(ruta: &Path) -> Result<T> {
    if !ruta.exists() {
        return Ok(T::default());
    }
    let texto = fs::read_to_string(ruta).with_context(|| format!("leer {}", ruta.display()))?;
    serde_json::from_str(texto.trim_start_matches('\u{feff}'))
        .with_context(|| format!("{} mal formado", ruta.display()))
}

/// Escribe en un temporal y renombra, para no dejar archivos a medias.
pub(crate) fn escribir_json<T: Serialize>(ruta: &Path, v: &T) -> Result<()> {
    if let Some(p) = ruta.parent() {
        fs::create_dir_all(p)?;
    }
    let tmp = ruta.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(v)?)?;
    fs::rename(&tmp, ruta).with_context(|| format!("guardar {}", ruta.display()))?;
    Ok(())
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Estado {
    pub tema_activo: Option<String>,
    /// Módulos que el tema activo intentó aplicar (bien o mal) y que hay que restaurar al cambiar.
    pub tocados: Vec<String>,
    pub fondos_pausados: bool,
}

impl Estado {
    pub fn cargar(r: &Rutas) -> Result<Estado> {
        leer_json(&r.estado())
    }
    pub fn guardar(&self, r: &Rutas) -> Result<()> {
        escribir_json(&r.estado(), self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Ajustes {
    pub carpeta_temas: PathBuf,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes { carpeta_temas: PathBuf::from(r"D:\Temas") }
    }
}

impl Ajustes {
    pub fn cargar(r: &Rutas) -> Result<Ajustes> {
        leer_json(&r.ajustes())
    }
    pub fn guardar(&self, r: &Rutas) -> Result<()> {
        escribir_json(&r.ajustes(), self)
    }
}

/// Capturas del estado original: `módulo → captura`.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Original(pub BTreeMap<String, Value>);

impl Original {
    pub fn cargar(r: &Rutas) -> Result<Original> {
        leer_json(&r.original())
    }
    pub fn guardar(&self, r: &Rutas) -> Result<()> {
        escribir_json(&r.original(), self)
    }
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.0.get(id)
    }
    pub fn poner(&mut self, id: &str, v: Value) {
        self.0.insert(id.to_string(), v);
    }
}

/// Pasa las capturas de `ids` (y sus carpetas) de `original` a `original.prev`.
pub fn archivar_original(r: &Rutas, ids: &[String]) -> Result<()> {
    let mut orig = Original::cargar(r)?;
    let mut prev: Original = leer_json(&r.original_prev())?;
    for id in ids {
        if let Some(v) = orig.0.remove(id) {
            prev.0.insert(id.clone(), v);
        }
        let src = r.dir_original(id);
        let dst = r.dir_original_prev(id);
        if dst.exists() {
            fs::remove_dir_all(&dst)?;
        }
        if src.exists() {
            fs::create_dir_all(dst.parent().expect("dir_original_prev tiene padre"))?;
            fs::rename(&src, &dst)?;
        }
    }
    escribir_json(&r.original_prev(), &prev)?;
    orig.guardar(r)
}
```

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor estado`
Expected: 5 tests PASS.

- [ ] **Paso 6: Commit**

```bash
git add motor/src
git commit -m "feat(motor): rutas y archivos de estado"
```

---

### Tarea 9: `nucleo.rs` — aplicar tema y restaurar original

**Archivos:**
- Crear: `motor/src/nucleo.rs`
- Modificar: `motor/src/lib.rs`

Se testea con módulos falsos que anotan lo que se les pide.

- [ ] **Paso 1: Declarar el módulo**

En `lib.rs`: `pub mod nucleo;`

- [ ] **Paso 2: Escribir los tests**

```rust
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
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor nucleo`
Expected: FALLA de compilación.

- [ ] **Paso 4: Implementar**

```rust
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
```

Nota: `restaurar_original` recorre `original.0` (un `BTreeMap`), o sea en orden alfabético: por eso el test espera `colores` antes que `terminal`.

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor nucleo`
Expected: 11 tests PASS.

- [ ] **Paso 6: Commit**

```bash
git add motor/src
git commit -m "feat(motor): aplicar tema y restaurar original"
```

---

### Tarea 10: `rotador.rs` — hilo de rotación de fondos

**Archivos:**
- Crear: `motor/src/rotador.rs`
- Modificar: `motor/src/lib.rs`

- [ ] **Paso 1: Declarar el módulo**

En `lib.rs`: `pub mod rotador;`

- [ ] **Paso 2: Escribir los tests**

```rust
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
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor rotador`
Expected: FALLA de compilación.

- [ ] **Paso 4: Implementar**

```rust
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
```

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor rotador`
Expected: 6 tests PASS. Si alguno es intermitente por tiempos en una máquina cargada, agrandar los márgenes del test (no los del código).

- [ ] **Paso 6: Commit**

```bash
git add motor/src
git commit -m "feat(motor): rotador de fondos"
```

---

### Tarea 11: módulo `fondos`

**Archivos:**
- Crear: `motor/src/modulos/fondos.rs`
- Modificar: `motor/src/modulos/mod.rs`

- [ ] **Paso 1: Declarar el submódulo**

En `modulos/mod.rs`: `pub mod fondos;`

- [ ] **Paso 2: Escribir los tests (parte pura)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ordenar_alfabetico_conserva_y_aleatorio_mezcla_los_mismos() {
        let l: Vec<PathBuf> = (0..20).map(|i| PathBuf::from(format!("{i:02}.jpg"))).collect();
        assert_eq!(ordenar(l.clone(), Orden::Alfabetico), l);
        let mut m = ordenar(l.clone(), Orden::Aleatorio);
        m.sort();
        assert_eq!(m, l);
    }

    #[test]
    fn lista_de_toma_las_imagenes_y_el_intervalo() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("sw");
        std::fs::create_dir_all(dir.join("fondos")).unwrap();
        for f in ["b.jpg", "a.png"] {
            std::fs::write(dir.join("fondos").join(f), b"x").unwrap();
        }
        let conf = json!({"nombre": "SW", "fondos": {"intervaloSeg": 30, "orden": "alfabetico"}});
        std::fs::write(dir.join("tema.json"), conf.to_string()).unwrap();
        let tema = Tema::cargar(&dir).unwrap();
        let (lista, intervalo) = lista_de(&tema).unwrap();
        assert_eq!(intervalo, Duration::from_secs(30));
        assert_eq!(lista, [dir.join("fondos").join("a.png"), dir.join("fondos").join("b.jpg")]);
    }
}
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor fondos`
Expected: FALLA de compilación.

- [ ] **Paso 4: Implementar**

```rust
//! Fondos de escritorio rotativos.

use super::Modulo;
use crate::registro::{self, RegValor};
use crate::rotador::Rotador;
use crate::tema::{listar_imagenes, Orden, Tema};
use crate::win;
use anyhow::{bail, Context, Result};
use rand::seq::SliceRandom;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const DESKTOP: &str = r"Control Panel\Desktop";

pub fn ordenar(mut lista: Vec<PathBuf>, orden: Orden) -> Vec<PathBuf> {
    if orden == Orden::Aleatorio {
        lista.shuffle(&mut rand::thread_rng());
    }
    lista
}

/// Imágenes (ya ordenadas) e intervalo de rotación del tema.
pub fn lista_de(tema: &Tema) -> Result<(Vec<PathBuf>, Duration)> {
    let conf = tema.conf.fondos.as_ref().context("el tema no define fondos")?;
    let lista = ordenar(listar_imagenes(&tema.dir.join("fondos"))?, conf.orden);
    if lista.is_empty() {
        bail!(r"no hay imágenes en fondos\");
    }
    Ok((lista, Duration::from_secs(conf.intervalo_seg)))
}

fn escribir_estilo(estilo: &RegValor) -> Result<()> {
    registro::escribir(DESKTOP, "WallpaperStyle", estilo)?;
    registro::escribir(DESKTOP, "TileWallpaper", &RegValor::Sz("0".into()))
}

/// Copia que Windows guarda del fondo actual; sirve si el archivo original ya no existe.
fn transcoded() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
        .join(r"Microsoft\Windows\Themes\TranscodedWallpaper")
}

pub struct Fondos {
    pub rotador: Arc<Rotador>,
    /// `D:\TemasApp`: ahí va `fondo-actual.<ext>` al restaurar.
    pub raiz: PathBuf,
}

impl Modulo for Fondos {
    fn id(&self) -> &'static str {
        "fondos"
    }

    fn capturar(&self, dir: &Path) -> Result<Value> {
        let actual = win::fondo_actual()?;
        let (fuente, ext) = if actual.is_file() {
            let ext = actual
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg")
                .to_ascii_lowercase();
            (actual, ext)
        } else {
            (transcoded(), "jpg".to_string())
        };
        let archivo = format!("fondo.{ext}");
        std::fs::copy(&fuente, dir.join(&archivo))
            .with_context(|| format!("copiar el fondo actual ({})", fuente.display()))?;
        Ok(json!({ "archivo": archivo, "estilo": registro::leer(DESKTOP, "WallpaperStyle")? }))
    }

    fn aplicar(&self, tema: &Tema) -> Result<()> {
        let conf = tema.conf.fondos.as_ref().context("el tema no define fondos")?;
        let (lista, intervalo) = lista_de(tema)?;
        escribir_estilo(&RegValor::Sz(conf.estilo.wallpaper_style().into()))?;
        self.rotador.iniciar(lista, intervalo);
        Ok(())
    }

    fn restaurar(&self, captura: &Value, dir: &Path) -> Result<()> {
        self.rotador.detener();
        let archivo = captura["archivo"].as_str().context("captura de fondos sin archivo")?;
        let ext = Path::new(archivo).extension().and_then(|e| e.to_str()).unwrap_or("jpg");
        // Copia a una ruta estable: `dir` puede moverse a original.prev después.
        let destino = self.raiz.join(format!("fondo-actual.{ext}"));
        std::fs::copy(dir.join(archivo), &destino)
            .with_context(|| format!("copiar {archivo} a {}", destino.display()))?;
        escribir_estilo(&serde_json::from_value(captura["estilo"].clone())?)?;
        win::poner_fondo(&destino)
    }
}
```

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor fondos`
Expected: 2 tests PASS.

- [ ] **Paso 6: Commit**

```bash
git add motor/src
git commit -m "feat(motor): módulo fondos rotativos"
```

---

### Tarea 12: módulo `terminal`

**Archivos:**
- Crear: `motor/src/modulos/terminal.rs`
- Modificar: `motor/src/modulos/mod.rs`

Toda la edición se prueba sobre textos y archivos temporales; nunca sobre el `settings.json` real.

- [ ] **Paso 1: Declarar el submódulo**

En `modulos/mod.rs`: `pub mod terminal;`

- [ ] **Paso 2: Escribir los tests**

```rust
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
```

- [ ] **Paso 3: Correr y ver que falla**

Run: `cargo test -p motor terminal`
Expected: FALLA de compilación.

- [ ] **Paso 4: Implementar**

```rust
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
```

**Sobre la API CST de `jsonc-parser`:** los nombres usados (`CstRootNode::parse`, `object_value`, `object_value_or_set`, `array_value`, `array_value_or_set`, `get`, `append`, `set_value`, `remove`, `elements`, `as_object`, `as_string_lit`, `decoded_value`, `CstInputValue`) corresponden a la 0.26. Si alguno difiere, consultar https://docs.rs/jsonc-parser/latest/jsonc_parser/cst/ y adaptar **sin cambiar el comportamiento** que fijan los tests.

- [ ] **Paso 5: Correr y ver que pasa**

Run: `cargo test -p motor terminal`
Expected: 9 tests PASS.

- [ ] **Paso 6: Correr toda la suite**

Run: `cargo test -p motor`
Expected: todos PASS.

- [ ] **Paso 7: Commit**

```bash
git add motor/src
git commit -m "feat(motor): módulo esquema de Windows Terminal"
```

---

### Tarea 13: migración desde `rotar.ps1` y tema Star Wars

Sin código en el repo: son operaciones únicas sobre los datos del usuario.

- [ ] **Paso 1: 🛑 CHECKPOINT (usuario) — confirmar la migración**

Pedir confirmación con este detalle exacto:
- Frenar el proceso de PowerShell que corre `D:\Wallpapers\rotar.ps1` (el fondo deja de rotar hasta que la app esté lista).
- **Mover** (no copiar) los 14 archivos de `D:\Wallpapers\StarWars\` a `D:\Temas\star-wars\fondos\`.
- Crear `D:\TemasApp\` con la captura del fondo original = fondo por defecto de Windows (`C:\Windows\Web\Wallpaper\Windows\img0.jpg`, copiado).

- [ ] **Paso 2: Frenar `rotar.ps1` (PowerShell)**

```powershell
Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" | Where-Object CommandLine -like '*rotar.ps1*' | ForEach-Object { Stop-Process -Id $_.ProcessId }
```

Verificar: el mismo `Get-CimInstance … | Where-Object …` sin el `ForEach-Object` no devuelve nada.

- [ ] **Paso 3: Mover los fondos (PowerShell)**

```powershell
New-Item -ItemType Directory -Force D:\Temas\star-wars\fondos | Out-Null
Move-Item D:\Wallpapers\StarWars\* D:\Temas\star-wars\fondos\
(Get-ChildItem D:\Temas\star-wars\fondos).Count
```

Expected: `14`.

- [ ] **Paso 4: Sembrar la captura del fondo original (PowerShell)**

```powershell
New-Item -ItemType Directory -Force D:\TemasApp\original\fondos | Out-Null
Copy-Item C:\Windows\Web\Wallpaper\Windows\img0.jpg D:\TemasApp\original\fondos\fondo.jpg
[IO.File]::WriteAllText('D:\TemasApp\original.json', '{"fondos":{"archivo":"fondo.jpg","estilo":{"tipo":"sz","valor":"10"}}}')
```

(`WriteAllText` escribe UTF-8 sin BOM.)

- [ ] **Paso 5: 🛑 CHECKPOINT (usuario) — imagen de bloqueo y colores de Star Wars**

Mostrarle al usuario las 14 imágenes (por ejemplo abriendo la carpeta con `mcp__ccd_host__reveal_path`) y pedirle que elija cuál va en la pantalla de bloqueo. Proponer y confirmar estos colores: acento `#FFE81F` (amarillo del logo), modo oscuro, acento en barra; terminal fondo `#000000`, texto `#FFE81F`, cursor `#4FC3F7`.

Copiar la elegida **conservando su extensión** (4 de las 14 son PNG) (PowerShell):
```powershell
Copy-Item D:\Temas\star-wars\fondos\<elegida.ext> D:\Temas\star-wars\bloqueo.<ext>
```

- [ ] **Paso 6: Escribir `D:\Temas\star-wars\tema.json`** (con los valores confirmados; `"bloqueo"` con la extensión real del archivo copiado)

```json
{
  "nombre": "Star Wars",
  "colores": { "acento": "#FFE81F", "modo": "oscuro", "transparencia": true, "acentoEnBarra": true },
  "fondos": { "intervaloSeg": 20, "orden": "aleatorio", "estilo": "rellenar" },
  "bloqueo": "bloqueo.jpg",
  "terminal": { "fondo": "#000000", "texto": "#FFE81F", "cursor": "#4FC3F7" }
}
```

(Si el spike de bloqueo de la Tarea 7 falló, omitir la línea `"bloqueo"`.)

- [ ] **Paso 7: Validar con un test de humo temporal**

Run: `cargo test -p motor tema` (sigue pasando) y además validar el tema real con un one-liner de ejemplo:

Crear `motor/examples/validar_tema.rs`:
```rust
//! Valida un tema: cargo run -p motor --example validar_tema -- D:\Temas\star-wars
fn main() -> anyhow::Result<()> {
    let dir = std::env::args().nth(1).expect("pasá la carpeta del tema");
    let tema = motor::tema::Tema::cargar(std::path::Path::new(&dir))?;
    println!("OK: {} — advertencias: {:?}", tema.conf.nombre, tema.advertencias);
    Ok(())
}
```

Run: `cargo run -p motor --example validar_tema -- D:\Temas\star-wars`
Expected: `OK: Star Wars — advertencias: []`

- [ ] **Paso 8: Commit** (sólo el ejemplo; los datos no se versionan)

```bash
git add motor/examples/validar_tema.rs
git commit -m "chore(motor): ejemplo para validar un tema"
```

---

### Tarea 14: tema Game of Thrones

- [ ] **Paso 1: Buscar fondos en Wallhaven**

Run:
```bash
curl -s "https://wallhaven.cc/api/v1/search?q=game%20of%20thrones&categories=110&purity=100&atleast=2560x1440&ratios=16x9&sorting=toplist&topRange=1y" | python -I -c "import json,sys;d=json.load(sys.stdin);[print(w['id'],w['resolution'],w['file_size']//1024,'KB',w['path']) for w in d['data'][:24]]"
```

Si vienen menos de 10 resultados, repetir con `topRange=3y`.

- [ ] **Paso 2: 🛑 CHECKPOINT (usuario) — confirmar descarga**

Presentar la lista (id, resolución, tamaño), excluyendo archivos de más de 10 MB, con el total estimado y el destino `D:\Temas\game-of-thrones\fondos\`. Pedir confirmación.

- [ ] **Paso 3: Descargar y verificar**

```bash
mkdir -p /d/Temas/game-of-thrones/fondos && cd /d/Temas/game-of-thrones/fondos && for p in <rutas confirmadas, formato "xx/wallhaven-ID.ext">; do curl -sfL -o "$(basename $p)" "https://w.wallhaven.cc/full/$p" || echo "FALLÓ $p"; done; file *
```

Expected: todos `JPEG image data` o `PNG image data`. Borrar sólo lo que no sea imagen, previa confirmación.

- [ ] **Paso 4: 🛑 CHECKPOINT (usuario) — imagen de bloqueo y colores**

Mostrar las imágenes, elegir la de bloqueo, y confirmar: acento `#C9A227`, modo oscuro, acento en barra; terminal fondo `#141414`, texto `#E8D9B0`, cursor `#C9A227`.

```powershell
Copy-Item D:\Temas\game-of-thrones\fondos\<elegida.ext> D:\Temas\game-of-thrones\bloqueo.<ext>
```

- [ ] **Paso 5: Escribir `D:\Temas\game-of-thrones\tema.json`** (`"bloqueo"` con la extensión real del archivo copiado)

```json
{
  "nombre": "Game of Thrones",
  "colores": { "acento": "#C9A227", "modo": "oscuro", "transparencia": true, "acentoEnBarra": true },
  "fondos": { "intervaloSeg": 20, "orden": "aleatorio", "estilo": "rellenar" },
  "bloqueo": "bloqueo.jpg",
  "terminal": { "fondo": "#141414", "texto": "#E8D9B0", "cursor": "#C9A227" }
}
```

- [ ] **Paso 6: Validar**

Run: `cargo run -p motor --example validar_tema -- D:\Temas\game-of-thrones`
Expected: `OK: Game of Thrones — advertencias: []`

---

### Tarea 15: pruebas de ida y vuelta contra Windows real

**Archivos:**
- Crear: `motor/tests/ida_vuelta.rs`

Estas pruebas cambian la configuración real por unos segundos y la restauran. Van con `#[ignore]` para que `cargo test` normal no las corra. Usan `D:\TemasApp\pruebas\` como raíz de estado (no el estado real de la app) y el tema real de Star Wars.

- [ ] **Paso 1: Escribir las pruebas**

```rust
//! Pruebas contra Windows real: cambian la configuración y la restauran.
//! Correr SÓLO con permiso del usuario:
//!   cargo test -p motor --test ida_vuelta -- --ignored --test-threads=1

use motor::estado::Rutas;
use motor::modulos::bloqueo::{self, Bloqueo};
use motor::modulos::colores::{self, Colores};
use motor::modulos::fondos::Fondos;
use motor::modulos::Modulo;
use motor::nucleo::Motor;
use motor::registro::{self, RegValor};
use motor::rotador::Rotador;
use motor::tema::Tema;
use motor::win;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

fn tema_sw() -> Tema {
    Tema::cargar(Path::new(r"D:\Temas\star-wars")).expect("falta el tema Star Wars (Tarea 13)")
}

/// Estado aislado para pruebas, que persiste (el fondo restaurado queda apuntando ahí).
fn rutas_prueba() -> Rutas {
    let r = Rutas::nuevo(r"D:\TemasApp\pruebas");
    let _ = std::fs::remove_file(r.original());
    let _ = std::fs::remove_file(r.estado());
    let _ = std::fs::remove_dir_all(r.raiz.join("original"));
    std::fs::create_dir_all(&r.raiz).unwrap();
    r
}

#[test]
#[ignore]
fn colores_ida_y_vuelta() {
    let antes = registro::capturar_valores(&colores::CLAVES).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let m = Colores;
    let captura = m.capturar(dir.path()).unwrap();
    m.aplicar(&tema_sw()).unwrap();
    assert_eq!(
        registro::leer(colores::DWM, "AccentColor").unwrap(),
        RegValor::Dword(motor::color::Rgb::desde_hex("#FFE81F").unwrap().abgr())
    );
    std::thread::sleep(Duration::from_secs(2));
    m.restaurar(&captura, dir.path()).unwrap();
    assert_eq!(registro::capturar_valores(&colores::CLAVES).unwrap(), antes);
}

#[test]
#[ignore]
fn bloqueo_dos_ciclos_con_motor() {
    let tema = tema_sw();
    if !tema.define("bloqueo") {
        eprintln!("el tema no define bloqueo (spike fallido): se saltea");
        return;
    }
    let rutas = rutas_prueba();
    let motor = Motor::nuevo(rutas.clone(), vec![Box::new(Bloqueo)]);
    let antes = registro::capturar_valores(&bloqueo::CLAVES).unwrap();
    for ciclo in 1..=2 {
        let inf = motor.aplicar(&tema, &[]).unwrap();
        assert!(inf.errores.is_empty(), "ciclo {ciclo}: {inf:?}");
        assert_eq!(registro::leer(bloqueo::CDM, "RotatingLockScreenEnabled").unwrap(), RegValor::Dword(0));

        let inf = motor.restaurar_original().unwrap();
        assert!(inf.errores.is_empty(), "ciclo {ciclo}: {inf:?}");
        assert_eq!(registro::capturar_valores(&bloqueo::CLAVES).unwrap(), antes, "ciclo {ciclo}");
        // La captura quedó archivada en original.prev con su imagen (se compara existencia, no bytes).
        let prev = rutas.dir_original_prev("bloqueo");
        let hay_imagen = std::fs::read_dir(&prev)
            .map(|d| d.filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().starts_with("bloqueo.")))
            .unwrap_or(false);
        assert!(hay_imagen, "ciclo {ciclo}: falta la imagen en {}", prev.display());
    }
}

#[test]
#[ignore]
fn fondos_dos_ciclos_con_motor() {
    let rutas = rutas_prueba();
    let rotador = Arc::new(Rotador::nuevo(|p| win::poner_fondo(p)));
    let fondos = Fondos { rotador: rotador.clone(), raiz: rutas.raiz.clone() };
    let motor = Motor::nuevo(rutas.clone(), vec![Box::new(fondos)]);
    let tema = tema_sw();
    let estilo_antes = registro::leer(r"Control Panel\Desktop", "WallpaperStyle").unwrap();

    for ciclo in 1..=2 {
        let inf = motor.aplicar(&tema, &[]).unwrap();
        assert!(inf.errores.is_empty(), "ciclo {ciclo}: {inf:?}");
        std::thread::sleep(Duration::from_secs(1));
        let actual = win::fondo_actual().unwrap();
        assert!(actual.starts_with(r"D:\Temas\star-wars\fondos"), "ciclo {ciclo}: {}", actual.display());

        let inf = motor.restaurar_original().unwrap();
        assert!(inf.errores.is_empty(), "ciclo {ciclo}: {inf:?}");
        assert!(!rotador.activo());
        let actual = win::fondo_actual().unwrap();
        assert!(actual.starts_with(&rutas.raiz), "ciclo {ciclo}: {}", actual.display());
        assert!(actual.is_file(), "ciclo {ciclo}: el fondo restaurado existe");
        assert_eq!(registro::leer(r"Control Panel\Desktop", "WallpaperStyle").unwrap(), estilo_antes);
    }
}
```

- [ ] **Paso 2: Compilar sin correr**

Run: `cargo test -p motor --test ida_vuelta --no-run`
Expected: compila.

- [ ] **Paso 3: 🛑 CHECKPOINT (usuario) — correr contra Windows real**

Pedir confirmación: "Voy a correr pruebas que cambian por unos segundos tus colores, pantalla de bloqueo y fondo, y los restauran. Al terminar, el fondo queda en una copia del fondo que tengas ahora. ¿Dale?". Con el sí:

Run: `cargo test -p motor --test ida_vuelta -- --ignored --test-threads=1`
Expected: 3 tests PASS. Si falla alguno, usar superpowers:systematic-debugging antes de tocar código.

- [ ] **Paso 4: Commit**

```bash
git add motor/tests
git commit -m "test(motor): ida y vuelta contra Windows real"
```

---

### Tarea 16: app Tauri mínima

**Archivos:**
- Crear: `package.json`, `ui/index.html`, `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/icons/*`
- Modificar: `Cargo.toml` (workspace)

- [ ] **Paso 1: Agregar `src-tauri` al workspace**

`Cargo.toml`:
```toml
[workspace]
members = ["motor", "src-tauri"]
resolver = "2"
```

- [ ] **Paso 2: Crear `package.json` e instalar el CLI**

```json
{
  "name": "temas-windows",
  "private": true,
  "scripts": {
    "tauri": "tauri"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2"
  }
}
```

Run: `npm install`
Expected: crea `node_modules/` y `package-lock.json` dentro del proyecto (en D:).

- [ ] **Paso 3: Crear `ui/index.html`**

```html
<!doctype html>
<html lang="es">
  <head><meta charset="utf-8"><title>Temas</title></head>
  <body>La galería llega en la etapa 2.</body>
</html>
```

- [ ] **Paso 4: Generar el ícono fuente y los íconos (PowerShell)**

```powershell
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 1024, 1024
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'; $g.TextRenderingHint = 'AntiAlias'
$g.Clear([System.Drawing.Color]::FromArgb(14, 14, 16))
$g.FillEllipse((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(201, 162, 39))), 112, 112, 800, 800)
$font = New-Object System.Drawing.Font 'Segoe UI', 440, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$fmt = New-Object System.Drawing.StringFormat; $fmt.Alignment = 'Center'; $fmt.LineAlignment = 'Center'
$g.DrawString('T', $font, [System.Drawing.Brushes]::Black, (New-Object System.Drawing.RectangleF 0, 0, 1024, 1024), $fmt)
New-Item -ItemType Directory -Force src-tauri\icons | Out-Null
$bmp.Save("$PWD\src-tauri\icons\fuente.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
```

Run: `npx tauri icon src-tauri/icons/fuente.png`
Expected: genera `src-tauri/icons/32x32.png`, `128x128.png`, `icon.ico`, etc.

- [ ] **Paso 5: Crear `src-tauri/Cargo.toml`**

```toml
[package]
name = "temas-windows"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-single-instance = "2"
tauri-plugin-dialog = "2"
motor = { path = "../motor" }
anyhow = "1"
```

- [ ] **Paso 6: Crear `src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Paso 7: Crear `src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Temas Windows",
  "version": "0.1.0",
  "identifier": "ar.nahuel.temaswindows",
  "build": {
    "frontendDist": "../ui"
  },
  "app": {
    "windows": [],
    "security": { "csp": null }
  },
  "bundle": {
    "active": false,
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.ico"]
  }
}
```

- [ ] **Paso 8: Crear `src-tauri/src/main.rs` mínimo (sin bandeja todavía)**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .plugin(tauri_plugin_dialog::init())
        .build(tauri::generate_context!())
        .expect("error al iniciar la app")
        .run(|_, evento| {
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = evento {
                api.prevent_exit();
            }
        });
}
```

- [ ] **Paso 9: Compilar**

Run: `cargo build -p temas-windows`
Expected: compila (la primera vez tarda varios minutos).

- [ ] **Paso 10: Commit**

```bash
git add Cargo.toml package.json package-lock.json ui src-tauri
git commit -m "chore: app Tauri mínima"
```

---

### Tarea 17: bandeja del sistema

**Archivos:**
- Crear: `src-tauri/src/app.rs`, `src-tauri/src/bandeja.rs`
- Modificar: `src-tauri/src/main.rs`

La lógica está en `motor` (ya testeada). Esta capa se verifica a mano en la Tarea 18.

- [ ] **Paso 1: Crear `src-tauri/src/app.rs`**

```rust
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
        let rotador = Arc::new(Rotador::nuevo(|p| win::poner_fondo(p)));
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
```

- [ ] **Paso 2: Crear `src-tauri/src/bandeja.rs`**

```rust
//! Ícono y menú de la bandeja del sistema.

use crate::app::App;
use motor::nucleo::Informe;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

const ID: &str = "principal";

fn construir_menu(h: &AppHandle, app: &App) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(h)?;
    let activo = app.estado().tema_activo;
    let temas = app.temas();
    if temas.is_empty() {
        menu.append(&MenuItem::with_id(h, "nada", "(no hay temas en la carpeta)", false, None::<&str>)?)?;
    }
    for (id, t) in &temas {
        match t {
            Ok(t) => menu.append(&CheckMenuItem::with_id(
                h,
                format!("tema:{id}"),
                &t.conf.nombre,
                true,
                activo.as_deref() == Some(id.as_str()),
                None::<&str>,
            )?)?,
            Err(_) => menu.append(&MenuItem::with_id(
                h,
                format!("roto:{id}"),
                format!("{id} (tema.json con errores)"),
                true,
                None::<&str>,
            )?)?,
        }
    }
    menu.append(&PredefinedMenuItem::separator(h)?)?;
    let rota = app.rotador.activo();
    let pausa = if app.rotador.pausado() { "Reanudar fondos" } else { "Pausar fondos" };
    menu.append(&MenuItem::with_id(h, "siguiente", "Siguiente fondo", rota, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(h, "pausa", pausa, rota, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(h, "restaurar", "Restaurar original", true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(h)?)?;
    menu.append(&MenuItem::with_id(h, "salir", "Salir", true, None::<&str>)?)?;
    Ok(menu)
}

fn tooltip(app: &App) -> String {
    match app.nombre_activo() {
        Some(n) => format!("Temas — {n}"),
        None => "Temas — original".into(),
    }
}

pub fn crear(h: &AppHandle) -> tauri::Result<()> {
    let app = h.state::<App>();
    let icono = h.default_window_icon().expect("falta el ícono de la app").clone();
    TrayIconBuilder::with_id(ID)
        .icon(icono)
        .tooltip(tooltip(&app))
        .menu(&construir_menu(h, &app)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|h, ev| manejar(h, ev.id().as_ref()))
        .build(h)?;
    Ok(())
}

fn refrescar(h: &AppHandle) {
    let h2 = h.clone();
    let _ = h.run_on_main_thread(move || {
        let app = h2.state::<App>();
        if let (Some(tray), Ok(menu)) = (h2.tray_by_id(ID), construir_menu(&h2, &app)) {
            let _ = tray.set_menu(Some(menu));
            let _ = tray.set_tooltip(Some(tooltip(&app)));
        }
    });
}

fn avisar(h: &AppHandle, titulo: &str, texto: String, tipo: MessageDialogKind) {
    h.dialog().message(texto).title(titulo).kind(tipo).show(|_| {});
}

fn informar(h: &AppHandle, r: anyhow::Result<Informe>) {
    match r {
        Ok(inf) if inf.errores.is_empty() && inf.advertencias.is_empty() => {}
        Ok(inf) => avisar(h, "Temas — hubo problemas", inf.resumen(), MessageDialogKind::Warning),
        Err(e) => avisar(h, "Temas — error", format!("{e:#}"), MessageDialogKind::Error),
    }
}

fn manejar(h: &AppHandle, id: &str) {
    let h = h.clone();
    let id = id.to_string();
    // Aplicar puede tardar (copias, WinRT): fuera del hilo de la interfaz.
    std::thread::spawn(move || {
        let app = h.state::<App>();
        if let Some(t) = id.strip_prefix("tema:") {
            informar(&h, app.aplicar(t));
        } else if let Some(t) = id.strip_prefix("roto:") {
            avisar(&h, "Temas — tema con errores", app.error_de(t), MessageDialogKind::Error);
        } else {
            match id.as_str() {
                "siguiente" => app.rotador.siguiente(),
                "pausa" => app.alternar_pausa(),
                "restaurar" => informar(&h, app.restaurar()),
                "salir" => {
                    h.exit(0);
                    return;
                }
                _ => {}
            }
        }
        refrescar(&h);
    });
}
```

Si alguna firma de Tauri 2 difiere (por ejemplo `show_menu_on_left_click` o `CheckMenuItem::with_id`), consultar https://docs.rs/tauri/2/ y adaptar sin cambiar el comportamiento.

- [ ] **Paso 3: Reemplazar `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bandeja;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .plugin(tauri_plugin_dialog::init())
        .setup(|a| {
            let estado = app::App::nuevo()?;
            if let Err(e) = estado.retomar() {
                eprintln!("no se pudo retomar la rotación: {e:#}");
            }
            a.manage(estado);
            bandeja::crear(a.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al iniciar la app")
        .run(|_, evento| {
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = evento {
                api.prevent_exit();
            }
        });
}
```

- [ ] **Paso 4: Compilar**

Run: `cargo build -p temas-windows`
Expected: compila sin errores.

- [ ] **Paso 5: Commit**

```bash
git add src-tauri/src
git commit -m "feat: bandeja del sistema con temas, fondos y restaurar"
```

---

### Tarea 18: verificación final

- [ ] **Paso 1: Suite completa**

Run: `cargo test -p motor`
Expected: todos PASS (los de `ida_vuelta` aparecen como ignorados).

- [ ] **Paso 2: Build de release**

Run: `npx tauri build --no-bundle`
Expected: `target/release/temas-windows.exe`.

- [ ] **Paso 3: 🛑 CHECKPOINT (usuario) — prueba completa en vivo**

Pedir confirmación para abrir la app y aplicar temas. Con el sí, lanzar `target\release\temas-windows.exe` (PowerShell: `Start-Process .\target\release\temas-windows.exe`) y recorrer con el usuario:

| # | Acción | Qué verificar |
|---|---|---|
| 1 | Clic en el ícono de la bandeja | Aparecen "Game of Thrones" y "Star Wars", ninguno marcado; tooltip "Temas — original" |
| 2 | Elegir **Star Wars** | Fondo SW; cambia a los 20 s; acento amarillo en barra/Inicio; Windows Terminal nueva pestaña con fondo negro y texto amarillo; Win+L muestra la imagen elegida |
| 3 | **Siguiente fondo** | Cambia al instante |
| 4 | **Pausar fondos**, esperar 40 s | No cambia; el menú dice "Reanudar fondos" |
| 5 | **Reanudar fondos** | Vuelve a rotar |
| 6 | Elegir **Game of Thrones** | Fondo, acento dorado, terminal y bloqueo de GoT; menú marca GoT |
| 7 | Cerrar la app con **Salir** y volver a abrirla | Sigue GoT marcado y la rotación retoma sola |
| 8 | **Restaurar original** | Fondo por defecto de Windows, colores y terminal como antes de empezar; Win+L muestra la pantalla de bloqueo original; menú sin marca |
| 9 | Romper a propósito una copia de tema (`D:\Temas\roto\tema.json` con `{"nombre": ""}`) | Aparece "roto (tema.json con errores)"; al elegirlo, diálogo con el error. Después borrar `D:\Temas\roto` (con confirmación) |

Sacar capturas del escritorio en los pasos 2, 6 y 8 y guardarlas **sólo** en `D:\proyects\navi\tmp\` (regla del usuario), por ejemplo con:
```powershell
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
[System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$bmp.Save("D:\proyects\navi\tmp\temas-paso-2.png"); $bmp.Dispose()
```

Cualquier falla: usar superpowers:systematic-debugging, corregir con test si la causa está en `motor`, y repetir la fila.

- [ ] **Paso 4: Actualizar el spec**

Marcar la etapa 1 como terminada en "Etapas" con la fecha y cualquier desvío (por ejemplo, si `bloqueo` se pospuso).

- [ ] **Paso 5: Commit final**

```bash
git add docs
git commit -m "docs: etapa 1 terminada"
```

- [ ] **Paso 6: Avisar al usuario** cómo abrir la app (`D:\proyects\temas-windows\target\release\temas-windows.exe`), que en esta etapa no arranca sola con Windows (llega en la etapa 2) y que `D:\Wallpapers\rotar.ps1` ya no se usa (no se borra).
