# Changelog

Todas las novedades destacables de **Meteor** (launcher de escritorio que unifica
juegos y apps de varias tiendas en una sola biblioteca).

El formato sigue, a grandes rasgos, [Keep a Changelog](https://keepachangelog.com/es-ES/)
y el proyecto usa versionado semántico aproximado. Las fechas son orientativas.

---

## [No publicado] — Trabajo en curso

### Añadido
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
- **Atajos globales por defecto**: ahora **F9** (Spotlight), **F10** (alternar
  overlay) y **F11** (ajustes del overlay), en lugar de combinaciones con Ctrl+Shift.
- La UI muestra siempre el **atajo real** (el personalizado del usuario o el
  por defecto) mediante un formateador común (`lib/shortcuts.ts`): Footer,
  tutorial, Ajustes, panel de notificaciones y ajustes del overlay.

### Corregido
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
