use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const CACHE_FILE: &str = "translate_cache.json";

/// Normalize a UI language tag ("es", "en-US", "es-ES"…) to a 2-letter target
/// code. Empty/unknown falls back to English.
fn target_lang(lang: &str) -> String {
    let code = lang.trim().get(..2).unwrap_or("en").to_lowercase();
    if code.is_empty() {
        "en".to_string()
    } else {
        code
    }
}

/// Translate text to `lang` via Google Translate's free endpoint, cached on disk
/// (keyed by target language + source text) so each string is fetched once per
/// language. IGDB summaries are already English, so `en` returns the original
/// untouched. On any failure the original text is returned, so the summary always
/// renders.
pub fn translate(app: &AppHandle, text: &str, lang: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    let target = target_lang(lang);
    // Source is English (IGDB); nothing to do for English UIs.
    if target == "en" {
        return text.to_string();
    }

    let key = format!("{target}:{}", hash(text));
    let mut cache = load_cache(app);
    if let Some(hit) = cache.get(&key) {
        return hit.clone();
    }

    let translated = google_translate(text, &target).unwrap_or_else(|| text.to_string());
    cache.insert(key, translated.clone());
    let _ = save_cache(app, &cache);
    translated
}

/// Unofficial gtx endpoint: returns a nested JSON array; the first element holds
/// the translated sentence segments, which we concatenate.
fn google_translate(text: &str, target: &str) -> Option<String> {
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={target}&dt=t&q={}",
        urlencoding::encode(text)
    );
    let json: serde_json::Value = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(10))
        .build()
        .get(&url)
        .set("User-Agent", "Mozilla/5.0")
        .call()
        .ok()?
        .into_json()
        .ok()?;

    let segments = json.get(0)?.as_array()?;
    let mut out = String::new();
    for seg in segments {
        if let Some(s) = seg.get(0).and_then(|v| v.as_str()) {
            out.push_str(s);
        }
    }
    (!out.trim().is_empty()).then_some(out)
}

fn hash(text: &str) -> String {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("No se pudo obtener la carpeta de datos: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(CACHE_FILE))
}

fn load_cache(app: &AppHandle) -> HashMap<String, String> {
    cache_path(app)
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

fn save_cache(app: &AppHandle, cache: &HashMap<String, String>) -> Result<(), String> {
    let path = cache_path(app)?;
    let data = serde_json::to_string(cache).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}
