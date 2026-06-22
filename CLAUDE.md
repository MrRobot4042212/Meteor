# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Contrato del proyecto Meteor

Contexto para Claude Code al ampliar este proyecto.

## Qué es

Launcher de escritorio para Windows que unifica juegos y apps de varias tiendas
(Steam, Epic, GOG, EA, Ubisoft, Xbox) más apps manuales en una sola biblioteca.
Tauri 2 (Rust) + Next.js 16 con **export estático** (`output: 'export'`). No hay
backend de red: la lógica nativa son comandos Rust invocados con
`@tauri-apps/api/core` → `invoke`.

## Reglas de arquitectura (respetar)

- **Toda lógica de sistema va en Rust**, en módulos por responsabilidad
  (`steam.rs`, `launcher.rs`, `storage.rs`, …). El front nunca toca el disco ni
  procesos directamente: siempre a través de un comando Tauri.
- **Cada fuente nueva = su propio módulo** (p. ej. `epic.rs`, `gog.rs`) que
  devuelve `Vec<Game>` y se mezcla en `get_library`. No meter parsing de varias
  tiendas en el mismo archivo.
- El modelo común es `Game` (`models.rs`). Si una fuente necesita campos nuevos,
  añadirlos como `Option<…>` con `#[serde(default)]` para no romper datos guardados.
- Los comandos devuelven `Result<T, String>`; el front muestra el error, nunca
  hace `panic`. Un fallo de una fuente no debe tumbar el resto de la biblioteca.
- Frontend tipado: todo comando tiene su wrapper en `src/lib/tauri.ts` y su tipo
  en `src/lib/types.ts` espejando el `struct` de Rust.
- Permisos: cualquier plugin nuevo de Tauri se declara en
  `src-tauri/capabilities/default.json`. (Las llamadas HTTP de carátulas se hacen
  desde Rust con `ureq`, no desde JS, así que no necesitan capability.)

## Fuentes y cómo detectan (estado actual)

Cada módulo expone `scan() -> Result<Vec<Game>, String>` y degrada a lista vacía
si la tienda no está instalada. `get_library` los mezcla en orden de prioridad y
**deduplica por nombre** (los escáneres nativos van antes que el genérico
`windows_apps`, así que un juego duplicado conserva la entrada más rica).

- `steam.rs` — `steamlocate`, manifiestos de cada librería. Arte: CDN de Steam.
- `epic.rs` — manifiestos `.item` en `%PROGRAMDATA%\Epic\...`; filtra a
  `AppCategories: games`. Lanza con `com.epicgames.launcher://`.
- `gog.rs` — registro `HKLM\...\GOG.com\Games`. Lanza el exe directo.
- `ubisoft.rs` — registro `Ubisoft\Launcher\Installs`. Lanza `uplay://launch/<id>/0`.
- `ea.rs` — registro `EA Games` / `Origin Games`; **heurística**: elige el exe más
  grande del install dir (salta instaladores/redist). Lanzamiento best-effort.
- `battlenet.rs` — **flavors de WoW**. Battle.net mete todas las versiones en una
  raíz (`World of Warcraft\`) con subcarpetas fijas (`_retail_`, `_classic_`,
  `_classic_era_`); el registro solo tiene una entrada "World of Warcraft", así que
  se localiza la raíz (`InstallLocation` de la clave de desinstalación, o rutas
  Program Files) y se emite **una `Game` por flavor instalado** (id
  `battlenet:wow_<flavor>`), lanzada con `battlenet://<código>` (`WoW`,
  `WoW_classic`, `WoW_classic_era`) y exe de fallback. PTRs ignorados. Va **antes**
  de `windows_apps` para que la dedup conserve estas entradas frente a la única del
  registro. Otros juegos de Blizzard (entrada única) siguen cayendo en
  `windows_apps` como `Battlenet`.
- `xbox.rs` — **principal**: enumera paquetes AppX cuyo install dir tiene
  `MicrosoftGame.config` (pilla también los de `WindowsApps`, no solo
  `XboxGames`) vía PowerShell, resuelve el **AUMID** y lanza por
  `shell:appsFolder\<AUMID>`. **Fallback** sin PowerShell: escaneo de
  `<unidad>:\XboxGames\*\Content` con `gamelaunchhelper.exe`.
- `windows_apps.rs` — **catch-all** del registro de desinstalación (HKLM 64/32 +
  HKCU). Filtra agresivamente (system components, runtimes, drivers, los propios
  clientes de las tiendas…) y **clasifica** por publisher/ruta en fuentes curadas
  (Battle.net, Riot, Rockstar, Amazon) o, si no, `GameSource::Windows`. Es ruidoso
  por naturaleza; los falsos positivos se **ocultan** desde la UI. Lanza por exe
  (de `DisplayIcon` o el mayor del install dir).

Lanzamiento (`launcher.rs`): Steam → `steam://rungameid`; `launch_uri` que empieza
por `shell:` → `explorer.exe` (apps de Xbox/Store); otros `launch_uri` → protocolo;
si no, se ejecuta `executable` directamente.

**Ocultar**: `hidden.json` (`storage.rs`) guarda ids ocultos; `get_library` los
filtra. Comandos `hide_game` / `hidden_count` / `restore_hidden`. En el front, el
icono del ojo en cada card oculta; Ajustes restaura.

**Favoritos y categorías**: overlays de usuario aplicados en `get_library` (igual
patrón que ocultos/carátulas), válidos para **cualquier** fuente. `favorites.json`
guarda ids favoritos; `categories.json` mapea id → `Vec<String>` de categorías.
Comandos `set_favorite(id, favorite)` y `set_categories(id, categories)` (reemplaza
la lista; vacía = borra la entrada, nombres trim + dedup case-insensitive). El
modelo `Game` lleva `favorite: bool` y `categories: Vec<String>` (`#[serde(default)]`,
no los rellenan los escáneres). En el front: estrella en cada card (persistente y
dorada si es favorito), botón de etiqueta abre `CategoryDialog` (chips para
categorías existentes + input para nuevas).

Las **categorías pueden existir vacías**: `category_names.json` guarda los nombres
creados explícitamente y `category_icons.json` mapea nombre → **icono** (clave de
un set incluido). Comandos `list_categories` (devuelve `Vec<Category>` =
`{name, icon}`), `add_category(name, icon?)`, `set_category_icon(name, icon)`,
`remove_category` (quita nombre, icono y lo despega de todos los juegos). El icono
es una **clave** de `src/lib/categoryIcons.tsx` (set monocromo incluido; el usuario
elige en una rejilla, no se inyecta SVG arbitrario). `NewCategoryDialog` tiene el
picker; sin icono → `TagIcon` por defecto.

**Sidebar en grupos separados**: «Biblioteca» (Todo/Favoritos), «Tiendas»
(fuentes automáticas = las "por defecto" de la app) y «Categorías» (las del
usuario, con su icono). El botón "Nueva categoría" (encima de Ajustes y un `+` en
la cabecera) abre `NewCategoryDialog`. `useLibrary` expone `categoryMeta` (con
iconos) / `refreshCategories`; `page.tsx` une `categoryMeta` + categorías en uso
(dedup por nombre, la entrada explícita conserva su icono). El filtro activo
(`favorites` o `cat:<n>`) cae a "Todo" si su último juego desaparece, salvo
categorías creadas vacías a propósito.

**Arrastrar y soltar**: cada `GameCard` es `draggable` y mete su `id` en el
`dataTransfer`. En la Sidebar, «Favoritos» (siempre visible, como zona permanente)
y cada categoría son zonas de soltado (`onDragOver`/`onDrop`, resaltado con
`ring-accent` al pasar por encima). Soltar un juego llama a `handleDropGame` en
`page.tsx`: en Favoritos → `set_favorite(id, true)`; en una categoría → añade el
nombre a `game.categories` y `set_categories`. Todo optimista (revierte con
`refresh()` si falla) y sin duplicar si ya estaba. **Gotcha Tauri**: el DnD HTML5
solo funciona con `"dragDropEnabled": false` en la ventana de `tauri.conf.json`;
con el valor por defecto (`true`) el webview intercepta el drop a nivel de SO
(cursor de "prohibido", no llegan los eventos). Cambiarlo exige **reiniciar**
`npm run app` (lo lee Rust al arrancar, no por hot-reload). Además los `<img>` de
las portadas llevan `draggable={false}` para que el drag lo capture la card.

## Carátulas y ajustes

- **Fuente única: IGDB.** `art.rs` → `resolve_cover(name)` prueba varias
  **variantes del nombre** (quita símbolos ™/®, sufijos de edición) contra IGDB
  (`igdb.rs`) y coge la portada `t_cover_big`. Todas las fuentes (incluido Steam)
  resuelven así; `steam.rs` ya no usa el arte de su CDN.
- **Caché en tres capas** (rápida → lenta): (1) imagen ya descargada en
  `covers/<hash>.jpg` → se sirve del disco, sin red; (2) URL cacheada en
  `cover_cache.json` → solo descarga la imagen, sin tocar la API; (3) consulta a
  IGDB (una vez por juego), guarda la URL y descarga. Las URLs se cachean para
  siempre; los **fallos solo con caducidad** (`NEGATIVE_TTL`). `clear_cache` borra
  el JSON **y** la carpeta `covers/`.
- Las imágenes locales se muestran vía el **protocolo `asset`** de Tauri
  (`convertFileSrc`, helper `src/lib/cover.ts`). Requiere el feature
  `protocol-asset` en `tauri` y `assetProtocol.scope` en `tauri.conf.json`
  (`$APPDATA/covers/**`). Los overrides manuales siguen siendo URLs remotas.
- `igdb.rs` autentica vía Twitch OAuth y cachea el token **en memoria** (`Mutex`
  estático) hasta poco antes de que caduque. Prefiere la coincidencia exacta de
  nombre. Las **credenciales van embebidas** en el binario (constantes, con
  override opcional por env `IGDB_CLIENT_ID` / `IGDB_CLIENT_SECRET` en build): el
  usuario no configura nada. (Un secret embebido es extraíble; rotar si se filtra.)
- No hay sistema de ajustes (`settings.rs` eliminado). El único comando de ajustes
  es `clear_cover_cache` (botón "Vaciar caché de carátulas").
- **Override manual**: `set_cover(id, url)` guarda una carátula por juego en
  `cover_overrides.json` (`storage.rs`). `get_library` la aplica al final, así que
  **gana sobre la auto-resolución** — cualquier carátula se puede arreglar a mano
  (botón de la card → `CoverDialog`). Además de pegar una **URL**, el usuario puede
  **arrastrar una imagen local** (o elegir archivo) en `CoverDialog`: se leen los
  bytes en JS y `set_cover_image(id, data, ext)` los guarda en `user_covers/`
  (`save_cover_image`) y fija el override a esa ruta. Esa carpeta está en el
  `assetProtocol.scope` y **sobrevive a `clear_cover_cache`** (que solo borra
  `covers/`).
- Las carátulas se resuelven **en segundo plano** desde el front (`useLibrary`, 3
  en paralelo para no pasar el rate-limit de IGDB); `get_library` no bloquea por red.
- **Splash de primer arranque** (`Splash.tsx`): `useLibrary` expone `booting`
  (true hasta cargar la biblioteca + terminar la **primera** pasada de carátulas) y
  `coverProgress` (`{done,total}`) para la barra. Solo el primer `refresh` lo
  dispara; los refrescos posteriores son silenciosos. `page.tsx` lo muestra mientras
  `booting && !splashDone`; un botón "Entrar ahora" (a los 4 s) permite saltarlo si
  la red va lenta. La animación indeterminada usa `.animate-meteor-slide`
  (`globals.css`).

## Tema / UI

Paleta de **rojo primary + azul info** (sin verde, a propósito). Las variables de
color viven en `src/app/globals.css` como **tripletes RGB** (`--background`,
`--foreground`, `--surface`, `--elevated`, `--sidebar`, `--border`,
`--muted-foreground`, `--primary` (rojo), `--primary-foreground`, `--primary-soft`,
`--ring`, `--popover`, `--info` (azul), `--destructive` (ámbar)…) con bloques
`:root` (claro) y `.dark` (oscuro). **El tema oscuro está activo** vía `class="dark"`
en el `<html>` (`layout.tsx`); cambiar a claro = quitar esa clase. Tipografía:
**Oxanium** (UI, body + display, var `--font-oxanium`) y **Source Code Pro** (mono,
`--font-mono`), cargadas con `next/font/google`.

`tailwind.config.ts` (Tailwind v3, `darkMode: 'class'`) mapea los tokens a esas
variables con `rgb(var(--x) / <alpha-value>)`, así que los modificadores de
opacidad (`bg-void/70`, `text-accent/40`…) siguen funcionando. Se conservan los
**alias heredados** para no reescribir componentes: `void→background`,
`surface→surface`, `elevated→elevated`, `line→border`, `ink→foreground`,
`muted→muted-foreground`, `accent→primary` (las CTA son rojas),
`accent-soft→primary-soft`; más `sidebar` (color propio, usado en la `<aside>`) e
`info` (azul, disponible). El texto sobre botones primary es `text-white`. Sombras
con elevación real (`shadow-card`; `shadow-glow` con halo del `--ring` rojo). Los
dots de fuente son grises uniformes (`bg-foreground/50`); la fuente
real se distingue por el icono etiquetado del sidebar. **Sin border-radius**: el
scale `borderRadius` está sobreescrito a `0` en `tailwind.config.ts` (look
modernista de esquinas rectas), así que todos los `rounded-*` (incluido
`rounded-full`) son rectos sin tocar componentes.

## Página de detalle (por juego)

Export estático → **no hay rutas dinámicas de servidor**; el detalle es **routing
en cliente**: `page.tsx` guarda `selectedId`, deriva `selected` de `games` por id
(siempre fresco) y monta `DetailView` dentro del `<main>` (Esc o botón "Biblioteca"
para volver). La card abre con `onOpen` (click en la portada; los botones internos
hacen `stopPropagation`).

`DetailView.tsx` carga sus datos lazy al abrir:
- **Metadatos IGDB** (`game_details(name)` → `igdb::fetch_details`, cacheado en
  `details_cache.json` por `art::details`, mismo patrón que carátulas con
  `NEGATIVE_TTL`): sinopsis, géneros, modos, año, dev/editor, rating. Géneros y
  modos se traducen a **castellano** con `src/lib/i18n.ts` (mapa fijo, fallback al
  original). La **sinopsis se traduce** con `translate.rs` (endpoint gratuito de
  **Google Translate** `gtx`, `tl=es`, con caché en `translate_cache.json` por
  hash del texto); `art::details` la traduce antes de cachear los detalles, con
  fallback al inglés si la petición falla.
- **Capturas propias del usuario** (no arte promocional): `user_screenshots(game)`
  → `screenshots.rs` reúne las de **Steam** (`userdata/<acc>/760/remote/<appid>/
  screenshots`) y **Windows Game Bar** (`Vídeos/Captures`, por prefijo de nombre),
  ordenadas por fecha. Devuelve rutas locales y las **autoriza en el asset scope en
  runtime** (`app.asset_protocol_scope().allow_file`), así se ven con
  `convertFileSrc` sin ampliar el scope estático. (Las screenshots de IGDB ya no se
  muestran.)
- **Info local**: `dir_size(path)` (tamaño en disco, walk iterativo) y
  `open_path(path)` (abre carpeta) en `files.rs`.
- **Tiempo de juego**: `playtime.rs` + crate `sysinfo`. `launch_game` ahora recibe
  `AppHandle` y, tras lanzar, llama `playtime::track`, que en un **hilo** espera a
  que aparezca un proceso cuyo exe esté bajo `install_dir` (o sea `executable`),
  cronometra hasta que sale, persiste en `playtime.json` (`get_playtime(id)` →
  `{seconds,last_played}`) y emite el evento **`playtime-updated`** (el front lo
  escucha con `listen` y refresca). Heurístico: para juegos de tienda el proceso
  real es otro exe, se casa por prefijo de `install_dir`; sesiones <30 s se ignoran.

Comandos nuevos: `game_details`, `get_playtime`, `dir_size`, `open_path`.
`clear_cover_cache` también borra `details_cache.json`.

## Limitaciones a tener presentes

- Sin SSR ni API routes (export estático). Nada de `fetch` a rutas internas.
- Las imágenes externas funcionan porque `csp` está en `null` en `tauri.conf.json`.
  Si se endurece la CSP, hay que permitir explícitamente el CDN de Steam.

## Roadmap (siguientes fases)

Hecho: Steam, Epic, GOG, EA, Ubisoft, Xbox + apps manuales; carátulas vía
SteamGridDB con fallback a Steam CDN.

1. **Robustez de fuentes**: EA/Ubisoft/Xbox usan heurísticas frágiles (formatos
   no documentados); afinar detección y lanzamiento. Pendiente: apps instaladas
   de Windows (claves de desinstalación del registro + accesos del menú inicio).
2. **Más metadatos**: géneros/IGDB; mejor *matching* de nombre → arte (ahora se
   coge el primer resultado, puede fallar en títulos ambiguos).
3. **Calidad de vida**: ~~favoritos~~ ✓ y "jugados recientemente",
   ~~categorías/etiquetas~~ ✓, ordenación (pendiente).
4. **Tiempo de juego**: vigilar el proceso lanzado y acumular horas por juego.
5. **Opcional en la nube**: backend Nest.js para sincronizar biblioteca y ajustes
   entre varios PCs (aquí sí entraría Nest, no antes).

## Comandos

```bash
npm install
npm run app         # desarrollo (tauri dev): compila Rust + Next, abre la ventana
npm run app:build   # instalador (.msi/.exe en src-tauri/target/release/bundle)
npx tsc --noEmit                       # typecheck del front
cd src-tauri && cargo check            # typecheck del back
```

- **No hay tests** en el proyecto (ni JS ni `cargo test`); no busques un runner.
- `npm run lint` (`next lint`) **está roto**: Next 16 quitó ese subcomando y
  trata `lint` como directorio. Usa `npx tsc --noEmit` para validar el front.
- `npm run dev` / `npm run build` levantan solo Next en el navegador: ahí los
  `invoke` a comandos Tauri fallan. Para probar lógica nativa usa `npm run app`.
- Los comandos Tauri viven en `lib.rs` (no hay archivo `commands.rs` aparte) y se
  registran en el `invoke_handler` del builder.
