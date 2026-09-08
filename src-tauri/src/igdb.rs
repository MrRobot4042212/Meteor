use serde::Deserialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// IGDB (Twitch) app credentials. **Build-time environment only**: they are read
// from `IGDB_CLIENT_ID` / `IGDB_CLIENT_SECRET` when the binary is compiled (CI
// secrets, or a gitignored `.env` locally — see `.env.example`).
//
// There is deliberately **no fallback constant**: an embedded client secret is
// extractable from any shipped binary, so a build without the variables simply
// ships without IGDB cover lookups (`resolve_cover` returns `None` and logs
// once) instead of leaking one shared credential to every user.
const CLIENT_ID: Option<&str> = option_env!("IGDB_CLIENT_ID");
const CLIENT_SECRET: Option<&str> = option_env!("IGDB_CLIENT_SECRET");

/// Warn once (not per lookup) when the build has no IGDB credentials.
static MISSING_LOGGED: OnceLock<()> = OnceLock::new();

/// The build-time credentials, or `None` (logged once) when this binary was
/// built without them.
fn credentials() -> Option<(&'static str, &'static str)> {
    match (CLIENT_ID, CLIENT_SECRET) {
        (Some(id), Some(secret)) if !id.trim().is_empty() && !secret.trim().is_empty() => {
            Some((id, secret))
        }
        _ => {
            MISSING_LOGGED.get_or_init(|| {
                eprintln!(
                    "[igdb] no IGDB credentials in this build (IGDB_CLIENT_ID / IGDB_CLIENT_SECRET); cover lookups are disabled"
                );
            });
            None
        }
    }
}

/// Cached Twitch app access token (IGDB auths through Twitch). Tokens last ~60
/// days; we cache it process-wide and refresh shortly before expiry.
struct Token {
    value: String,
    expires_at: u64,
}
static TOKEN: Mutex<Option<Token>> = Mutex::new(None);

/// Process-wide HTTP agent: reuses the TCP connection pool across all IGDB cover
/// lookups so each call doesn't pay the TLS/TCP handshake cost from scratch.
static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

fn agent() -> &'static ureq::Agent {
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(6))
            .timeout_read(Duration::from_secs(8))
            .build()
    })
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Get a valid Twitch app token, reusing the cached one when possible.
fn token() -> Option<String> {
    let (client_id, client_secret) = credentials()?;
    let mut guard = TOKEN.lock().ok()?;
    if let Some(tok) = guard.as_ref() {
        // 60s safety margin so a token doesn't expire mid-request.
        if now() + 60 < tok.expires_at {
            return Some(tok.value.clone());
        }
    }

    #[derive(Deserialize)]
    struct TokenResp {
        access_token: String,
        expires_in: u64,
    }

    let resp: TokenResp = agent()
        .post("https://id.twitch.tv/oauth2/token")
        .query("client_id", client_id)
        .query("client_secret", client_secret)
        .query("grant_type", "client_credentials")
        .call()
        .ok()?
        .into_json()
        .ok()?;

    let value = resp.access_token;
    *guard = Some(Token {
        value: value.clone(),
        expires_at: now() + resp.expires_in,
    });
    Some(value)
}

/// Resolve a vertical cover via IGDB for any of the given name variants.
pub fn resolve_cover(variants: &[String]) -> Option<String> {
    #[derive(Deserialize)]
    struct Game {
        #[serde(default)]
        name: String,
        cover: Option<Cover>,
    }
    #[derive(Deserialize)]
    struct Cover {
        image_id: String,
    }

    let (client_id, _) = credentials()?;
    let token = token()?;
    let agent = agent();
    let bearer = format!("Bearer {token}");

    for variant in variants {
        // Apicalypse query: search by name, only games that have cover art.
        let escaped = variant.replace('\\', "\\\\").replace('"', "\\\"");
        let body =
            format!("search \"{escaped}\"; fields name,cover.image_id; where cover != null; limit 6;");

        let Some(games): Option<Vec<Game>> = agent
            .post("https://api.igdb.com/v4/games")
            .set("Client-ID", client_id)
            .set("Authorization", &bearer)
            .set("Accept", "application/json")
            .send_string(&body)
            .ok()
            .and_then(|r| r.into_json().ok())
        else {
            continue;
        };
        if games.is_empty() {
            continue;
        }

        // Prefer an exact (case-insensitive) name match, else the first hit.
        let chosen = games
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(variant) && g.cover.is_some())
            .or_else(|| games.iter().find(|g| g.cover.is_some()))?;
        let image_id = &chosen.cover.as_ref()?.image_id;

        // `t_cover_big_2x` is the high-res portrait box-art (528×748) — twice
        // `t_cover_big`, so covers stay crisp on HiDPI screens and the detail hero.
        return Some(format!(
            "https://images.igdb.com/igdb/image/upload/t_cover_big_2x/{image_id}.jpg"
        ));
    }

    None
}
