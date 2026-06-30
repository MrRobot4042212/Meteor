use serde::Deserialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Built-in IGDB (Twitch) app credentials so cover art works out of the box with
// no per-user setup. Can be overridden at build time via env vars.
// NOTE: an embedded client secret is extractable from the binary; if it ever
// leaks/gets rate-limited, rotate it at dev.twitch.tv and rebuild.
const CLIENT_ID: &str = match option_env!("IGDB_CLIENT_ID") {
    Some(v) => v,
    None => "l2bm4pzvvdnxpj3avnn50n77d6pia6",
};
const CLIENT_SECRET: &str = match option_env!("IGDB_CLIENT_SECRET") {
    Some(v) => v,
    None => "bv2nd5hfk9gzmfem071eytynvbjcz0",
};

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
        .query("client_id", CLIENT_ID)
        .query("client_secret", CLIENT_SECRET)
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
            .set("Client-ID", CLIENT_ID)
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
