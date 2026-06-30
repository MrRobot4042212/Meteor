# Changelog

Todas las novedades destacables de **Meteor** (launcher de escritorio que unifica
juegos y apps de varias tiendas en una sola biblioteca).

El formato sigue, a grandes rasgos, [Keep a Changelog](https://keepachangelog.com/es-ES/)
y el proyecto usa versionado semántico aproximado. Las fechas son orientativas.

---

## [No publicado] — Trabajo en curso

### Añadido
- **Overlay adaptativo: detecta si cuesta FPS y reacciona (MPO)** (`metrics.rs`,
  `overlay_dcomp.rs`, `system.rs`, `OverlayMpoPanel.tsx`): un overlay sin inyección solo
  es **gratis** si Windows le concede un **plano hardware (MPO)**; en multimonitor o con
  refrescos mezclados, DWM lo **compone** y el juego pierde *independent-flip* → bajón de
  FPS/latencia. Ahora el overlay **se entera en vivo**: el sampler lee el modo de
  composición real del swapchain (`overlay::composition_mode`) tras una breve ventana de
  medición por sesión y clasifica el HUD como **libre** (plano hardware) o **costando**
  (DWM componiendo), con histéresis. Estado expuesto a la UI (`overlay_health` + evento
  `overlay-health`). Nuevo ajuste **`mpo_mode`**: `"always"` (mostrar siempre, como hasta
  ahora) o `"performance"` (si detecta que está costando, **se auto-oculta** para no bajar
  los FPS). Comando **`overlay_mpo_diagnostics`**: monitores activos, refresco por monitor
  + si están **mezclados**, y **HAGS** (programación de GPU por hardware) — los
  bloqueadores de MPO. La UI (pantalla in-game + Ajustes → Métricas) muestra un **badge de
  salud** (Sin coste / Costando FPS) y, cuando cuesta, los **pasos concretos** para que
  Windows conceda el plano hardware (igualar refrescos, activar HAGS, evitar multimonitor).
- **Diagnóstico profundo del overlay (opt-in)** (`overlay_diag.rs`): activable con la
  variable de entorno `METEOR_OVERLAY_DEBUG=1` (cero coste si no se pone — no hay spam
  por tick). Vuelca a **stderr y a `<app log dir>\overlay-debug.log`**: el backend
  elegido, la **decisión de gating** cada vez que cambia (overlay on, juego, pid, fg_pid,
  si dibuja) con la **ventana en primer plano clasificada** (borderless/exclusiva vs
  ventana, rect vs monitor — la condición que decide el MPO), un **heartbeat cada 3 s**
  con la muestra en vivo (fps/frame/gpu/cpu/temp/ram) y —lo clave— el **modo de
  composición real del swapchain** vía `IDXGISwapChainMedia::GetFrameStatisticsMedia`:
  `OVERLAY` (plano hardware/MPO → sin coste) vs `COMPOSED` (DWM compone → input lag) vs
  `COMPOSITION_FAILURE`. Es el test definitivo en runtime para saber si el overlay
  realmente está en un plano hardware o forzando composición.
- **Saludo personalizado en Inicio**: el «Bienvenido de vuelta» del Home ahora
  incluye el **nombre del usuario** de Windows. Comando Rust `username()`
  (`GetUserNameExW`/`NameDisplay`, con fallback a `%USERNAME%`); el front toma el
  primer nombre y lo capitaliza. Clave i18n `home.welcomeBackName`.
- **Tutorial guiado interactivo** (`GuidedTour.tsx`): un *product tour* de 12 pasos
  que resalta cada función **sobre la UI real** (recorte tipo coachmark + tooltip
  flotante), con navegación por teclado y anclaje mediante atributos `data-tour`.
  Enfoque mixto: **auto-dispara** el menú contextual y la ficha de detalle, e
  **ilustra** Spotlight, overlay, selección múltiple y arrastrar. Arranca tras el
  primer escaneo y es re-lanzable.
- **Botón «?»** en la barra superior (y en Ajustes → Aplicación) para **relanzar
  el tutorial** cuando se quiera.
- **Idiomas Español / Inglés** con **react-i18next**:
  - Infraestructura en `src/i18n/` (config, catálogos `en.ts`/`es.ts`,
    `I18nProvider`) con cambio de idioma **en vivo** en todas las ventanas.
  - Ajuste `AppSettings.language` (`system` | `es` | `en`; por defecto sigue el
    idioma del sistema y cae a inglés). **Selector** en Ajustes → Aplicación.
  - Traducido: Sidebar, TopBar, Footer, Home, GameCard, Spotlight, Splash,
    IntroSplash, UpdatePrompt, ConfirmDialog, Onboarding, NotificationsPanel,
    OverlaySettingsScreen, los menús/toasts/diálogos de `page.tsx`, **toda la pantalla
    de Ajustes** (Aplicación + Sistema + Métricas), la **ficha de detalle**
    (`DetailView`, namespace `detail`) y el **tutorial guiado** (`GuidedTour`, namespace
    `tour`; pasos con texto enriquecido vía `<Trans>` + componentes de icono/Kbd). Los
    géneros/modos de IGDB respetan el idioma. **Todos los diálogos** traducidos
    (namespace `dialog`): añadir app, carátula, categorías (individual/lote/nueva/
    editar) y elementos ocultos. Barrido del frontend **completo**.
  - **Sinopsis bilingüe**: `translate.rs` ahora traduce **por idioma de UI**
    (`translate(app, text, lang)`, `tl=<lang>`, caché por `lang:hash`; `en`
    devuelve el original de IGDB). `details_cache_v2.json` guarda la sinopsis
    **original en inglés** y `art::details(name, lang)` la traduce a la salida en
    cada petición; `game_details(name, lang)` recibe el idioma y `DetailView`
    re-pide al cambiarlo. La caché v1 (español horneado) se descarta. **i18n 100%.**

### Cambiado
- **Métricas más ligeras durante el juego (menos CPU en segundo plano)**: tres recortes
  quirúrgicos al subsistema de telemetría sin cambiar la arquitectura (sigue sin inyección de
  DLL, HUD nativo MPO-friendly, muestreo *gated*):
  - **Watcher de playtime `O(juegos activos)` en vez de `O(todos los procesos)`** (`playtime.rs`):
    enumeraba **todos** los procesos del sistema y resolvía el exe path de cada uno **cada 5 s**
    (un syscall por proceso, 300+ procesos), incluso con un juego ya identificado. Ahora guarda
    el **PID** de cada sesión activa y entre escaneos completos solo hace una **comprobación de
    vida barata por PID** (`proc_alive`, `OpenProcess`+`GetExitCodeProcess`, 1 syscall por juego);
    el escaneo completo se espacia (`FULL_SCAN_SECS`=20 s) o se fuerza si hay un lanzamiento
    pendiente. El PID publicado al overlay sale del mapa de sesiones (sin re-escanear). Coste
    aceptado: un juego lanzado **fuera** de Meteor puede tardar hasta ~20 s en detectarse (los
    lanzados desde Meteor siguen en ~5 s).
  - **Sampler sin `sysinfo` en el bucle por segundo** (`metrics.rs` + nuevo `sysstat.rs`): CPU% y
    RAM ahora vía Win32 directo (`GetSystemTimes` / `GlobalMemoryStatusEx`) en vez del refresco de
    `sysinfo::System` cada tick. Mismo dato mostrado (CPU% global + RAM usada/total), sin enumerar
    procesos ni asignar.
  - **Micro-opts ADLX (AMD)** (`adlx_shim.cpp` + `amd.rs`): se **cachea `TotalVRAM`** (constante por
    GPU) en vez de pedirlo en cada sample, y la **lectura de FPS** (`GetCurrentFPS`) se **omite**
    cuando el overlay no muestra ninguna métrica de FPS.
  - **Parseo de PresentMon sin asignación por frame** (`presentmon.rs`): a 200-800 FPS el parser
    hacía `split(',').collect::<Vec>()` por cada present; ahora extrae solo la columna de frametime
    con `split(',').nth(idx)` (sin `Vec`), recortando CPU en NVIDIA a FPS altos.
  - **Diagnóstico MPO multimonitor** (`overlay_diag.rs`): el reporte de la ventana en primer plano
    ahora incluye **nº de monitores** y si el juego está en el **monitor primario** (el HUD se pinta
    en el primario; un juego en monitor secundario explica que el HUD no se vea y que se pierda MPO).
- **Overlay sin WebView2 residente durante el juego (el cambio de rendimiento real)**: la
  ventana `overlay` (WebView2) se creaba **oculta al arrancar y vivía toda la sesión**. Una
  WebView2 oculta **mantiene viva toda su pila Chromium** (proceso browser + renderer +
  **proceso GPU**), que **compite con el juego en la composición de DWM** — justo lo que el
  HUD nativo se construyó para evitar, anulado por esta ventana fantasma. Ahora el HUD lo
  pinta solo la ventana nativa y la WebView del overlay **únicamente sirve la pantalla de
  ajustes**, así que se crea **bajo demanda** (`ensure_overlay_window`) al abrir los ajustes
  in-game y se **destruye al cerrarlos** (`set_overlay_interactive(false)` → `close()`).
  Resultado: **cero procesos Chromium del overlay mientras se juega** (antes ~3 procesos +
  RAM + proceso GPU residentes). El tamaño del monitor para el HUD se lee ahora de la ventana
  `main` (siempre presente) y la hotkey de ajustes la gestiona Rust (`toggle_overlay_settings`),
  ya que la ventana puede no existir para recibir un `emit`.
- **Overlay solo DirectComposition: eliminado el fallback GDI (decisión de rendimiento)**:
  tras testeo profundo, una ventana GDI `UpdateLayeredWindow` **nunca** es elegible para MPO,
  así que siempre saca al juego de *independent-flip* → input lag. Era el peor caso justo
  cuando más dolía. Ahora el HUD es **solo DComp**: si DComp no inicializa, el HUD se
  **desactiva** en vez de caer a GDI (no se entrega un overlay garantizado-laggy). Se
  borró toda la maquinaria GDI del HUD de `overlay_native.rs`, que queda reducido a los
  helpers de ventana en primer plano. Además: **restaurado el gate de foco** (un
  diagnóstico lo dejó dibujando sobre el escritorio con el juego en segundo plano) y
  **eliminado el log de depuración por tick** del sampler y de DComp. Coste residual del
  overlay = NVML+sysinfo a 1 s + un *present* por segundo en plano hardware cuando hay MPO
  ≈ cero; donde DWM deniega MPO, la composición es inevitable para cualquier ventana sobre
  el juego (límite de DWM, no del código).
- **HUD del overlay reescrito a una ventana nativa (mucho más ligero)**: el HUD ya
  **no es WebView2**. Un overlay de Chromium transparente corría su propio proceso
  GPU compitiendo con el juego y forzaba composición de DWM (input lag, sensación de
  bajo framerate). Ahora se pinta en una **ventana Win32 *layered* nativa**
  (`overlay_native.rs`): dibujo GDI a un **DIB ARGB de 32 bits** + `UpdateLayeredWindow`,
  *content-sized*, sin redirection bitmap → compone al mínimo y es *MPO-friendly*, así
  el juego conserva su ruta de baja latencia. El sampler (`metrics.rs`) dibuja el HUD
  directo en vez de emitir eventos a un webview. La **pantalla de ajustes in-game**
  sigue en el WebView2 `overlay` (visible solo al abrirla; el HUD nativo se oculta
  mientras tanto vía `set_settings_open`).
- **Overlay hiperligero: backend DirectComposition + flip swapchain (MPO real)**: el
  HUD GDI-`UpdateLayeredWindow` es ciudadano MPO de segunda (DWM lo compone por la ruta
  de *redirection*). Nuevo backend `overlay_dcomp.rs` que dibuja con **DirectComposition
  + DXGI flip swapchain + Direct2D/DirectWrite** — la superficie que DWM **promociona a
  plano hardware (MPO)**, así el juego conserva *independent-flip* y el overlay no añade
  composición ni input lag. Un facade `overlay.rs` elige DComp y, si el init D3D/DComp
  falla, **desactiva el HUD** (sin fallback GDI, ver más abajo). Mejoras transversales: **present-on-change** (no se
  redibuja si el texto no cambió), reposición de ventana solo si cambia, y constructor de
  filas **compartido** entre ambos backends (la lista de métricas no diverge). Sin crates
  nuevas: solo features del crate `windows` ya presente.
- **HUD solo cuando el juego está en foco**: el sampler ya solo dibuja (y muestrea) si la
  ventana del juego es la de **primer plano**. Al hacer alt-tab, el HUD se oculta y el
  sampler entra en reposo — antes componía un topmost sobre el escritorio para nada (el
  conteo de tiempo de juego sigue igual).
- **FPS en NVIDIA solo con admin (sin intentos en vacío)**: PresentMon (ETW) exige
  elevación, que no cambia en runtime, así que ahora se comprueba **una vez** al arrancar
  el controlador: sin admin **no se intenta lanzar nunca** (cero overhead, cero *access
  denied*). En AMD el FPS lo da ADLX sin admin igualmente. Resultado: hiperligero por
  defecto en ambos vendors.
- **Atajos globales por defecto**: ahora **F9** (Spotlight), **F10** (alternar
  overlay) y **F11** (ajustes del overlay), en lugar de combinaciones con Ctrl+Shift.
- La UI muestra siempre el **atajo real** (el personalizado del usuario o el
  por defecto) mediante un formateador común (`lib/shortcuts.ts`): Footer,
  tutorial, Ajustes, panel de notificaciones y ajustes del overlay.

### Corregido
- **Overlay DirectComposition no iniciaba en GPUs AMD (HUD no aparecía)**: el swapchain
  de composición (`overlay_dcomp.rs`) se creaba con `Scaling: DXGI_SCALING_NONE`, que
  `CreateSwapChainForComposition` **rechaza** en muchos drivers (AMD incluido) con
  `DXGI_ERROR_INVALID_CALL` (`0x887A0001`); como el HUD in-game **no tiene fallback GDI**
  por diseño, fallaba el init y el overlay quedaba desactivado toda la sesión (las
  métricas se muestreaban bien, pero nada se dibujaba). Fix: usar `DXGI_SCALING_STRETCH`
  (el buffer va 1:1 con el contenido del HUD, así que no hay stretch real ni se pierde la
  elegibilidad MPO). De paso, `init()` ahora envuelve cada llamada DXGI/D3D/DComp en un
  paso etiquetado que registra **qué llamada concreta** falla en `overlay-debug.log`.
- **Panic en debug de sysinfo que tumbaba el watcher de playtime (y con él el overlay)**:
  sysinfo 0.30 hace `process_times/10_000_000 - 11_644_473_600` sin proteger la resta; si
  `GetProcessTimes` falla en un proceso protegido (queda 0), hay **underflow**. En release
  solo *wrap* (overflow-checks off → `start_time` basura que no leemos), pero en **debug
  hacía panic** y mataba el hilo del watcher — que es quien publica el juego al overlay, así
  que el HUD dejaba de aparecer. Fix: `[profile.dev.package.sysinfo] overflow-checks = false`
  (mismo comportamiento que release, solo para ese crate; nuestro código conserva los
  checks). Actualizar a 0.33.1 no servía: arrastra la misma resta.
- **Input lag del overlay en juegos *borderless* (clave)**: el HUD nativo llamaba a
  `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` para no salir en grabaciones, pero
  la *display affinity* mete la ventana en la composición protegida de DWM y la
  **descalifica de MPO**. Sin MPO, una ventana *topmost* sobre un juego borderless saca
  a DWM de *independent-flip* y lo pasa a *composed-flip* → input lag y sensación de
  bajo framerate. Se elimina la llamada: el HUD vuelve a ser candidato a plano hardware
  (MPO) y el juego conserva su ruta de baja latencia. **Trade-off**: el HUD ahora sí
  aparece en capturas/grabaciones.
- **PresentMon dejaba de reintentar en bucle sin admin (golpea a NVIDIA)**: si
  PresentMon fallaba al lanzarse (ETW exige admin → *access denied*) o moría enseguida
  con el juego aún abierto, el controlador **reintentaba `CreateProcess` cada 500 ms**
  durante toda la partida = hitches. Afecta sobre todo a NVIDIA, donde PresentMon es la
  **única** fuente de FPS. Ahora recuerda el PID fallido (`failed_pid`) y no reintenta
  hasta que cambia el juego.
- **PresentMon ya no corre en GPUs AMD**: con AMD el FPS lo da **ADLX** nativo (sin
  admin), pero PresentMon se lanzaba igual (sesión ETW + parseo CSV por frame) para un
  dato que se descartaba. Ahora el sampler marca `ADLX_FPS_ACTIVE` cuando ADLX entrega
  FPS y la puerta `want_fps()` deja PresentMon **en reposo**; si se selecciona una GPU
  NVML o ADLX deja de dar FPS, PresentMon retoma.
- **Autostart no arrancaba con admin permanente**: si el instalador marcaba Meteor
  como administrador (flag `RUNASADMIN`) y se activaba el inicio con Windows, no
  arrancaba. Causa: el autostart usaba la clave `HKCU\...\Run`, que Windows
  **bloquea en silencio** en el login para apps que requieren UAC. Ahora, cuando
  Meteor corre elevado, el autostart se gestiona con una **tarea programada**
  (`MeteorAutostart`, `/SC ONLOGON /RL HIGHEST`) que sí arranca elevado sin prompt;
  la clave Run muerta se limpia y hay **migración automática** en el arranque para
  usuarios que ya la tenían (`elevation.rs` + `set_autostart`/`get_autostart`).
- **Input lag con el overlay de métricas activado**: la ventana del overlay cubría
  todo el monitor y rompía el *independent flip / MPO* del juego (en
  borderless/ventana), añadiendo 1–2 frames de latencia pese a tener FPS altos.
  Ahora el overlay es una **caja pequeña pegada a la esquina** y solo se agranda a
  pantalla completa al abrir sus ajustes.
- **Input lag y stutter del overlay (2ª pasada)**: dos causas más, resueltas.
  (1) La ventana usaba una caja **fija de 340×400** aunque el HUD ocupara mucho
  menos; ahora se **redimensiona al tamaño real del HUD**, minimizando la superficie
  compuesta y favoreciendo que DWM la **promueva a un plano hardware (MPO)**. (2) El
  overlay re-aseraba *topmost* (toggle `NOTOPMOST→TOPMOST`) **en cada tick**, forzando
  recomposición de DWM y *stutter periódico*; ahora solo re-asierta cuando **cambia la
  ventana en primer plano** (`metrics.rs`, gate por `last_fg`).

---

## [0.0.8]

### Añadido
- **Sistema de ajustes del launcher** con configuración del overlay y módulos de
  integración con tiendas.
- **Selector de GPU + panel «Mi equipo»** en Ajustes: el sampler inicializa NVML y
  ADLX a la vez, comando `system_info` (CPU/RAM/SO, discos, placa base, pantallas y
  lista de GPUs).
- Atajos globales **configurables** (Spotlight / alternar overlay / ajustes del
  overlay) editables desde Ajustes.

### Cambiado
- Mejora en la **detección de aplicaciones** (ampliación de `apps_db`) y
  **actualización dinámica de los atajos** en caliente.

---

## [0.0.7]

### Añadido
- **Monitorización de rendimiento**:
  - **FPS / frametime** vía **PresentMon** (ETW, sin inyección de DLL; requiere el
    binario y ejecutar como administrador).
  - **Temperatura de CPU** vía sidecar **LibreHardwareMonitor** (`cputemp.exe`,
    requiere admin y driver compatible con HVCI).
  - **GPU AMD** mediante **ADLX** (uso/temp/VRAM/clock/power y FPS), además de NVML
    para NVIDIA.
- **Elevación a administrador** bajo demanda (`is_elevated`, `restart_as_admin`)
  con aviso en la pestaña Métricas, y soporte de privilegios en el instalador.

---

## [0.0.6]

### Añadido
- **Home / Dashboard** como vista por defecto: tarjetas de estadísticas (tiempo
  total, esta semana, sesiones, juegos jugados), **«Continuar jugando»**, rankings
  de **juegos más jugados** y **apps más usadas**, todo derivado del tiempo de juego.

---

## [0.0.5]

### Añadido
- **IntroSplash**: pantalla de intro breve en cada arranque.
- **Página de detalle** enriquecida con metadatos de IGDB (sinopsis, géneros,
  modos, temas, perspectiva, saga, duración para completar, tráilers, galería,
  juegos similares) y enlaces dinámicos.
- **Capturas del usuario** en el detalle (Steam + Windows Game Bar).

---

## [Base inicial] — Fundamentos del launcher

### Biblioteca y fuentes
- **Biblioteca unificada** con escáneres nativos por tienda, cada uno en su módulo
  Rust y mezclados en `get_library` con deduplicación por nombre:
  **Steam, Epic, GOG, EA, Ubisoft, Xbox, Battle.net** (flavors de WoW), más fuentes
  curadas (Riot, Rockstar, Amazon) y el catch-all del registro de Windows.
- **Apps manuales** añadidas por el usuario.
- **`apps_db`**: gran librería de firmas para separar **aplicaciones** de **juegos**.
- **Icono real del .exe** para apps sin carátula (extraído de los recursos PE con
  `pelite`, cacheado como `.ico`).

### Carátulas y metadatos
- **Carátulas desde IGDB** (`art.rs` + `igdb.rs`) con caché en tres capas (imagen en
  disco → URL cacheada → consulta a la API) y servidas por el protocolo `asset`.
- **Override manual** de carátula por URL o **arrastrando una imagen local**.
- **Traducción de la sinopsis** al español (`translate.rs`, Google Translate `gtx`).

### Gestión y UX
- **Ocultar** elementos, **favoritos**, **categorías** (con iconos, drag & drop,
  reordenado, menú contextual) y **reclasificar juego ↔ aplicación**.
- **Búsqueda fuzzy**, **ordenación** (nombre / más jugados / recientes) y
  **selección múltiple** con acciones en lote.
- **Spotlight global**: paleta de lanzamiento rápida con atajo global, funciona
  aunque Meteor esté minimizado.
- **Menú contextual**, **arrastrar y soltar** a Favoritos/categorías, confirmación
  de acciones destructivas y **footer de atajos**.

### Sistema y plataforma
- **Tiempo de juego**: watcher global (`playtime.rs` + `sysinfo`) que cronometra
  cualquier juego se lance como se lance, con historial de sesiones y recuperación
  tras cierre.
- **Discord Rich Presence** (`discord.rs`, IPC local, client id embebido).
- **Bandeja del sistema** (cerrar oculta a la bandeja) e **iniciar con Windows**
  (`tauri-plugin-autostart`).
- **Auto-actualización** desde GitHub Releases (firmada, con barra de progreso y
  relaunch).
- **Onboarding** de primer arranque con splash de carga y auto-escaneo.

### Overlay de métricas in-game (fase inicial)
- HUD que se pinta **sobre el juego** con GPU/CPU/RAM (NVML/sysinfo), ventana
  transparente click-through, configurable (posición, métricas), con hotkey global.

---

## Tema y arquitectura

- **Tauri 2 (Rust) + Next.js 16** con export estático; toda la lógica de sistema en
  comandos Rust. UI con paleta rojo + azul, tipografía Oxanium/Source Code Pro,
  esquinas rectas y efectos de carta (tilt 3D + glow).

[No publicado]: #no-publicado--trabajo-en-curso
[0.0.8]: #008
[0.0.7]: #007
[0.0.6]: #006
[0.0.5]: #005
