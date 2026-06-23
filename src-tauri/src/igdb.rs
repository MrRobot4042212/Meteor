use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Rich metadata for a game's detail page, resolved from IGDB and cached on disk.
/// New fields are `#[serde(default)]` so older cached entries still deserialize.
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
    /// IGDB themes (e.g. "Action", "Horror").
    #[serde(default)]
    pub themes: Vec<String>,
    /// Player perspectives (e.g. "First person").
    #[serde(default)]
    pub perspectives: Vec<String>,
    /// Franchise / series name, if any.
    #[serde(default)]
    pub franchise: Option<String>,
    /// Promotional artwork image URLs (1080p).
    #[serde(default)]
    pub artworks: Vec<String>,
    /// YouTube video ids for trailers/clips.
    #[serde(default)]
    pub videos: Vec<String>,
    /// A few similar games for discovery.
    #[serde(default)]
    pub similar: Vec<SimilarGame>,
    /// Time-to-beat (seconds) from IGDB's official `game_time_to_beats` endpoint.
    #[serde(default)]
    pub time_to_beat: Option<TimeToBeat>,
    /// Websites for the game (wikis, official, reddit, etc.).
    #[serde(default)]
    pub websites: Vec<Website>,
}

/// A related website for the game.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Website {
    pub category: u32,
    pub url: String,
}

/// A related game suggestion (name + cover for a thumbnail).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimilarGame {
    pub name: String,
    pub cover_url: Option<String>,
}

/// How long the game takes to beat, in seconds (IGDB aggregated).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimeToBeat {
    pub hastily: Option<u32>,
    pub normally: Option<u32>,
    pub completely: Option<u32>,
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

        // `t_cover_big_2x` is the high-res portrait box-art (528×748) — twice
        // `t_cover_big`, so covers stay crisp on HiDPI screens and the detail hero.
        return Some(format!(
            "https://images.igdb.com/igdb/image/upload/t_cover_big_2x/{image_id}.jpg"
        ));
    }

    None
}

/// Build a full IGDB image URL for a given size preset and image id.
fn img_url(size: &str, image_id: &str) -> String {
    format!("https://images.igdb.com/igdb/image/upload/{size}/{image_id}.jpg")
}

/// Fetch rich metadata (summary, genres, rating, media, similar games, themes,
/// time-to-beat…) for any of the given name variants. Returns the first match.
pub fn fetch_details(variants: &[String]) -> Option<GameDetails> {
    #[derive(Deserialize)]
    struct ApiGame {
        #[serde(default)]
        id: u64,
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
        themes: Vec<Named>,
        #[serde(default)]
        player_perspectives: Vec<Named>,
        franchise: Option<Named>,
        #[serde(default)]
        involved_companies: Vec<Involved>,
        #[serde(default)]
        screenshots: Vec<Shot>,
        #[serde(default)]
        artworks: Vec<Shot>,
        #[serde(default)]
        videos: Vec<Video>,
        #[serde(default)]
        similar_games: Vec<Similar>,
        #[serde(default)]
        websites: Vec<Website>,
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
    #[derive(Deserialize)]
    struct Video {
        #[serde(default)]
        video_id: String,
    }
    #[derive(Deserialize)]
    struct Cover {
        image_id: String,
    }
    #[derive(Deserialize)]
    struct Similar {
        #[serde(default)]
        name: String,
        cover: Option<Cover>,
    }

    let token = token()?;
    let agent = agent();
    let bearer = format!("Bearer {token}");

    for variant in variants {
        let escaped = variant.replace('\\', "\\\\").replace('"', "\\\"");
        let body = format!(
            "search \"{escaped}\"; fields name,id,summary,rating,rating_count,first_release_date,\
             genres.name,game_modes.name,themes.name,player_perspectives.name,franchise.name,\
             involved_companies.company.name,involved_companies.developer,\
             involved_companies.publisher,screenshots.image_id,artworks.image_id,\
             videos.video_id,similar_games.name,similar_games.cover.image_id,\
             websites.category,websites.url; limit 8;"
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

        // Official time-to-beat (separate endpoint, keyed by the game id).
        let time_to_beat = fetch_time_to_beat(&agent, &bearer, g.id);

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
                .map(|s| img_url("t_1080p", &s.image_id))
                .collect(),
            themes: g.themes.iter().map(|n| n.name.clone()).collect(),
            perspectives: g.player_perspectives.iter().map(|n| n.name.clone()).collect(),
            franchise: g
                .franchise
                .as_ref()
                .map(|f| f.name.clone())
                .filter(|s| !s.trim().is_empty()),
            artworks: g
                .artworks
                .iter()
                .map(|a| img_url("t_1080p", &a.image_id))
                .collect(),
            videos: g
                .videos
                .iter()
                .filter(|v| !v.video_id.is_empty())
                .map(|v| v.video_id.clone())
                .collect(),
            similar: g
                .similar_games
                .iter()
                .filter(|s| !s.name.is_empty())
                .take(10)
                .map(|s| SimilarGame {
                    name: s.name.clone(),
                    cover_url: s.cover.as_ref().map(|c| img_url("t_cover_big", &c.image_id)),
                })
                .collect(),
            time_to_beat,
            websites: g.websites.clone(),
        });
    }

    None
}

/// Query IGDB's `game_time_to_beats` endpoint for a game id (values in seconds).
/// Best-effort: any failure or empty result yields `None`.
fn fetch_time_to_beat(agent: &ureq::Agent, bearer: &str, game_id: u64) -> Option<TimeToBeat> {
    if game_id == 0 {
        return None;
    }
    #[derive(Deserialize)]
    struct Ttb {
        hastily: Option<u32>,
        normally: Option<u32>,
        completely: Option<u32>,
    }
    let body = format!("fields hastily,normally,completely; where game_id = {game_id}; limit 1;");
    let arr: Vec<Ttb> = agent
        .post("https://api.igdb.com/v4/game_time_to_beats")
        .set("Client-ID", CLIENT_ID)
        .set("Authorization", bearer)
        .set("Accept", "application/json")
        .send_string(&body)
        .ok()
        .and_then(|r| r.into_json().ok())?;
    let t = arr.into_iter().next()?;
    if t.hastily.is_none() && t.normally.is_none() && t.completely.is_none() {
        return None;
    }
    Some(TimeToBeat {
        hastily: t.hastily,
        normally: t.normally,
        completely: t.completely,
    })
}
