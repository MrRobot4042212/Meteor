# Nexo

Biblioteca unificada de juegos y aplicaciones para Windows. Detecta automáticamente
tus juegos de Steam y te deja añadir cualquier otra app o juego a mano, todo en una
sola ventana desde la que lanzar lo que quieras.

Stack: **Tauri 2 (Rust)** como núcleo nativo + **Next.js 16 (export estático)** como
frontend. No hay servidor: toda la lógica vive en Rust y se invoca desde React.

---

## Requisitos (Windows)

1. **Node.js 18+** — https://nodejs.org
2. **Rust** (incluye `cargo`) — https://rustup.rs
3. **Microsoft C++ Build Tools** (workload "Desktop development with C++") —
   https://visualstudio.microsoft.com/visual-cpp-build-tools/
4. **WebView2** — ya viene preinstalado en Windows 11.

## Arrancar en desarrollo

```bash
npm install
npm run app        # = tauri dev (compila Rust + levanta Next y abre la ventana)
```

La primera compilación de Rust tarda un poco; las siguientes son incrementales.

## Compilar el instalador

```bash
npm run app:build  # genera .msi / .exe en src-tauri/target/release/bundle
```

---

## Cómo funciona

- **Steam**: `steamlocate` localiza la carpeta de Steam, recorre todas las librerías
  y lee los manifiestos de cada juego instalado. Las carátulas se cargan del CDN
  público de Steam (capsule vertical, con *fallback* al `header.jpg`).
- **Apps manuales**: se eligen con el selector de archivos nativo y se guardan en
  `manual_apps.json` dentro de la carpeta de datos de la app.
- **Lanzar**: los juegos de Steam se abren con `steam://rungameid/<id>` (para que
  Steam gestione overlay/actualizaciones); las apps manuales se ejecutan directamente.

## Estructura

```
src/                      # Frontend Next.js
  app/                    # layout, page, estilos globales
  components/             # Sidebar, GameCard, AddAppDialog, iconos
  hooks/useLibrary.ts     # carga/recarga de la biblioteca
  lib/                    # tipos + wrappers tipados de los comandos Tauri
src-tauri/                # Núcleo Rust
  src/
    lib.rs                # comandos Tauri y builder
    models.rs             # Game / GameSource
    steam.rs              # escaneo de Steam
    manual ↦ storage.rs   # persistencia de apps manuales
    launcher.rs           # lanzamiento de procesos / protocolo Steam
  capabilities/           # permisos (core + dialog)
  tauri.conf.json
```

Consulta `CLAUDE.md` para el contrato del proyecto y las siguientes fases.
