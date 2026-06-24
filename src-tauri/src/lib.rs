#[cfg(windows)]
mod amd;
mod appicons;
mod apps_db;
mod art;
mod cputemp;
#[cfg(windows)]
mod elevation;
mod discord;
mod battlenet;
mod ea;
mod epic;
mod files;
mod gog;
mod igdb;
mod launcher;
mod metrics;
mod models;
mod playtime;
mod presentmon;
mod screenshots;
mod steam;
mod storage;
mod system;
mod translate;
mod ubisoft;
mod windows_apps;
mod xbox;

use models::{Category, Game, GameSource, AppSettings};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

/// Return the unified library across every supported source, sorted by name.
///
/// Each scanner runs independently: a failure in one source (store not
/// installed, corrupt manifest…) degrades to an empty list for that source
/// instead of failing the whole call. Sources are merged in priority order and
/// deduplicated by name, so a game owned on several stores shows up once with
/// the best available metadata (Steam first, since it ships CDN cover art).
#[tauri::command]
fn get_library(app: AppHandle) -> Result<Vec<Game>, String> {
    let mut games: Vec<Game> = Vec::new();
    games.extend(steam::scan().unwrap_or_default());
    games.extend(epic::scan().unwrap_or_default());
    games.extend(gog::scan().unwrap_or_default());
    games.extend(xbox::scan().unwrap_or_default());
    games.extend(ea::scan().unwrap_or_default());
    games.extend(ubisoft::scan().unwrap_or_default());
    // Battle.net (WoW flavors) before the generic scan so its richer per-flavor
    // entries win the dedup over the single "World of Warcraft" registry entry.
    games.extend(battlenet::scan().unwrap_or_default());
    // Generic registry scan last, so dedup keeps the richer native entry when a
    // game is also found by a dedicated scanner.
    games.extend(windows_apps::scan().unwrap_or_default());
    games.extend(storage::load_manual(&app)?);

    // Deduplicate by name, keeping the first (highest-priority) occurrence.
    let mut seen = HashSet::new();
    games.retain(|g| seen.insert(g.name.to_lowercase()));

    // Drop entries the user has hidden (false positives from the generic scan).
    let hidden = storage::load_hidden(&app);
    if !hidden.is_empty() {
        let (visible, hidden_games): (Vec<Game>, Vec<Game>) = games.into_iter().partition(|g| !hidden.iter().any(|h| h == &g.id));
        games = visible;
        let _ = storage::save_hidden_cache(&app, &hidden_games);
    } else {
        let _ = storage::save_hidden_cache(&app, &[]);
    }

    // User-set cover overrides win over whatever each source provided.
    let overrides = storage::load_cover_overrides(&app);
    if !overrides.is_empty() {
        for game in &mut games {
            if let Some(url) = overrides.get(&game.id) {
                game.cover_url = Some(url.clone());
            }
        }
    }

    // User overlays: favorites and manual categories, keyed by game id.
    let favorites = storage::load_favorites(&app);
    let categories = storage::load_categories(&app);
    if !favorites.is_empty() || !categories.is_empty() {
        for game in &mut games {
            if favorites.iter().any(|f| f == &game.id) {
                game.favorite = true;
            }
            if let Some(cats) = categories.get(&game.id) {
                game.categories = cats.clone();
            }
        }
    }

    // User override: reclassify an entry as app/game (fixes mis-detection). Since
    // the library is re-scanned each call, "game" only needs to undo an App
    // detection (store sources are already games), so the real source is kept
    // whenever possible and removing the override self-heals on the next scan.
    let type_overrides = storage::load_type_overrides(&app);
    if !type_overrides.is_empty() {
        for game in &mut games {
            match type_overrides.get(&game.id).map(String::as_str) {
                Some("app") => game.source = GameSource::App,
                Some("game") if game.source == GameSource::App => {
                    game.source = GameSource::Windows;
                }
                _ => {}
            }
        }
    }

    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    // Persist for instant startup next time and for the playtime watcher's index.
    write_library_cache(&app, &games);
    Ok(games)
}

/// Path of the on-disk snapshot of the last computed library.
fn library_cache_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().ok()?;
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("library_cache.json"))
}

fn write_library_cache(app: &AppHandle, games: &[Game]) {
    if let (Some(path), Ok(data)) = (library_cache_path(app), serde_json::to_string(games)) {
        let _ = std::fs::write(path, data);
    }
}

/// The last computed library from disk (empty if never scanned). The frontend
/// paints this instantly, then calls `get_library` to refresh in the background.
#[tauri::command]
fn cached_library(app: AppHandle) -> Result<Vec<Game>, String> {
    let Some(path) = library_cache_path(&app) else {
        return Ok(Vec::new());
    };
    let list = std::fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default();
    Ok(list)
}

/// Set a manual cover URL for a game id (empty/None clears it). Overrides always
/// take precedence over auto-resolved artwork.
#[tauri::command]
fn set_cover(app: AppHandle, id: String, url: Option<String>) -> Result<(), String> {
    storage::set_cover_override(&app, &id, url.as_deref())
}

/// Save a dropped/picked local image as a game's cover and set it as the override.
/// Returns the saved local path (rendered via the asset protocol).
#[tauri::command]
fn set_cover_image(
    app: AppHandle,
    id: String,
    data: Vec<u8>,
    ext: String,
) -> Result<String, String> {
    let path = storage::save_cover_image(&app, &id, &data, &ext)?;
    storage::set_cover_override(&app, &id, Some(&path))?;
    Ok(path)
}

/// Resolve a cover image for a game name via IGDB, cached on disk. The frontend
/// calls this lazily for entries without artwork.
#[tauri::command]
fn resolve_cover(app: AppHandle, name: String) -> Result<Option<String>, String> {
    Ok(art::resolve(&app, &name))
}

/// Wipe the cover cache (URLs + downloaded images) so everything re-resolves.
#[tauri::command]
fn clear_cover_cache(app: AppHandle) -> Result<(), String> {
    art::clear_cache(&app)
}

/// Reclassify an entry as an application or a game (`"app"` / `"game"`), or clear
/// the override (any other value) to fall back to auto-detection.
#[tauri::command]
fn set_game_type(app: AppHandle, id: String, kind: String) -> Result<(), String> {
    let k = kind.as_str();
    storage::set_type_override(&app, &id, if k == "app" || k == "game" { Some(k) } else { None })
}

/// Hide a game from the library (e.g. a non-game picked up by the registry scan).
#[tauri::command]
fn hide_game(app: AppHandle, id: String) -> Result<(), String> {
    storage::set_hidden(&app, &id, true)
}

/// Unhide a game from the library.
#[tauri::command]
fn unhide_game(app: AppHandle, id: String) -> Result<(), String> {
    storage::set_hidden(&app, &id, false)
}

/// Get the cached metadata of hidden games.
#[tauri::command]
fn get_hidden_library(app: AppHandle) -> Result<Vec<Game>, String> {
    storage::load_hidden_cache(&app)
}

/// Number of currently-hidden games (shown in settings so they can be restored).
#[tauri::command]
fn hidden_count(app: AppHandle) -> Result<usize, String> {
    Ok(storage::load_hidden(&app).len())
}

/// Restore every hidden game.
#[tauri::command]
fn restore_hidden(app: AppHandle) -> Result<(), String> {
    storage::clear_hidden(&app)
}

#[tauri::command]
fn add_manual_app(
    app: AppHandle,
    name: String,
    executable: String,
    cover_url: Option<String>,
) -> Result<Game, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("El nombre no puede estar vacío".into());
    }

    let mut manual = storage::load_manual(&app)?;
    let id = format!(
        "manual:{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis()
    );

    let game = Game {
        id,
        name,
        source: GameSource::Manual,
        app_id: None,
        executable: Some(executable),
        install_dir: None,
        cover_url: cover_url.filter(|s| !s.trim().is_empty()),
        launch_uri: None,
        favorite: false,
        categories: Vec::new(),
    };

    manual.push(game.clone());
    storage::save_manual(&app, &manual)?;
    Ok(game)
}

/// Remove a manually-added app. Store-managed entries are ignored.
#[tauri::command]
fn remove_game(app: AppHandle, id: String) -> Result<(), String> {
    let mut manual = storage::load_manual(&app)?;
    let before = manual.len();
    manual.retain(|g| g.id != id);
    if manual.len() != before {
        storage::save_manual(&app, &manual)?;
    }
    Ok(())
}

/// Mark or unmark a game as favorite (applies to any source, not just manual).
#[tauri::command]
fn set_favorite(app: AppHandle, id: String, favorite: bool) -> Result<(), String> {
    storage::set_favorite(&app, &id, favorite)
}

/// Replace the manual category list for a game id (empty list clears it).
#[tauri::command]
fn set_categories(app: AppHandle, id: String, categories: Vec<String>) -> Result<(), String> {
    storage::set_categories(&app, &id, &categories)
}

/// Every explicitly-created category with its icon (persist even with zero games).
#[tauri::command]
fn list_categories(app: AppHandle) -> Result<Vec<Category>, String> {
    Ok(storage::load_categories_meta(&app))
}

/// Create a category by name, optionally with an icon key from the bundled set.
#[tauri::command]
fn add_category(app: AppHandle, name: String, icon: Option<String>) -> Result<(), String> {
    storage::add_category_name(&app, &name, icon.as_deref())
}

/// Set (or clear, with None) the icon key for an existing category.
#[tauri::command]
fn set_category_icon(app: AppHandle, name: String, icon: Option<String>) -> Result<(), String> {
    storage::set_category_icon(&app, &name, icon.as_deref())
}

/// Delete a category and strip it from every game.
#[tauri::command]
fn remove_category(app: AppHandle, name: String) -> Result<(), String> {
    storage::remove_category_name(&app, &name)
}

/// Rename a category everywhere (merges if the new name already exists).
#[tauri::command]
fn rename_category(app: AppHandle, old: String, new: String) -> Result<(), String> {
    storage::rename_category_name(&app, &old, &new)
}

/// Persist the explicit category order (as shown in the sidebar).
#[tauri::command]
fn set_category_order(app: AppHandle, names: Vec<String>) -> Result<(), String> {
    storage::set_category_order(&app, &names)
}

/// Rich IGDB metadata for the detail page (summary, genres, rating, shots…).
#[tauri::command]
fn game_details(app: AppHandle, name: String) -> Result<Option<igdb::GameDetails>, String> {
    Ok(art::details(&app, &name))
}

/// Accumulated play stats (seconds + last played) for a game id.
#[tauri::command]
fn get_playtime(app: AppHandle, id: String) -> Result<playtime::PlayStat, String> {
    Ok(playtime::get(&app, &id))
}

/// Play stats for every tracked game id (for sorting the library).
#[tauri::command]
fn all_playtime(
    app: AppHandle,
) -> Result<std::collections::HashMap<String, playtime::PlayStat>, String> {
    Ok(playtime::all(&app))
}

/// Total size in bytes of a directory (for the detail page's file info).
#[tauri::command]
fn dir_size(path: String) -> Result<u64, String> {
    files::dir_size(&path)
}

/// Extract the real icon embedded in an app's executable (cached PNG path), used
/// as the icon for apps without a cover or known brand logo.
#[tauri::command]
fn app_icon(app: AppHandle, path: String) -> Result<Option<String>, String> {
    Ok(appicons::extract(&app, &path))
}

/// Bring the main window to the front (used by the tray and Spotlight).
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Whether Meteor is set to launch on Windows login.
#[tauri::command]
fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|e| format!("Failed to read autostart: {e}"))
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let auto = app.autolaunch();
    // Idempotente: si ya está en el estado pedido no hacemos nada. Evita que
    // `disable()` falle con "el sistema no puede encontrar el archivo
    // especificado (os error 2)" al borrar la clave Run del registro cuando
    // nunca estuvo activado (y lo simétrico al activar uno ya activo).
    let already = auto.is_enabled().unwrap_or(false);
    if enabled == already {
        return Ok(());
    }
    if enabled {
        auto.enable().map_err(|e| e.to_string())
    } else {
        auto.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn get_app_settings(state: tauri::State<'_, std::sync::Mutex<AppSettings>>) -> Result<AppSettings, String> {
    Ok(state.lock().unwrap().clone())
}

#[tauri::command]
fn system_info() -> Result<system::SystemInfo, String> {
    Ok(system::collect())
}

/// Whether Meteor is running elevated (admin). Admin is required for CPU temp and
/// for FPS on NVIDIA (PresentMon). False on non-Windows.
#[tauri::command]
fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        elevation::is_elevated()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Relaunch Meteor as administrator (UAC prompt), then exit this instance.
#[tauri::command]
fn restart_as_admin(app: AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        elevation::relaunch_elevated()?;
        // Let the command response flush, then quit so only the elevated copy runs.
        let handle = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(400));
            handle.exit(0);
        });
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("Solo disponible en Windows.".into())
    }
}

#[tauri::command]
fn set_app_settings(
    app: AppHandle,
    state: tauri::State<'_, std::sync::Mutex<AppSettings>>,
    settings: AppSettings,
) -> Result<(), String> {
    storage::save_settings(&app, &settings);
    apply_overlay_settings(&app, &settings);
    *state.lock().unwrap() = settings;
    Ok(())
}

/// Apply the overlay config live: update the sampler and, when disabling, hide the
/// overlay window immediately and tell it to refresh its config.
fn apply_overlay_settings(app: &AppHandle, settings: &AppSettings) {
    let fps_wanted = settings.overlay.show_fps || settings.overlay.show_frametime;
    metrics::configure(
        settings.overlay.enabled,
        settings.overlay.interval_ms,
        fps_wanted,
        settings.overlay.show_cpu_temp,
    );
    metrics::set_gpu(settings.overlay.gpu.clone());
    if !settings.overlay.enabled {
        if let Some(w) = app.get_webview_window("overlay") {
            let _ = w.hide();
        }
    }
    // The overlay window re-reads settings (position, which metrics) on this event.
    let _ = app.emit_to("overlay", "overlay-config", ());
}

/// Toggle the overlay on/off (the global hotkey). Persists and applies live.
fn toggle_overlay(app: &AppHandle) {
    if let Some(state) = app.try_state::<std::sync::Mutex<AppSettings>>() {
        let mut s = state.lock().unwrap().clone();
        s.overlay.enabled = !s.overlay.enabled;
        storage::save_settings(app, &s);
        apply_overlay_settings(app, &s);
        *state.lock().unwrap() = s;
    }
}
/// The saved Discord Rich Presence client id (empty = disabled).
#[tauri::command]
fn get_discord_client_id(app: AppHandle) -> Result<String, String> {
    Ok(storage::load_discord_client_id(&app))
}

/// Save the Discord client id and apply it live to the presence watcher.
#[tauri::command]
fn set_discord_client_id(app: AppHandle, id: String) -> Result<(), String> {
    storage::save_discord_client_id(&app, &id)?;
    discord::set_client_id(&id);
    Ok(())
}

/// Open a folder in the OS file manager.
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    files::open_path(&path)
}

/// The user's own screenshots for a game (Steam + Windows Game Bar).
#[tauri::command]
fn user_screenshots(app: AppHandle, game: Game) -> Result<Vec<String>, String> {
    Ok(screenshots::user_screenshots(&app, &game))
}

#[tauri::command]
fn launch_game(game: Game) -> Result<(), String> {
    launcher::launch(&game)
    // Playtime is accumulated by the global watcher (see `playtime::start`),
    // which times any library game regardless of how it was launched.
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Launch on login (Windows registry Run key). The MacosLauncher arg is
        // ignored on Windows; no launch args needed.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // In-app auto-update (checks GitHub Releases) + relaunch after install.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Closing the main window hides Meteor to the tray instead of quitting,
        // so the playtime/Discord/Spotlight watchers keep running. Real quit is
        // the tray's "Salir" item.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let minimize = window
                        .app_handle()
                        .try_state::<std::sync::Mutex<AppSettings>>()
                        .map(|s| s.lock().unwrap().minimize_to_tray)
                        .unwrap_or(true);
                    
                    if minimize {
                        api.prevent_close();
                        let _ = window.hide();
                    } else {
                        // Let it close, which exits the app.
                    }
                }
            }
        })
        .plugin(
            // Global Spotlight hotkey: bring Meteor up and open the launcher palette
            // from anywhere. The handler runs for our one registered shortcut.
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    // Ctrl+Shift+O toggles the metrics overlay; Shift+Space opens Spotlight.
                    if *shortcut
                        == Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyO)
                    {
                        toggle_overlay(app);
                    } else {
                        show_main(app);
                        let _ = app.emit("open-spotlight", ());
                    }
                })
                .build(),
        )
        .setup(|app| {
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
            
            let handle = app.handle().clone();
            let settings = storage::load_settings(&handle);
            // Apply the saved overlay config to the sampler before it starts.
            metrics::configure(
                settings.overlay.enabled,
                settings.overlay.interval_ms,
                settings.overlay.show_fps || settings.overlay.show_frametime,
                settings.overlay.show_cpu_temp,
            );
            metrics::set_gpu(settings.overlay.gpu.clone());
            app.manage(std::sync::Mutex::new(settings));

            // Full-screen transparent, click-through, always-on-top overlay window
            // for in-game metrics. Created hidden; the metrics sampler shows it only
            // while a game is running and the overlay is enabled. It loads the same
            // bundle and renders the HUD based on its window label.
            {
                use tauri::{WebviewUrl, WebviewWindowBuilder};
                if let Ok(overlay) = WebviewWindowBuilder::new(
                    app,
                    "overlay",
                    WebviewUrl::App("index.html".into()),
                )
                .title("Meteor Overlay")
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .shadow(false)
                .focused(false)
                .visible(false)
                .build()
                {
                    // Cover the primary monitor and let clicks pass through to the game.
                    if let Ok(Some(mon)) = overlay.primary_monitor() {
                        let size = mon.size();
                        let _ = overlay.set_size(tauri::PhysicalSize::new(size.width, size.height));
                        let _ = overlay.set_position(tauri::PhysicalPosition::new(0, 0));
                    }
                    let _ = overlay.set_ignore_cursor_events(true);
                }
            }

            // Close any play sessions left dangling by a previous crash/force-quit,
            // then start the global watcher that times games however they launch.
            playtime::reconcile(&handle);
            // Load the saved Discord client id so the watcher can set Rich Presence.
            discord::set_client_id(&storage::load_discord_client_id(&handle));
            // Start the metrics sampler (idle until the overlay is on and a game runs),
            // the PresentMon controller (idle until FPS is wanted + a game runs) and the
            // CPU-temp sidecar controller (idle until CPU temp is wanted + a game runs).
            metrics::start(handle.clone());
            presentmon::start(handle.clone());
            cputemp::start(handle.clone());
            playtime::start(handle);
            // Register the global shortcuts: Spotlight (Shift+Space) and the metrics
            // overlay toggle (Ctrl+Shift+O).
            let spotlight = Shortcut::new(Some(Modifiers::SHIFT), Code::Space);
            let _ = app.global_shortcut().register(spotlight);
            let overlay_toggle =
                Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyO);
            let _ = app.global_shortcut().register(overlay_toggle);

            // System tray: Meteor lives in the tray so the watchers keep running
            // after the window is closed. Left-click or "Mostrar Meteor" reopens
            // the window; "Salir" really quits.
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

                let show = MenuItem::with_id(app, "show", "Mostrar Meteor", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show, &quit])?;
                let _tray = TrayIconBuilder::with_id("main")
                    .icon(app.default_window_icon().unwrap().clone())
                    .tooltip("Meteor")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => show_main(app),
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            show_main(tray.app_handle());
                        }
                    })
                    .build(app)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_library,
            resolve_cover,
            set_cover,
            set_cover_image,
            clear_cover_cache,
            hide_game,
            unhide_game,
            get_hidden_library,
            hidden_count,
            restore_hidden,
            set_game_type,
            add_manual_app,
            remove_game,
            set_favorite,
            set_categories,
            list_categories,
            add_category,
            set_category_icon,
            remove_category,
            rename_category,
            set_category_order,
            game_details,
            get_playtime,
            all_playtime,
            cached_library,
            dir_size,
            app_icon,
            get_discord_client_id,
            set_discord_client_id,
            get_autostart,
            set_autostart,
            get_app_settings,
            set_app_settings,
            system_info,
            is_elevated,
            restart_as_admin,
            open_path,
            user_screenshots,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación Tauri");
}
