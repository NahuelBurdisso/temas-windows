# Temas Windows

Selector de temas para Windows 11. Un "tema" es una carpeta con un `tema.json` y sus archivos
(fondos, colores, pantalla de bloqueo, esquema de la terminal...). La app lo aplica de una vez
desde la bandeja del sistema y todo se puede deshacer con **Restaurar original**.

Funciona sin privilegios de administrador: todo se hace en `HKCU` o con APIs de usuario.

## Estado

Etapa 1 terminada: el motor y la bandeja del sistema.

- Aplicar un tema: colores del sistema (acento, modo claro/oscuro, transparencia), fondos
  rotativos, pantalla de bloqueo y esquema de Windows Terminal.
- Captura del estado original antes de tocar nada, y restauración exacta de lo que se cambió.
- Rotador de fondos con intervalo y orden configurables.
- Validación de `tema.json`: un tema inválido se informa y no se aplica, sin romper la app.
- Ícono en la bandeja con la lista de temas, cambio de fondo y restaurar.

Previsto para las etapas siguientes: galería (React), instalador NSIS, sonidos, íconos del
escritorio, protector de pantalla y widget. El diseño completo está en
[`docs/superpowers/specs`](docs/superpowers/specs/2026-10-08-selector-temas-design.md).

## Estructura

| Carpeta | Qué |
|---|---|
| `motor/` | Crate Rust con la lógica: modelo y validación del tema, estado, registro, rotador y un módulo por cada cosa que se aplica (`colores`, `fondos`, `bloqueo`, `terminal`) |
| `src-tauri/` | App Tauri 2: bandeja del sistema y ciclo de vida |
| `ui/` | Interfaz web |
| `docs/` | Diseño y plan de implementación |

## Un tema

```
<carpeta de temas>\<id>\
├── tema.json        configuración (sólo "nombre" es obligatorio)
├── fondos\          wallpapers que rotan
└── bloqueo.jpg      pantalla de bloqueo
```

```json
{
  "nombre": "Game of Thrones",
  "colores": { "acento": "#C9A227", "modo": "oscuro", "transparencia": true, "acentoEnBarra": true },
  "fondos": { "intervaloSeg": 20, "orden": "aleatorio", "estilo": "rellenar" },
  "bloqueo": "bloqueo.jpg",
  "terminal": { "fondo": "#141414", "texto": "#E8D9B0", "cursor": "#C9A227" }
}
```

Una sección ausente significa que el tema no toca esa parte del sistema. Este repositorio no
incluye temas ni imágenes: cada uno arma los suyos.

## Desarrollo

Requiere Windows 11, Rust estable (MSVC), Node y WebView2.

```bash
npm install
npm run tauri dev        # app con bandeja
cargo test -p motor      # pruebas del motor (ida y vuelta aplicar/restaurar)
cargo run -p motor --example validar_tema -- <carpeta-del-tema>
```
