# Selector de temas para Windows 11 — Diseño

Fecha: 2026-10-08
Estado: aprobado en brainstorming, pendiente de revisión del documento

## Objetivo

Una app de escritorio que aplica "temas" temáticos (Star Wars, Game of Thrones, etc.) a Windows 11.
Cada tema cambia de una vez: fondos rotativos, colores del sistema, pantalla de bloqueo, sonidos,
esquema de la terminal, íconos del escritorio, protector de pantalla, intro musical y widget.
Todo es reversible con un botón "Restaurar original".

Reemplaza al script actual `D:\Wallpapers\rotar.ps1`.

## Restricciones

- Windows 11, usuario sin privilegios de administrador: todo se hace en `HKCU` o APIs de usuario.
- Todo lo instalado va en el disco `D:`.
- Toda descarga de contenido (imágenes, sonidos, íconos, fuentes) e instalación (Rainmeter) se
  confirma con el usuario antes de hacerla.
- Toolchain disponible sin instalar nada: Node 22, Rust stable MSVC, VS 2019 Build Tools, WebView2.

## Stack

- **Tauri 2** (Rust) para el motor, la bandeja del sistema y la ventana.
- **React + Vite + Tailwind** para la galería.
- Crates previstos: `windows` (Win32/WinRT), `winreg`, `serde`/`serde_json`, `rodio` o audio vía webview,
  `tauri-plugin-notification`, `tauri-plugin-autostart`.

## Ubicaciones

| Qué | Dónde |
|---|---|
| Código de la app | `D:\proyects\temas-windows\` |
| Temas | `D:\Temas\<id>\` (configurable en Ajustes) |
| Foto del estado original | `D:\Temas\_original.json` |
| Tema activo y ajustes | `D:\Temas\_estado.json` |

La carpeta actual `D:\Wallpapers\StarWars` se **mueve** (previa confirmación) a `D:\Temas\star-wars\fondos\`,
y se detiene el proceso de `rotar.ps1`.

## Estructura de un tema

```
D:\Temas\<id>\
├── tema.json        configuración (obligatorio)
├── portada.jpg      imagen de la tarjeta en la galería
├── fondos\          wallpapers que rotan (.jpg, .jpeg, .png, .bmp)
├── bloqueo.jpg      pantalla de bloqueo
├── sonidos\         .wav
├── intro.mp3        audio corto al aplicar
├── iconos\          .ico
└── widget\          skin de Rainmeter
```

### `tema.json`

Sólo `nombre` es obligatorio. Cada sección ausente significa "este tema no toca esa parte de Windows";
esa parte queda como esté en ese momento.

```json
{
  "nombre": "Game of Thrones",
  "colores": { "acento": "#C9A227", "modo": "oscuro", "transparencia": true, "acentoEnBarra": true },
  "fondos": { "intervaloSeg": 20, "orden": "aleatorio", "estilo": "rellenar" },
  "bloqueo": "bloqueo.jpg",
  "sonidos": { "logon": "sonidos/logon.wav", "notificacion": "sonidos/cuervo.wav", "error": "sonidos/espada.wav" },
  "intro": { "audio": "intro.mp3", "frases": ["Se acerca el invierno."] },
  "terminal": { "fondo": "#141414", "texto": "#E8D9B0", "cursor": "#C9A227" },
  "protector": { "tipo": "presentacion", "minutos": 10 },
  "iconos": { "papeleraVacia": "iconos/papelera.ico", "papeleraLlena": "iconos/papelera-llena.ico", "esteEquipo": "iconos/stark.ico" },
  "widget": { "skin": "widget" }
}
```

Valores admitidos:
- `colores.modo`: `"oscuro"` | `"claro"`.
- `fondos.orden`: `"aleatorio"` | `"alfabetico"`. `fondos.estilo`: `"rellenar"` | `"ajustar"` | `"expandir"` | `"centrar"`.
  `fondos.intervaloSeg`: entero ≥ 5 (por defecto 20).
- `sonidos`: claves lógicas mapeadas a eventos de Windows — `logon` → `WindowsLogon`, `logoff` → `WindowsLogoff`,
  `notificacion` → `Notification.Default`, `error` → `SystemHand`, `advertencia` → `SystemExclamation`,
  `papelera` → `EmptyRecycleBin`.
- `protector.tipo`: `"presentacion"` (fondos del tema) | `"crawl"` (texto inclinado estilo Star Wars, requiere `protector.texto`).
- Rutas siempre relativas a la carpeta del tema.

Un `tema.json` inválido no rompe la app: el tema aparece en la galería marcado con el error y no se puede aplicar.

## Motor (Rust)

### Módulos

Cada módulo implementa la misma interfaz:

```rust
trait Modulo {
    fn id(&self) -> &'static str;
    fn capturar(&self) -> Result<serde_json::Value>;        // estado actual de Windows
    fn aplicar(&self, tema: &Tema) -> Result<()>;
    fn restaurar(&self, captura: &serde_json::Value) -> Result<()>;
}
```

| Módulo | Qué toca | Mecanismo |
|---|---|---|
| `fondos` | Wallpaper rotativo | Hilo de la app que llama `SystemParametersInfoW(SPI_SETDESKWALLPAPER)` cada N s; estilo en `HKCU\Control Panel\Desktop\WallpaperStyle`/`TileWallpaper`. |
| `colores` | Acento, modo, transparencia, acento en barra | `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` (`AppsUseLightTheme`, `SystemUsesLightTheme`, `EnableTransparency`, `ColorPrevalence`), `HKCU\Software\Microsoft\Windows\DWM` (`AccentColor`, `ColorizationColor`, `ColorPrevalence`), `HKCU\...\Explorer\Accent` (`AccentPalette`, `AccentColorMenu`, `StartColorMenu`); luego `WM_SETTINGCHANGE` con `"ImmersiveColorSet"`. |
| `bloqueo` | Pantalla de bloqueo | WinRT `Windows.System.UserProfile.LockScreen.SetImageFileAsync`. Captura: copia de la imagen actual vía `LockScreen.GetImageStream`. |
| `sonidos` | Sonidos del sistema | `HKCU\AppEvents\Schemes\Apps\.Default\<evento>\.Current` (valor por defecto = ruta .wav); `HKCU\AppEvents\Schemes` = `.Current`. |
| `terminal` | Windows Terminal | Edita `%LOCALAPPDATA%\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json`: agrega/actualiza el esquema `"Tema activo"` y fija `profiles.defaults.colorScheme`. Hace copia `settings.json.bak` antes del primer cambio. Si no existe el archivo, el módulo se saltea con aviso. |
| `iconos` | Papelera y Este equipo | `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\CLSID\{645FF040-5081-101B-9F08-00AA002F954E}\DefaultIcon` (`empty`, `full`) y `{20D04FE0-3AEA-1069-A2D8-08002B30309D}\DefaultIcon`; luego `SHChangeNotify(SHCNE_ASSOCCHANGED)`. |
| `intro` | Música + notificación | La ventana/webview reproduce el audio (volumen de Ajustes); notificación con una frase al azar de `intro.frases`. No se captura ni restaura. |
| `protector` | Protector de pantalla | La app copiada como `temas-protector.scr` en `D:\Temas\_bin\`, que atiende `/s` (pantalla completa con fondos o crawl), `/c` (abre Ajustes) y `/p` (ignorado). `HKCU\Control Panel\Desktop` → `SCRNSAVE.EXE`, `ScreenSaveActive`, `ScreenSaveTimeOut`. |
| `widget` | Reloj + cita del día | `Rainmeter.exe !ActivateConfig` con la skin del tema; desactiva la del tema anterior. Si Rainmeter no está instalado, se saltea con aviso. |

### Flujo "Aplicar tema"

1. Si no existe `_original.json`: correr `capturar()` de todos los módulos y guardarlo. Nunca se sobrescribe después.
2. Validar `tema.json`.
3. Para cada módulo que el tema define y que el usuario no desmarcó: `aplicar()`. Los errores se juntan; un módulo que falla no frena a los demás.
4. Guardar tema activo y módulos aplicados en `_estado.json`.
5. Intro (si está activada en Ajustes).
6. Devolver a la UI la lista de módulos ok / fallidos con el mensaje de error.

Al cambiar de un tema a otro, los módulos que el tema nuevo **no** define se restauran desde `_original.json`,
para que no queden restos del tema anterior (ej.: íconos de Star Wars con el tema de GoT sin íconos).

### Flujo "Restaurar original"

`restaurar()` de cada módulo con su captura de `_original.json`, detener la rotación, limpiar el tema activo
en `_estado.json`. `_original.json` se conserva.

### Persistencia entre reinicios

Colores, sonidos, íconos, terminal, bloqueo y protector viven en el registro/archivos y persisten.
La rotación de fondos sólo corre con la app abierta. Ajuste "Iniciar con Windows" (desactivado por defecto)
vía `tauri-plugin-autostart`; al arrancar, la app retoma la rotación del tema activo sin reaplicar el resto.

## Selector (UI)

Mockup aprobado: `D:\Wallpapers\.superpowers\brainstorm\1728-1791481412\galeria.html`.

### Bandeja del sistema

Ícono con la sigla/ícono del tema activo. Menú:
- Lista de temas (el activo marcado) → aplica con todos sus módulos.
- Siguiente fondo · Pausar/Reanudar fondos · Abrir galería… · Restaurar original · Salir.

Cerrar la ventana de la galería no cierra la app; sólo "Salir" la termina.

### Galería

- Ventana ~1100×700, siempre oscura, con el color de acento del tema activo.
- Grilla de tarjetas (portada + nombre, marca de activo, marca de error si el `tema.json` es inválido) + tarjeta "+ Nuevo tema".
- Panel de detalle del tema seleccionado: carrusel de fondos, muestras de color, casilla por módulo
  (deshabilitada si el tema no lo define), botones ▶ para escuchar sonidos e intro, botón "Aplicar tema".
- Tras aplicar, muestra el resultado por módulo (ok / error con mensaje).
- La galería se actualiza sola al cambiar el contenido de la carpeta de temas (file watcher).

### Nuevo tema

Pide un nombre, crea `D:\Temas\<id>\` con `tema.json` de ejemplo y subcarpetas vacías, y la abre en el Explorador.
`<id>` = nombre en minúsculas, sin acentos, espacios → guiones.

### Ajustes

Iniciar con Windows (off por defecto) · carpeta de temas · reproducir intro (on) · volumen de intro.

## Contenido inicial

- **Star Wars**: los 14 fondos ya descargados (migrados).
- **Game of Thrones**: fondos de Wallhaven (misma búsqueda: SFW, 16:9, ≥ 2560×1440).
- Bloqueo y portada: una imagen de los fondos de cada tema, elegida con el usuario.
- Sonidos, intro, íconos, fuentes: se buscan fuentes y se confirman con el usuario, o los aporta él.
  Uso personal únicamente.

## Etapas

Cada etapa termina en algo usable.

1. **Núcleo + bandeja**: motor (`fondos`, `colores`, `bloqueo`, `terminal`), captura/restauración, menú de bandeja,
   temas Star Wars (migrado) y Game of Thrones.
2. **Galería**: ventana, detalle, casillas por módulo, "Nuevo tema", Ajustes (incluye autostart).
3. **Sonidos + intro.**
4. **Íconos + protector de pantalla** (presentación y crawl).
5. **Widget** (Rainmeter en `D:`, previa confirmación; fuentes por usuario).

## Pruebas

- **Unitarias (Rust)**: parseo y validación de `tema.json` (válido, faltantes, valores fuera de rango, rutas inexistentes);
  lectura/escritura de `_original.json` y `_estado.json`; generación de `<id>` desde el nombre; mapeo de claves de sonidos.
- **Ida y vuelta por módulo** (integración, corre en la máquina real): `capturar` → `aplicar` → verificar que cambió →
  `restaurar` → verificar que el estado es idéntico a la captura. Es la garantía de "Restaurar original".
- **Terminal**: pruebas sobre una copia temporal de `settings.json`, nunca sobre el real.
- **Visual**: capturas del escritorio al cierre de cada etapa.

## Fuera de alcance

- Cambiar la fuente del sistema (riesgoso).
- Navi con personalidad temática.
- Rotación automática de temas por día/hora.
- Editor de temas dentro de la app (posible a futuro).
- Cursores.
