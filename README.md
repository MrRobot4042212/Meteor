# Meteor

Biblioteca unificada de juegos y aplicaciones para Windows. Detecta automáticamente
tus juegos de **Steam, Epic, GOG, EA, Ubisoft y Xbox (Game Pass)**, te deja añadir
cualquier otra app o juego a mano, y muestra las carátulas de cada uno — todo en una
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

Cada tienda tiene su propio escáner; si una no está instalada, simplemente no
aporta juegos (no rompe el resto). La lista se mezcla y se deduplica por nombre.

- **Steam**: `steamlocate` recorre las librerías y lee los manifiestos. Arte del
  CDN público de Steam.
- **Epic**: manifiestos `.item` en `%PROGRAMDATA%\Epic\...`. Lanza vía
  `com.epicgames.launcher://`.
- **GOG**: registro (`GOG.com\Games`). Lanza el ejecutable directamente.
- **Ubisoft**: registro (`Ubisoft\Launcher\Installs`). Lanza vía `uplay://`.
- **EA / Origin**: registro (`EA Games` / `Origin Games`); detección best-effort.
- **Xbox / Game Pass**: escanea `XboxGames\...\MicrosoftGame.config` y lanza con
  `gamelaunchhelper.exe`.
- **Apps manuales**: se eligen con el selector de archivos nativo y se guardan en
  `manual_apps.json` dentro de la carpeta de datos de la app.

### Carátulas

Todas las carátulas (de cualquier tienda, Steam incluido) se obtienen de **IGDB**.
Meteor busca por nombre (probando variantes para absorber símbolos y ediciones),
prefiere la coincidencia exacta y usa la portada vertical de IGDB. IGDB va por la
API de Twitch, así que necesitas un **Client ID + Client Secret** (gratis, en
`dev.twitch.tv/console/apps`) y pegarlos en *Ajustes*.

Las imágenes se **descargan y guardan en disco** (`covers/`) la primera vez, así
que a partir de ahí cargan al instante, sin red y sin volver a llamar a la API
(la URL también se cachea en `cover_cache.json`). Los fallos se reintentan pasado
un tiempo, así que las carátulas que falten no se quedan vacías para siempre.

¿Una carátula sigue sin salir o no es la correcta? Pasa el ratón por encima de la
card y pulsa el icono de imagen para **ponerle una a mano** (override que gana
sobre la automática). Las credenciales de Twitch/IGDB se guardan en `settings.json`.

## Estructura

```
src/                      # Frontend Next.js
  app/                    # layout, page, estilos globales
  components/             # Sidebar, GameCard, AddAppDialog, SettingsDialog, iconos
  hooks/useLibrary.ts     # carga/recarga + resolución de carátulas en 2º plano
  lib/                    # tipos, wrappers de comandos Tauri, metadatos de fuentes
src-tauri/                # Núcleo Rust
  src/
    lib.rs                # comandos Tauri y builder
    models.rs             # Game / GameSource
    steam.rs epic.rs gog.rs ea.rs ubisoft.rs xbox.rs   # un escáner por tienda
    art.rs                # resolución de carátulas vía IGDB (+ caché)
    igdb.rs               # cliente IGDB (auth Twitch OAuth)
    settings.rs           # ajustes (API key de SteamGridDB)
    storage.rs            # persistencia de apps manuales
    launcher.rs           # lanzamiento (protocolo de tienda o ejecutable)
  capabilities/           # permisos (core + dialog)
  tauri.conf.json
```

Consulta `CLAUDE.md` para el contrato del proyecto y las siguientes fases.
