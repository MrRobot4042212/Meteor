use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Rich metadata for a game's detail page, resolved from IGDB and cached on disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameDetails {
    pub summary: Option<String>,
    /// IGDB aggregate rating, 0–100.
    pub rating: Option<u32>,
    pub rating_count: Option<u32>,
    pub release_year: Option<i32>,
    pub genres: Vec<String>,
    pub modes: Vec<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    /// Full screenshot image URLs (1080p).
    pub screenshots: Vec<String>,
}

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

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(8))
        .build()
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

        // `t_cover_big` is IGDB's standard portrait box-art (264×374), ideal for
        // our 2:3 cards.
        return Some(format!(
            "https://images.igdb.com/igdb/image/upload/t_cover_big/{image_id}.jpg"
        ));
    }

    None
}

/// Fetch rich metadata (summary, genres, rating, screenshots, companies…) for
/// any of the given name variants. Returns the first variant that matches.
pub fn fetch_details(variants: &[String]) -> Option<GameDetails> {
    #[derive(Deserialize)]
    struct ApiGame {
        #[serde(default)]
        name: String,
        summary: Option<String>,
        rating: Option<f64>,
        rating_count: Option<u32>,
        first_release_date: Option<i64>,
        #[serde(default)]
        genres: Vec<Named>,
        #[serde(default)]
        game_modes: Vec<Named>,
        #[serde(default)]
        involved_companies: Vec<Involved>,
        #[serde(default)]
        screenshots: Vec<Shot>,
    }
    #[derive(Deserialize)]
    struct Named {
        #[serde(default)]
        name: String,
    }
    #[derive(Deserialize)]
    struct Involved {
        #[serde(default)]
        developer: bool,
        #[serde(default)]
        publisher: bool,
        company: Option<Named>,
    }
    #[derive(Deserialize)]
    struct Shot {
        image_id: String,
    }

    let token = token()?;
    let agent = agent();
    let bearer = format!("Bearer {token}");

    for variant in variants {
        let escaped = variant.replace('\\', "\\\\").replace('"', "\\\"");
        let body = format!(
            "search \"{escaped}\"; fields name,summary,rating,rating_count,first_release_date,\
             genres.name,game_modes.name,involved_companies.company.name,\
             involved_companies.developer,involved_companies.publisher,screenshots.image_id; \
             limit 8;"
        );

        let Some(games): Option<Vec<ApiGame>> = agent
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
        let g = games
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(variant))
            .unwrap_or(&games[0]);

        let developer = g
            .involved_companies
            .iter()
            .find(|c| c.developer)
            .and_then(|c| c.company.as_ref())
            .map(|c| c.name.clone());
        let publisher = g
            .involved_companies
            .iter()
            .find(|c| c.publisher)
            .and_then(|c| c.company.as_ref())
            .map(|c| c.name.clone());

        // first_release_date is a unix timestamp; derive the year (avg-year secs
        // is precise enough for a display year, no chrono dependency needed).
        let release_year = g
            .first_release_date
            .map(|ts| 1970 + (ts as f64 / 31_556_952.0).floor() as i32);

        return Some(GameDetails {
            summary: g.summary.clone().filter(|s| !s.trim().is_empty()),
            rating: g.rating.map(|r| r.round() as u32),
            rating_count: g.rating_count,
            release_year,
            genres: g.genres.iter().map(|n| n.name.clone()).collect(),
            modes: g.game_modes.iter().map(|n| n.name.clone()).collect(),
            developer,
            publisher,
            screenshots: g
                .screenshots
                .iter()
                .map(|s| {
                    format!(
                        "https://images.igdb.com/igdb/image/upload/t_1080p/{}.jpg",
                        s.image_id
                    )
                })
                .collect(),
        });
    }

    None
}
