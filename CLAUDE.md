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
  HKCU). Filtra en **dos niveles**: `is_junk` descarta lo que nunca es un item de
  biblioteca (runtimes, drivers, components del SO, vendors de hardware, los
  clientes de las tiendas…); lo que sobrevive lo **clasifica** `classify` por
  publisher/ruta en fuentes curadas de juego (Battle.net, Riot, Rockstar, Amazon),
  o como **`GameSource::App`** si `apps_db::is_app` lo reconoce como aplicación
  (no juego), o si no `GameSource::Windows` (probable juego). Curado y heurístico:
  los falsos positivos se **ocultan** desde la UI (las entradas `App`/`Windows`
  llevan el icono del ojo, no el de borrar). Lanza por exe (de `DisplayIcon` o el
  mayor del install dir). El `DisplayName` se **limpia** con `clean_name` (quita
  versión/arquitectura: «OBS Studio 30.0.2» → «OBS Studio», «7-Zip (x64)» →
  «7-Zip», «Git version 2.43.0» → «Git»; conserva números sin punto como
  ediciones/años: «Office 365», «Visual Studio 2022»). El `id` deriva del nombre
  limpio, así que cambiar la limpieza puede resetear overlays (ocultos/favoritos).
- `apps_db.rs` — **librería grande de firmas de apps** que usa `windows_apps` para
  separar aplicaciones de juegos. `is_app(name, publisher, install_location)` casa
  por substring case-insensitive contra tres tablas: `APP_DIR_HINTS` (rutas
  per-user donde no viven juegos, p. ej. `\AppData\Local\Programs\`),
  `APP_PUBLISHERS` (editores de software: Microsoft, Google, Adobe, JetBrains,
  VideoLAN…) y `APP_NAMES` (cientos de nombres concretos por categoría: navegadores,
  comunicación, multimedia, creatividad, audio, ofimática, dev, utilidades, cloud,
  seguridad/VPN, remoto, descargas, periféricos). **Aditiva**: ampliar las tablas
  para clasificar más apps; tokens **específicos** (evitar palabras genéricas que
  choquen con títulos de juegos). Sin match → queda como `Windows`.

**Icono real del .exe (logo de apps)**: las entradas `App` (y cualquiera sin
carátula) muestran el icono **embebido en su ejecutable** — así cualquier app
instalada (Chrome, VLC, 7-Zip, Thunderbird…) sale con su icono correcto **sin
lista curada ni red**. `appicons.rs` lee el grupo de iconos de los **recursos PE**
con **`pelite`** (Rust puro, sin libs nativas; se descartó `systemicons` por
conflicto `gtk-sys` con `rfd`), escribe un `.ico` cacheado en `app_icons/<hash>.ico`
y lo autoriza en el asset scope en runtime (WebView2 pinta `.ico` en `<img>`).
Comando `app_icon(path)` → ruta local; `useLibrary` lo resuelve **lazy** (pasada
aparte, solo entradas `App` sin cover, concurrencia 6, sin red). El front guarda la
ruta en `Game.icon` (**campo solo de cliente**, no del modelo Rust) y lo pinta
centrado en la card/detalle. Prioridad de imagen: **override manual > icono del exe
> letra**. (Hubo una capa previa de logos SVG curados en `public/logos/` +
`appLogos.ts`; se eliminó por redundante con el icono del exe e inconsistente.)
**Importante**: `useLibrary` **no resuelve carátulas de IGDB para entradas `app`**
(IGDB es de juegos → daría arte equivocado, p. ej. el navegador Brave → la película
«Brave»); las apps se quedan con su icono del exe. El resto de fuentes (incluido el
catch-all `windows`, «probable juego») sí piden IGDB.

Lanzamiento (`launcher.rs`): Steam → `steam://rungameid`; **Battle.net → exe
directo** (su `battlenet://` solo enfoca el launcher, no arranca el juego, así que
se lanza el `executable` del flavor y de paso el proceso es vigilable para playtime;
fallback a la URI si no hay exe); resto: `launch_uri` que empieza por `shell:` →
`explorer.exe` (apps de Xbox/Store); otros `launch_uri` → protocolo; si no, se
ejecuta `executable` directamente.

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
`remove_category` (quita nombre, icono y lo despega de todos los juegos),
`rename_category(old, new)` (renombra en nombres, icono y en la lista de cada
juego; **fusiona** si `new` ya existe) y `set_category_order(names)` (persiste el
orden en `category_names.json`; **promueve a explícitas** las categorías solo-en-uso
que se incluyan). El icono es una **clave** de `src/lib/categoryIcons.tsx` (set
monocromo incluido; el usuario elige en una rejilla, no se inyecta SVG arbitrario).
`NewCategoryDialog` crea; `EditCategoryDialog` renombra/cambia icono; sin icono →
`TagIcon` por defecto.

**Gestión de categorías en el sidebar**: cada fila de categoría es **draggable
para reordenar** (usa el mime `application/x-meteor-cat`; al soltar, si el drag es
de categoría reordena, si es de un juego lo asigna — así convive con el DnD de
cards). **Click derecho** en una categoría abre un menú contextual (Editar →
`EditCategoryDialog`; Eliminar con `confirm`): el `Sidebar` emite
`onCategoryContextMenu(cat,x,y)` y `page.tsx` reusa el mismo `ContextMenu` que las
cards (estado `menu` genérico `{x,y,items}`, lo comparten juego y categoría). El
orden mostrado = orden guardado de `categoryMeta`, y las categorías solo-en-uso se
añaden al final alfabéticamente. Reordenar/eliminar refrescan vía
`setCategoryOrder`/`remove_category` + `refreshCategories()`/`refresh()`.

**Confirmación de acciones destructivas** (`ConfirmDialog.tsx`): ocultar/quitar un
juego o app y eliminar una categoría piden confirmación con un modal propio (Enter
confirma, Esc cancela; botón en color `destructive`). `page.tsx` tiene un estado
`confirm` `{title,message,confirmLabel,onConfirm}`; `handleHide`/`handleRemove`/
`bulkHide`/`handleDeleteCategory` muestran el diálogo y la acción real vive en
`doHide`/`doRemove`/`doBulkHide`/`doDeleteCategory` (que también cierran el detalle
si el item afectado estaba abierto). Sustituye al `window.confirm` nativo.

**Footer de atajos** (`Footer.tsx`): barra inferior fija (el layout de `page.tsx`
es **columna**: fila `sidebar+main` con `flex-1` y el footer debajo) que lista los
atajos con chips `<kbd>`: Ctrl+Shift+Espacio (Spotlight), clic derecho (acciones),
Ctrl+clic (selección), arrastrar (categorizar/reordenar), ↑/↓/↵ (buscador), Esc
(cerrar/volver). Las barras flotantes (acciones en lote, toast) van a `bottom-14`
para no taparlo.

**Sidebar en grupos separados**: «Biblioteca» (Todo/Favoritos), «Proveedores»
(fuentes automáticas de juego = las "por defecto" de la app; excluye `app`),
«Aplicaciones» (filtro `Apps` = `GameSource::App`, solo si hay alguna) y
«Categorías» (las del usuario, con su icono). El botón "Nueva categoría" (encima de Ajustes y un `+` en
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

**Power-user UX** (en `page.tsx`):
- **Menú contextual** (click derecho en `GameCard` → `onContextMenu(g,x,y)`):
  `ContextMenu.tsx` flota en el cursor (se clampa al viewport, cierra con Esc/click
  fuera/scroll). Acciones: jugar, favorito, categorías, carátula, abrir carpeta
  (`open_path` de `install_dir` o carpeta del exe) y ocultar/quitar según fuente.
- **Ordenar**: `<select>` en la cabecera — Nombre (A-Z), Más jugados, Jugados
  recientemente. Usa `playtimes` (mapa id→`PlayStat` que expone `useLibrary` vía el
  comando `all_playtime`, recargado al evento `playtime-updated`). Se desactiva
  mientras hay búsqueda (los resultados se ordenan por relevancia).
- **Búsqueda fuzzy**: `src/lib/fuzzy.ts` (`fuzzyScore`, sin deps): substring directo
  como señal fuerte, si no subsecuencia con bonus por consecutivos y por inicio de
  palabra; con query, `visible` filtra por match y ordena por score.
- **Selección múltiple**: botón «Seleccionar» (o **Ctrl/Cmd+click** en una card)
  entra en `selectMode`; en ese modo el click alterna selección (`selectedIds:
  Set`), aparece checkbox y se ocultan estrella/herramientas/overlay y el drag. Una
  **barra flotante** muestra el nº seleccionado + acciones en lote: Todos (visibles),
  Favorito, Categorías (`BulkCategoryDialog`, solo añade — no borra), Ocultar,
  Cancelar. Todas optimistas (`Promise.allSettled`, revierten con `refresh()` si
  fallan). Esc sale de selección (o cierra el detalle si está abierto).

## Carátulas y ajustes

- **Fuente única: IGDB.** `art.rs` → `resolve_cover(name)` prueba varias
  **variantes del nombre** (quita símbolos ™/®, sufijos de edición) contra IGDB
  (`igdb.rs`) y coge la portada **`t_cover_big_2x`** (528×748, alta resolución).
  Todas las fuentes (incluido Steam) resuelven así; `steam.rs` ya no usa el arte de
  su CDN. (Las carátulas ya descargadas en `covers/` no se re-piden; para subirlas a
  2x hay que «Vaciar caché de carátulas» en Ajustes.)
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
- **Tiempo de juego (watcher global)**: `playtime.rs` + crate `sysinfo`. Se **mide
  todo desde Meteor** (no se lee el tiempo de Steam/VDF: cifras homogéneas). Un
  **único hilo en background** (`playtime::start`, arrancado en `setup`) hace polling
  de **todos** los procesos cada `POLL_SECS`=5 y los casa contra el **índice de la
  biblioteca** (`library_cache.json`, recargado cada `INDEX_REFRESH_SECS`=60): un
  juego se cronometra **se lance como se lance** (Meteor, Steam, acceso directo…),
  ya no solo lo abierto desde la app. Matching: el exe del proceso == `executable`,
  o está bajo `install_dir` excluyendo helpers (`EXCLUDE`: crash handlers, redist,
  anti-cheat…). Mantiene en memoria `id → (start, last_seen)`; al desaparecer el
  proceso cierra la sesión, persiste en `playtime.json` (`get_playtime(id)` →
  `{seconds, last_played, history}`, **`history` = `Vec<{start,end}>`**) y emite
  **`playtime-updated`**. Sesiones <30 s (`MIN_SESSION_SECS`) se ignoran.
  `launch_game` ya **no** rastrea (solo lanza). **Recuperación tras cierre**: cada
  poll vuelca las sesiones en curso a `active_sessions.json`; `playtime::reconcile`
  (en `setup`, antes de `start`) cierra al arrancar las colgadas hasta su
  `last_seen`. El detalle muestra tiempo jugado, **últimos 14 días** (de `history`),
  nº de sesiones, última vez y tamaño.

Comandos nuevos: `game_details`, `get_playtime`, `all_playtime`,
`dir_size`, `open_path`, `app_icon`, `cached_library`, `get_discord_client_id`,
`set_discord_client_id`, `get_autostart`, `set_autostart`.

**Spotlight global** (`Spotlight.tsx` + plugin `tauri-plugin-global-shortcut`): atajo
**Ctrl+Shift+Space** registrado en `setup`; su handler trae la ventana `main` al
frente (show/focus/unminimize) y emite **`open-spotlight`**. El front lo escucha y
abre una paleta: input con autofocus, búsqueda **fuzzy** (`fuzzyScore`) sobre la
biblioteca, ↑/↓ para moverse, Enter lanza (`launch_game`), Esc cierra; sin query
muestra favoritos primero. Funciona aunque Meteor esté minimizado/sin foco.

**Home / Dashboard** (`Home.tsx`, filtro `home` del Sidebar, **vista por defecto** al
abrir): no toca Rust, todo se deriva en cliente de `games` + `playtimes` (el mapa
id→`PlayStat` con `history` que ya expone `useLibrary`). `page.tsx` define
`showingHome = filter === 'home' && !query.trim()`; si se cumple, el `<main>` pinta
`<Home>` en vez de la grid y oculta el orden + «Seleccionar» (no hay grid). **Buscar
desde Inicio** funciona: con query, `home` cae en `inFilter = todos` y se muestran
resultados fuzzy como una grid normal. Secciones del dashboard: tarjetas de stats
(tiempo total, esta semana, sesiones 7d, juegos jugados), **gráfica de barras de los
últimos 7 días** (buckets con límites en **medianoches locales reales** —no pasos de
24h fijos, así el cambio de hora/DST no descuadra— y cada sesión de `history` se
**reparte entre los días que abarca** clipando `[start,end]` contra cada ventana, así
una sesión que cruza medianoche cuenta en ambos días; hoy resaltado en `accent`), **«Continuar jugando»** (fila scroll-x por
`last_played` desc) y dos rankings separados por `source` (`RankList`): **«Juegos más
jugados»** (`source !== 'app'`) y **«Apps más usadas»** (`source === 'app'`), cada uno
top 6 por `seconds` y mostrado solo si tiene entradas. Sin datos de playtime
muestra un fallback con favoritos/primeros juegos. Cards propias ligeras (`Thumb`
replica la cascada carátula→icono exe→letra de `GameCard`, sin tilt/drag).

**Auto-update** (`UpdatePrompt.tsx` + plugins `tauri-plugin-updater`/`-process`):
al arrancar, `check()` consulta el endpoint `releases/latest/download/latest.json`
del repo de GitHub; si hay versión nueva (firmada), muestra un banner abajo-derecha
con `downloadAndInstall` (barra de progreso) → `relaunch()`. Silencioso si está al
día, sin red o en **dev** (la comprobación falla y se ignora en try/catch). Config en
`tauri.conf.json`: `bundle.createUpdaterArtifacts: true` y `plugins.updater`
(`endpoints` + `pubkey`). Permisos `updater:default` + `process:default` en la
capability `default`. **Firma**: la release se firma con una clave Ed25519; la
**privada** va en GitHub Secrets (`TAURI_SIGNING_PRIVATE_KEY` +
`_PASSWORD`), la **pública** en `tauri.conf.json` (`pubkey`). El workflow
`.github/workflows/release.yml` (en push de tag `v*`) compila en `windows-latest`
con `tauri-action`, firma, crea la Release y sube `latest.json`. **Importante**: con
`createUpdaterArtifacts` activo, un `tauri build` local **necesita** las env vars de
firma o falla; los builds de release se hacen vía el workflow.

**Bandeja del sistema + autostart** (en `lib.rs`): Meteor es un launcher que vive
en background (watcher de playtime, Discord, Spotlight global). Por eso **cerrar la
ventana `main` no mata el proceso**: el handler `on_window_event` intercepta
`CloseRequested` de `main`, hace `prevent_close()` + `window.hide()`. Un **tray icon**
(`TrayIconBuilder` en `setup`, feature `tray-icon` + `image-png` de tauri; usa
`default_window_icon`) con menú **Mostrar Meteor / Salir**: click izquierdo o
«Mostrar» reabren la ventana (`show_main`, helper compartido con el handler de
Spotlight); «Salir» hace `app.exit(0)` (única salida real). **Iniciar con Windows**
vía `tauri-plugin-autostart` (escribe la Run key del registro): comandos propios
`get_autostart`/`set_autostart` (no se llama al plugin desde JS, así que no necesita
capability), toggle en `SettingsDialog` (optimista, revierte si falla).

**Arranque instantáneo (caché de biblioteca)**: `get_library` vuelca el resultado a
`library_cache.json` al final. Al abrir, `useLibrary` pinta primero `cached_library`
(sin splash) y luego refresca en background; solo hay splash en el **primer arranque
real** (sin caché). El mismo fichero es el índice que usa el watcher de playtime.

**IDs estables**: las entradas del registro (`windows_apps`) usan `windows:<clave de
desinstalación>` (GUID/product code), no el nombre, así que limpiar el `DisplayName`
o que la app se actualice **no pierde** overlays (ocultos/favoritos/categorías/
playtime). (Cambio único de esquema: overlays previos por nombre se resetean una vez.)

**Discord Rich Presence** (`discord.rs`, crate `discord-rich-presence` — IPC local,
sin SDK): el **watcher de playtime** lo conduce. En cada poll calcula el juego
corriendo más recientemente y, si cambia, llama `discord::set_playing(name, start)`
(`details`=nombre, `state`=«Jugando», timer desde el inicio de la sesión) o
`clear()` si no hay ninguno. Best-effort: si Discord no está abierto o no hay id,
no hace nada (y reintenta porque `presence` solo se fija cuando Discord acepta).
**Client id embebido**: `DEFAULT_CLIENT_ID` (constante, override por env
`DISCORD_CLIENT_ID` en build) trae el Application ID de la app de Discord del
proyecto, así **funciona para todos sin configurar nada**. El Application ID **no
es secreto** (solo lo es el client *secret*, que RPC no usa), igual que las creds
de IGDB. El id efectivo = override del usuario si lo hay, si no el embebido; el
**override** es opcional, se guarda en `discord.json` (`get/set_discord_client_id`),
se aplica en vivo (`set_client_id` cierra la conexión para reconectar) y se carga en
`setup`. Discord muestra "Playing <nombre de esa app>" (de ahí nombrarla «Meteor»).
`LARGE_IMAGE` (vacío) permite una imagen si se sube un asset al portal de Discord.
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
3. **Calidad de vida**: ~~favoritos~~ ✓, ~~"jugados recientemente"~~ ✓ (Home),
   ~~categorías/etiquetas~~ ✓, ~~ordenación~~ ✓.
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
