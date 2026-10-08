# Selector de temas para Windows 11 — Diseño

Fecha: 2026-10-08
Estado: aprobado en brainstorming, revisado (v2)

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
- Crates previstos: `windows` (Win32/WinRT), `winreg`, `serde`/`serde_json`, `jsonc-parser` (con `cst`),
  `rodio`, `notify` (file watcher), `tauri-plugin-notification`, `tauri-plugin-autostart`, `tauri-plugin-single-instance`.

## Ubicaciones

| Qué | Dónde |
|---|---|
| Código de la app | `D:\proyects\temas-windows\` |
| App instalada (instalador NSIS, desde la etapa 2) | `D:\Programas\TemasWindows\` |
| Estado de la app (fijo) | `D:\TemasApp\` |
| ├ Ajustes | `D:\TemasApp\ajustes.json` |
| ├ Tema activo y módulos tocados | `D:\TemasApp\estado.json` |
| ├ Captura del estado original | `D:\TemasApp\original.json` + archivos en `D:\TemasApp\original\` |
| └ Binarios auxiliares (`.scr`) | `D:\TemasApp\bin\` |
| Temas (configurable en Ajustes) | `D:\Temas\<id>\` por defecto |

En la etapa 1 la app corre desde el build de desarrollo / release local; el instalador llega en la etapa 2
(necesario para autostart estable y para que las notificaciones tengan AppUserModelID).

### Migración desde `rotar.ps1` (etapa 1, previa confirmación)

1. Detener el proceso `powershell.exe` cuya línea de comandos contiene `rotar.ps1` (no tiene autoarranque).
2. **Mover** `D:\Wallpapers\StarWars\*` a `D:\Temas\star-wars\fondos\`.
3. Sembrar la entrada `fondos` de `original.json` a mano, no con `capturar()`: el fondo actual es de Star Wars
   y el fondo real del usuario (`Downloads\90b68b52-....png`, según `D:\Wallpapers\fondo_original.txt`) ya no existe.
   Decisión del usuario: el original es el fondo por defecto de Windows 11,
   `C:\Windows\Web\Wallpaper\Windows\img0.jpg`, copiado a `D:\TemasApp\original\fondo.jpg`, con `WallpaperStyle=10`.

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

Sólo `nombre` es obligatorio. Una sección ausente significa que el tema no define esa parte
(ver "Qué pasa con lo que el tema no aplica").

```json
{
  "nombre": "Game of Thrones",
  "colores": { "acento": "#C9A227", "modo": "oscuro", "transparencia": true, "acentoEnBarra": true },
  "fondos": { "intervaloSeg": 20, "orden": "aleatorio", "estilo": "rellenar" },
  "bloqueo": "bloqueo.jpg",
  "sonidos": { "notificacion": "sonidos/cuervo.wav", "error": "sonidos/espada.wav" },
  "intro": { "audio": "intro.mp3", "frases": ["Se acerca el invierno."] },
  "terminal": { "fondo": "#141414", "texto": "#E8D9B0", "cursor": "#C9A227" },
  "protector": { "tipo": "presentacion", "minutos": 10 },
  "iconos": { "papeleraVacia": "iconos/papelera.ico", "papeleraLlena": "iconos/papelera-llena.ico", "esteEquipo": "iconos/stark.ico" },
  "widget": { "skin": "widget", "archivo": "Reloj.ini" }
}
```

Valores admitidos:
- `colores.modo`: `"oscuro"` | `"claro"`. `acentoEnBarra: true` sólo tiene efecto en modo oscuro;
  con `"claro"` se ignora y la validación lo marca como advertencia.
- `fondos.orden`: `"aleatorio"` | `"alfabetico"`. `fondos.intervaloSeg`: entero ≥ 5 (por defecto 20).
  `fondos.estilo` → `WallpaperStyle`: `rellenar`=10, `ajustar`=6, `expandir`=2, `centrar`=0 (siempre `TileWallpaper=0`).
- `sonidos`: claves lógicas → eventos de Windows:
  `notificacion` → `Notification.Default`, `correo` → `Notification.Mail`, `error` → `SystemHand`,
  `advertencia` → `SystemExclamation`, `papelera` → `EmptyRecycleBin`,
  `conectar` → `DeviceConnect`, `desconectar` → `DeviceDisconnect`.
  (El sonido de inicio de Windows 11 no es configurable por usuario sin admin; queda fuera.)
- `terminal`: `fondo`, `texto`, `cursor` obligatorios dentro de la sección; opcional `ansi` con cualquiera de los
  16 colores (`black`, `red`, …, `brightWhite`). Los que falten se toman del esquema **Campbell**.
  `selectionBackground` = `cursor` si no se indica.
- `protector.tipo`: `"presentacion"` (fondos del tema) | `"crawl"` (texto inclinado estilo Star Wars, requiere `protector.texto`).
- Rutas siempre relativas a la carpeta del tema.

Un `tema.json` inválido no rompe la app: el tema aparece en la galería marcado con el error y no se puede aplicar.

## Motor (Rust)

### Módulos

```rust
trait Modulo {
    fn id(&self) -> &'static str;
    /// Estado actual de Windows para este módulo. Puede copiar archivos a `dir_original`.
    fn capturar(&self, dir_original: &Path) -> Result<serde_json::Value>;
    fn aplicar(&self, tema: &Tema) -> Result<()>;
    fn restaurar(&self, captura: &serde_json::Value) -> Result<()>;
}
```

| Módulo | Qué toca | Mecanismo | Captura / restauración |
|---|---|---|---|
| `fondos` | Wallpaper rotativo | Escribe `WallpaperStyle`/`TileWallpaper` en `HKCU\Control Panel\Desktop` y luego `SystemParametersInfoW(SPI_SETDESKWALLPAPER)`; un hilo rota cada N s. | Copia del fondo actual a `original\fondo.<ext>` + estilo; si la ruta que informa Windows ya no existe, copia `%APPDATA%\Microsoft\Windows\Themes\TranscodedWallpaper`. Restaurar detiene la rotación, **copia** la imagen capturada a `D:\TemasApp\fondo-actual.<ext>` (ruta estable, fuera de `original\`) y pone esa. |
| `colores` | Acento, modo, transparencia, acento en barra | `HKCU\...\Themes\Personalize` (`AppsUseLightTheme`, `SystemUsesLightTheme`, `EnableTransparency`, `ColorPrevalence`); `HKCU\Software\Microsoft\Windows\DWM` (`AccentColor`, `ColorizationColor`, `ColorPrevalence`); `HKCU\...\Explorer\Accent` (`AccentPalette`, `AccentColorMenu`, `StartColorMenu`); **`HKCU\Control Panel\Desktop\AutoColorization = 0`** (si no, Windows pisa el acento en cada cambio de fondo). `AccentPalette` (32 bytes = 8 colores RGBA) se genera del `acento` variando luminosidad HSL: +30 %, +20 %, +10 %, base, −10 %, −20 %, −30 % y un 8.º tono gris (`#696969`); `AccentColorMenu` = base, `StartColorMenu` = −20 % (valores exactos a confirmar en el spike de colores). Orden de bytes: `AccentColor`, `AccentColorMenu`, `StartColorMenu` y `AccentPalette` en **ABGR**; `DWM\ColorizationColor` en **ARGB**. Luego `WM_SETTINGCHANGE` con `"ImmersiveColorSet"`. | Todos esos valores tal cual (los `REG_BINARY` en hex). |
| `bloqueo` | Pantalla de bloqueo | WinRT `LockScreen.SetImageFileAsync`; además apaga Spotlight: `HKCU\...\ContentDeliveryManager` `RotatingLockScreenEnabled=0`, `RotatingLockScreenOverlayEnabled=0`. | Valores de `ContentDeliveryManager` + copia de la imagen actual (`LockScreen.GetImageStream`) a `original\bloqueo.<ext>`. Si Spotlight estaba activo, restaurar lo reactiva en vez de fijar la imagen. |
| `sonidos` | Sonidos del sistema | `HKCU\AppEvents\Schemes\Apps\.Default\<evento>\.Current` (valor por defecto = ruta .wav); `HKCU\AppEvents\Schemes` = `.Current`. | `.Current` de cada evento mapeado + valor de `Schemes`. |
| `terminal` | Windows Terminal | Busca `settings.json` en: `Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\`, `Packages\Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe\LocalState\` y `%LOCALAPPDATA%\Microsoft\Windows Terminal\`; aplica a todos los que existan. Es JSONC: se edita con `jsonc-parser` (CST) **preservando comentarios**. Agrega/reemplaza el esquema `"Tema activo"` y fija `profiles.defaults.colorScheme`. Perfiles con `colorScheme` propio no se tocan (aceptado). Backup `settings.json.bak` antes del primer cambio. Si no hay ninguno, el módulo no hace nada (captura vacía), sin error. | Por cada `settings.json` encontrado: su ruta y el valor previo de `profiles.defaults.colorScheme` (o "ausente"). Restaurar borra el esquema `"Tema activo"` y repone o elimina esa clave. |
| `iconos` | Papelera y Este equipo | `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\CLSID\{645FF040-5081-101B-9F08-00AA002F954E}\DefaultIcon` (`(Default)`, `empty`, `full`) y `{20D04FE0-3AEA-1069-A2D8-08002B30309D}\DefaultIcon`; luego `SHChangeNotify(SHCNE_ASSOCCHANGED)`. | Valores previos o "clave ausente" (restaurar la borra). |
| `intro` | Música + notificación | Audio con `rodio` desde Rust (no depende de que exista la ventana); notificación con una frase al azar de `intro.frases`. | No se captura ni restaura. |
| `protector` | Protector de pantalla | El exe de la app copiado como `D:\TemasApp\bin\temas-protector.scr` (se recopia en cada arranque de la app si cambió). El exe evalúa `/s` (pantalla completa: fondos o crawl), `/c` (abre Ajustes) y `/p` (sale) **antes** de iniciar bandeja y single-instance. Escribe `SCRNSAVE.EXE`, `ScreenSaveActive`, `ScreenSaveTimeOut` en `HKCU\Control Panel\Desktop` y llama `SystemParametersInfo(SPI_SETSCREENSAVEACTIVE / SPI_SETSCREENSAVETIMEOUT)`. | Esos tres valores. |
| `widget` | Reloj + cita del día | Carpeta de skins = `SkinPath` de `Rainmeter.ini`. Copia `<tema>\widget\` a `<SkinPath>\TemasWindows\<id>\`, ejecuta `Rainmeter.exe !RefreshApp` y luego `!ActivateConfig "TemasWindows\<id>" "<archivo>"`; desactiva (`!DeactivateConfig`) la config del tema anterior. Si Rainmeter no está instalado, se saltea con aviso. | Lista de configs `TemasWindows\*` activas (normalmente ninguna). Restaurar las desactiva. |

### Captura del estado original

- `original.json` es un mapa `módulo → captura`. Se completa **por módulo**: al aplicar un módulo que no tiene
  captura, se captura justo antes y se agrega al archivo. Así los módulos que llegan en etapas posteriores
  también tienen su original.
- Una captura existente no se sobrescribe mientras haya un tema activo.
- **"Restaurar original"** restaura todo. Las entradas que se restauraron bien pasan a `original.prev.json`
  (y sus archivos a `original.prev\`); las que fallaron se quedan en `original.json` para reintentar.
  El próximo "Aplicar" captura de nuevo lo que falte, de modo que los cambios manuales del usuario hechos sin tema activo se respetan.

### Flujo "Aplicar tema"

1. Validar `tema.json`. Si es inválido, abortar sin tocar nada.
2. Calcular `nuevos` = módulos que el tema define **y** el usuario no desmarcó.
3. Para cada módulo de `nuevos` sin captura en `original.json`: capturar y guardar.
4. Para cada módulo de `estado.json.tocados` que **no** está en `nuevos`: `restaurar()` desde `original.json`.
5. Para cada módulo de `nuevos`: `aplicar()`. Los errores se juntan; uno que falla no frena a los demás.
6. Guardar en `estado.json`: tema activo y `tocados` = todos los módulos de `nuevos` que se intentaron aplicar,
   hayan salido bien o no (uno que falló a mitad puede haber escrito algo y debe restaurarse después).
7. Intro (si está activada en Ajustes y el tema la define).
8. Devolver a la UI la lista ok / fallidos con mensaje.

### Qué pasa con lo que el tema no aplica

- Un módulo que **ningún tema tocó** nunca se modifica (se queda como el usuario lo tenga).
- Un módulo que el **tema anterior** aplicó y el nuevo no aplica (porque no lo define o porque el usuario lo desmarcó)
  vuelve al original. No quedan restos de un tema mezclados con otro.

### Persistencia entre reinicios

Colores, sonidos, íconos, terminal, bloqueo y protector viven en el registro/archivos y persisten.
La rotación de fondos sólo corre con la app abierta. Ajuste "Iniciar con Windows" (desactivado por defecto)
vía `tauri-plugin-autostart`; al arrancar, la app retoma la rotación del tema activo sin reaplicar el resto.
Una sola instancia (`tauri-plugin-single-instance`).

## Selector (UI)

Mockup aprobado: `D:\Wallpapers\.superpowers\brainstorm\1728-1791481412\galeria.html`.

### Bandeja del sistema

Ícono de la app (con la inicial del tema activo cuando sea posible). Menú:
- Lista de temas (el activo marcado) → aplica con todos sus módulos.
- Siguiente fondo · Pausar/Reanudar fondos · Abrir galería… (desde la etapa 2) · Restaurar original · Salir.

Cerrar la ventana de la galería no cierra la app; sólo "Salir" la termina.

### Galería (etapa 2)

- Ventana ~1100×700, siempre oscura, con el color de acento del tema activo.
- Grilla de tarjetas (portada + nombre, marca de activo, marca de error si el `tema.json` es inválido) + tarjeta "+ Nuevo tema".
- Panel de detalle: carrusel de fondos, muestras de color, casilla por módulo (deshabilitada si el tema no lo define),
  botones ▶ para escuchar sonidos e intro, botón "Aplicar tema".
- Tras aplicar, muestra el resultado por módulo (ok / error con mensaje).
- Se actualiza sola al cambiar el contenido de la carpeta de temas (`notify`).

### Nuevo tema

Pide un nombre, crea `<temas>\<id>\` con `tema.json` de ejemplo y subcarpetas vacías, y la abre en el Explorador.
`<id>` = nombre en minúsculas, sin acentos, no alfanuméricos → guiones. Si ya existe, se agrega `-2`, `-3`, …

### Ajustes (`D:\TemasApp\ajustes.json`)

Iniciar con Windows (off por defecto) · carpeta de temas (`D:\Temas`) · reproducir intro (on) · volumen de intro (60 %).

## Contenido inicial

- **Star Wars**: los 14 fondos ya descargados (migrados).
- **Game of Thrones**: fondos de Wallhaven (misma búsqueda: SFW, 16:9, ≥ 2560×1440), previa confirmación.
- Bloqueo y portada: una imagen de los fondos de cada tema, elegida con el usuario.
- Colores y terminal de cada tema: propuestos y ajustados con el usuario.
- Sonidos, intro, íconos, fuentes: se buscan fuentes y se confirman con el usuario, o los aporta él. Uso personal.

## Etapas

Cada etapa termina en algo usable.

1. **Núcleo + bandeja**: spike inicial de colores (confirmar que el acento cambia en vivo en barra/Inicio sin cerrar sesión);
   motor con `fondos`, `colores`, `bloqueo`, `terminal`; captura por módulo y restauración; menú de bandeja (sin "Abrir galería");
   migración desde `rotar.ps1`; temas Star Wars y Game of Thrones.
2. **Galería + instalador**: ventana, detalle, casillas por módulo, "Nuevo tema", Ajustes, autostart, instalador NSIS en `D:\Programas\`.
3. **Sonidos + intro.**
4. **Íconos + protector de pantalla** (presentación y crawl).
5. **Widget** (Rainmeter en `D:`, previa confirmación; fuentes por usuario).

## Pruebas

- **Unitarias (Rust)**: parseo y validación de `tema.json` (válido, faltantes, valores fuera de rango, rutas inexistentes,
  advertencia modo claro + acento en barra); `original.json`/`estado.json` (merge por módulo, rotación a `.prev`);
  cálculo de qué restaurar al cambiar de tema; generación de `<id>` con colisiones; mapeo de sonidos;
  generación de `AccentPalette`; completar esquema de terminal desde Campbell; edición JSONC que preserva comentarios.
- **Ida y vuelta por módulo** (integración, máquina real): `capturar` → `aplicar` → verificar que cambió →
  `restaurar` → verificar que el estado es idéntico a la captura. Para `fondos` y `bloqueo` se corren **dos ciclos**
  completos (aplicar → restaurar original → aplicar → restaurar) para cubrir la rotación a `.prev`. En `bloqueo` se comparan
  valores de registro y la existencia de la imagen copiada, no los bytes (`GetImageStream` re-codifica).
- **Terminal**: siempre sobre copias temporales de `settings.json`, nunca sobre el real.
- **Visual**: capturas del escritorio al cierre de cada etapa (el registro correcto no garantiza que se vea bien).

## Fuera de alcance

- Cambiar la fuente del sistema (riesgoso).
- Sonido de inicio de Windows (requiere admin).
- Navi con personalidad temática.
- Rotación automática de temas por día/hora.
- Editor de temas dentro de la app (posible a futuro).
- Cursores.
