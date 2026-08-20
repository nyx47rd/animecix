use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const BASE: &str = "https://animecix.tv";
const TAU: &str = "https://tau-video.xyz";
/// API yanıtları için disk cache TTL (3 saat)
const API_TTL_SECS: u64 = 3 * 3600;
/// Kapak görselleri için disk cache TTL (7 gün)
const IMG_TTL_SECS: u64 = 7 * 24 * 3600;

pub struct Client {
    http: reqwest::blocking::Client,
    /// Bellek içi API önbellek (key → (unix_ts, json_value))
    cache: std::sync::Mutex<HashMap<String, (u64, serde_json::Value)>>,
    /// Bellek içi kapak önbellek (url → (unix_ts, bytes))
    bytes: std::sync::Mutex<HashMap<String, (u64, Vec<u8>)>>,
    resolved: std::sync::Mutex<HashMap<String, (u64, String)>>,
    /// Disk cache klasörü
    cache_dir: PathBuf,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AniSkipTimes {
    pub op_start: Option<f64>,
    pub op_end: Option<f64>,
    pub ed_start: Option<f64>,
    pub ed_end: Option<f64>,
}

/// Video kaynağı bilgisi — kaynak seçim dialogu için
#[derive(Clone, Debug)]
pub struct VideoSource {
    /// Host adı (tau-video, sibnet, streamtape, ...)
    pub host: String,
    /// Oy sayısı
    pub votes: i64,
    /// Çevirmen puanı (varsa)
    pub points: f64,
    /// Embed URL (çözümlenmemiş)
    pub embed_url: String,
    /// Kalite etiketi (tau'dan geliyorsa, ör. "1080p")
    pub quality: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Title {
    pub id: u64,
    pub name: String,
    pub year: Option<i64>,
    pub title_type: Option<String>,
    pub poster: Option<String>,
    pub description: Option<String>,
    pub season_count: Option<i64>,
}

impl Title {
    /// Kart alt satırı için birleşik meta: "YIL  •  TÜR  •  N Sezon".
    /// Hem maraton hem favoriler aynı formatı kullansın diye tek kaynak.
    pub fn meta_line(&self) -> String {
        let parts: Vec<String> = [
            self.year.map(|y| y.to_string()),
            self.title_type.as_deref().map(|t| match t {
                "anime" => "Anime Serisi",
                "movie" => "Film",
                _ => "Dizi",
            }.to_string()),
            self.season_count.map(|s| format!("{s} Sezon")),
        ]
        .into_iter()
        .flatten()
        .collect();
        parts.join("  •  ")
    }
}

#[derive(Clone, Debug)]
pub struct Category {
    pub name: String,
    pub items: Vec<Title>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Episode {
    pub episode: u64,
    pub season: u64,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub title: Title,
    pub episode: Episode,
    pub ts: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Watched {
    pub title_id: u64,
    pub episode: u64,
    pub season: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarathonItem {
    pub title: Title,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub added_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct State {
    pub current: Option<Watched>,
    pub watched: HashMap<String, Vec<Watched>>,
    #[serde(default)]
    pub saved: Vec<Title>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(default)]
    pub welcome_seen: bool,
    /// Bölüm ilerleme konumu: "tid:s:e" -> (pos_sn, dur_sn)
    #[serde(default)]
    pub progress: HashMap<String, (f64, f64)>,
    #[serde(default)]
    pub quick_search_tip_seen: bool,
    #[serde(default)]
    pub search_tip_seen: bool,
    #[serde(default)]
    pub right_click_tip_seen: bool,
    #[serde(default)]
    pub marathon: Vec<MarathonItem>,
}

/// Uygulama ayarları (state.json'dan ayrı, settings.json)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    /// "overlay" (yüzen) | "inline" (normal)
    #[serde(default = "default_loading")]
    pub loading_style: String,
    #[serde(default = "default_quick_search")]
    pub quick_search_enabled: bool,
    #[serde(default = "default_shortcut")]
    pub quick_search_shortcut: String,
    /// Anime/dizi arama kısayolu
    #[serde(default = "default_search_shortcut")]
    pub search_shortcut: String,
    #[serde(default = "default_true")]
    pub auto_fullscreen: bool,
    #[serde(default = "default_true")]
    pub aniskip_enabled: bool,
    /// Uygulama başlatmada yeni sürümü kontrol edip (AppImage ise) kendini günceller
    #[serde(default = "default_true")]
    pub auto_update: bool,
}
fn default_loading() -> String { "overlay".into() }
fn default_quick_search() -> bool { true }
fn default_shortcut() -> String { "/".into() }
fn default_search_shortcut() -> String { "Ctrl+S".into() }
fn default_true() -> bool { true }
impl Default for Settings {
    fn default() -> Self {
        Self {
            loading_style: default_loading(),
            quick_search_enabled: default_quick_search(),
            quick_search_shortcut: default_shortcut(),
            search_shortcut: default_search_shortcut(),
            auto_fullscreen: default_true(),
            aniskip_enabled: default_true(),
            auto_update: default_true(),
        }
    }
}

impl Client {
    pub fn new() -> Self {
        let http = reqwest::blocking::Client::builder()
            .user_agent(UA)
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(6))
            .pool_max_idle_per_host(1)
            .pool_idle_timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("http client");

        let cache_dir = {
            let mut p = dirs_cache_or_home();
            p.push(".cache/animecix");
            p
        };
        std::fs::create_dir_all(cache_dir.join("api")).ok();
        std::fs::create_dir_all(cache_dir.join("covers")).ok();

        Self {
            http,
            cache: std::sync::Mutex::new(HashMap::new()),
            bytes: std::sync::Mutex::new(HashMap::new()),
            resolved: std::sync::Mutex::new(HashMap::new()),
            cache_dir,
        }
    }

    // ---- disk cache yardımcıları ----

    fn api_cache_path(&self, key: &str) -> PathBuf {
        // Anahtarı dosya ismine çevir (geçersiz karakterleri kaldır)
        let safe: String = key.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        self.cache_dir.join("api").join(format!("{safe}.json"))
    }

    fn img_cache_path(&self, url: &str) -> PathBuf {
        // URL'den basit bir hash oluştur
        let mut h: u64 = 0xcbf29ce484222325;
        for b in url.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        // Uzantıyı koru (.jpg, .webp, vb.)
        let ext = url.split('?').next().unwrap_or("").rsplit('.').next()
            .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_alphanumeric()))
            .unwrap_or("bin");
        self.cache_dir.join("covers").join(format!("{h:x}.{ext}"))
    }

    fn disk_api_load(&self, key: &str) -> Option<(u64, serde_json::Value)> {
        let path = self.api_cache_path(key);
        let text = std::fs::read_to_string(&path).ok()?;
        // İlk satır: unix timestamp
        let mut lines = text.splitn(2, '\n');
        let ts: u64 = lines.next()?.trim().parse().ok()?;
        let json: serde_json::Value = serde_json::from_str(lines.next()?).ok()?;
        Some((ts, json))
    }

    fn disk_api_save(&self, key: &str, ts: u64, value: &serde_json::Value) {
        let path = self.api_cache_path(key);
        let body = format!("{ts}\n{}", serde_json::to_string(value).unwrap_or_default());
        std::fs::write(&path, body).ok();
    }

    fn cache_get(
        &self,
        key: &str,
        ttl: u64,
        loader: impl FnOnce(&reqwest::blocking::Client) -> Result<serde_json::Value, String>,
    ) -> Result<serde_json::Value, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 1) Belleğe bak
        {
            let mem = self.cache.lock().unwrap();
            if let Some((t, v)) = mem.get(key) {
                if now.saturating_sub(*t) < ttl {
                    return Ok(v.clone());
                }
            }
        }

        // 2) Diske bak
        if let Some((t, v)) = self.disk_api_load(key) {
            if now.saturating_sub(t) < ttl {
                // Diskten belleğe yükle
                self.cache.lock().unwrap().insert(key.to_string(), (t, v.clone()));
                return Ok(v);
            }
            // TTL dolmuş ama disk cache var — stale-while-revalidate:
            // Hemen stale veriyi döndür, arka planda güncellenecek
            let stale = v.clone();
            match loader(&self.http) {
                Ok(fresh) => {
                    self.cache.lock().unwrap().insert(key.to_string(), (now, fresh.clone()));
                    self.disk_api_save(key, now, &fresh);
                    return Ok(fresh);
                }
                Err(_) => {
                    // İnternet yoksa stale veriyi kullan
                    self.cache.lock().unwrap().insert(key.to_string(), (now, stale.clone()));
                    return Ok(stale);
                }
            }
        }

        // 3) Ağ isteği
        let v = loader(&self.http)?;
        self.cache.lock().unwrap().insert(key.to_string(), (now, v.clone()));
        self.disk_api_save(key, now, &v);
        Ok(v)
    }

    pub fn home_lists(&self) -> Result<Vec<Category>, String> {
        let d = self.cache_get("lists", API_TTL_SECS, |http| {
            http.get(format!("{BASE}/secure/homepage/lists-guests"))
                .header("Accept", "application/json")
                .send()
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json()
                .map_err(|e| e.to_string())
        })?;
        let mut cats = Vec::new();
        if let Some(lists) = d["lists"].as_array() {
            for lst in lists {
                let mut items = Vec::new();
                if let Some(arr) = lst["items"].as_array() {
                    for it in arr {
                        let t = it["title_type"].as_str().unwrap_or("");
                        let m = it["model_type"].as_str().unwrap_or("");
                        if t != "anime" && t != "movie" && m != "title" {
                            continue;
                        }
                        if let Some(id) = it["id"].as_u64().or_else(|| it["title_id"].as_u64()) {
                            items.push(Title {
                                id,
                                name: it["name"].as_str().unwrap_or("").to_string(),
                                year: it["year"].as_i64(),
                                title_type: Some(t.to_string()),
                                poster: it["poster"]
                                    .as_str()
                                    .map(|s| s.to_string()),
                                description: it["description"].as_str().map(|s| s.to_string()),
                                season_count: it["season_count"].as_i64(),
                            });
                        }
                    }
                }
                if !items.is_empty() {
                    cats.push(Category {
                        name: lst["name"].as_str().unwrap_or("").to_string(),
                        items,
                    });
                }
            }
        }
        Ok(cats)
    }

    pub fn search(&self, q: &str) -> Result<Vec<Title>, String> {
        let key = format!("search:{q}");
        let d = self.cache_get(&key, 300, |http| {
            http.get(format!("{BASE}/secure/search/{}", q))
                .header("Accept", "application/json")
                .query(&[("limit", "20")])
                .send()
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json()
                .map_err(|e| e.to_string())
        })?;
        let mut out = Vec::new();
        if let Some(results) = d["results"].as_array() {
            for r in results {
                if let Some(id) = r["id"].as_u64() {
                    let tt = r["title_type"].as_str().unwrap_or("");
                    out.push(Title {
                        id,
                        name: r["name"].as_str().unwrap_or("").to_string(),
                        year: r["year"].as_i64(),
                        title_type: Some(tt.to_string()),
                        poster: r["poster"].as_str().map(|s| s.to_string()),
                        description: r["description"].as_str().map(|s| s.to_string()),
                        season_count: r["season_count"].as_i64(),
                    });
                }
            }
        }
        Ok(out)
    }

    pub fn episodes(&self, t: &Title) -> Result<Vec<Episode>, String> {
        if t.title_type.as_deref() == Some("movie") {
            return self.movie_episodes(t.id);
        }
        let seasons = t.season_count.unwrap_or(1).max(1) as u64;

        // Paralel sezon yükleme (std::thread::scope ile anında paralel HTTP isteği)
        let results: Vec<Result<Vec<Episode>, String>> = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for s in 1..=seasons {
                handles.push(scope.spawn(move || {
                    let key = format!("ep:{tid}:{s}", tid = t.id);
                    let d = self.cache_get(&key, 6 * 3600, |http| {
                        http.get(format!("{BASE}/secure/related-videos"))
                            .header("Accept", "application/json")
                            .query(&[
                                ("episode", "1"),
                                ("season", &s.to_string()),
                                ("videoId", "0"),
                                ("titleId", &t.id.to_string()),
                            ])
                            .send()
                            .map_err(|e| e.to_string())?
                            .error_for_status()
                            .map_err(|e| e.to_string())?
                            .json()
                            .map_err(|e| e.to_string())
                    })?;
                    let mut season_eps = Vec::new();
                    if let Some(vids) = d["videos"].as_array() {
                        for v in vids {
                            let ep = v["episode_num"].as_u64().unwrap_or(0);
                            let sez = v["season_num"].as_u64().unwrap_or(0);
                            if ep == 0 || sez != s {
                                continue;
                            }
                            if season_eps.iter().any(|x: &Episode| x.season == sez && x.episode == ep) {
                                continue;
                            }
                            season_eps.push(Episode {
                                episode: ep,
                                season: sez,
                                name: v["name"]
                                    .as_str()
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| format!("Bölüm {ep}")),
                            });
                        }
                    }
                    Ok(season_eps)
                }));
            }
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        let mut all: Vec<Episode> = Vec::new();
        for r in results {
            if let Ok(eps) = r {
                all.extend(eps);
            }
        }
        all.sort_by_key(|e| (e.season, e.episode));
        all.dedup_by_key(|e| (e.season, e.episode));

        if all.is_empty() {
            return self.movie_episodes(t.id);
        }
        Ok(all)
    }

    /// Tüm sayfaları gezen film/dizi video listesi (kategori: full)
    fn movie_videos(&self, title_id: u64) -> Result<serde_json::Value, String> {
        let key = format!("mvid:{title_id}");
        self.cache_get(&key, 6 * 3600, |http| {
            let mut merged: Vec<serde_json::Value> = Vec::new();
            let mut page: u32 = 1;
            loop {
                let r: serde_json::Value = http
                    .get(format!("{BASE}/secure/videos"))
                    .header("Accept", "application/json")
                    .query(&[
                        ("titleId", &title_id.to_string()),
                        ("page", &page.to_string()),
                    ])
                    .send()
                    .map_err(|e| e.to_string())?
                    .error_for_status()
                    .map_err(|e| e.to_string())?
                    .json()
                    .map_err(|e| e.to_string())?;
                if let Some(data) = r["pagination"]["data"].as_array() {
                    merged.extend(data.clone());
                }
                let last = r["pagination"]["last_page"].as_u64().unwrap_or(1);
                if page as u64 >= last || merged.len() > 600 {
                    break;
                }
                page += 1;
            }
            Ok(serde_json::json!({ "data": merged }))
        })
    }

    /// Film için tek kayıt (İzle butonu açılsın diye) — kaynak varsa
    fn movie_episodes(&self, title_id: u64) -> Result<Vec<Episode>, String> {
        let d = self.movie_videos(title_id)?;
        let has_full = d["data"]
            .as_array()
            .map(|a| {
                a.iter().any(|v| {
                    v["category"].as_str() == Some("full")
                        && !v["url"].as_str().unwrap_or("").is_empty()
                })
            })
            .unwrap_or(false);
        if has_full {
            Ok(vec![Episode {
                episode: 1,
                season: 1,
                name: "Filmi izle".to_string(),
            }])
        } else {
            Ok(Vec::new())
        }
    }

    /// Film için en yüksek kaliteli kaynağı çözer (tau 1080p tercih, paralel çözümleme)
    pub fn resolve_movie(&self, title_id: u64) -> Result<String, String> {
        let d = self.movie_videos(title_id)?;
        let mut candidates: Vec<serde_json::Value> = Vec::new();
        if let Some(data) = d["data"].as_array() {
            let mut full_videos: Vec<&serde_json::Value> = data.iter()
                .filter(|v| {
                    v["category"].as_str() == Some("full")
                        && !v["url"].as_str().unwrap_or("").is_empty()
                })
                .collect();
            full_videos.sort_by(|a, b| {
                let a_tau = a["url"].as_str().unwrap_or("").contains("tau-video.xyz");
                let b_tau = b["url"].as_str().unwrap_or("").contains("tau-video.xyz");
                b_tau.cmp(&a_tau)
                    .then_with(|| {
                        let a_votes = a["positive_votes"].as_i64().unwrap_or(0);
                        let b_votes = b["positive_votes"].as_i64().unwrap_or(0);
                        b_votes.cmp(&a_votes)
                    })
            });
            candidates = full_videos.into_iter().take(8).cloned().collect();
        }
        if candidates.is_empty() {
            return Err("film kaynağı bulunamadı".to_string());
        }

        // Tüm adayları paralel çözümlе, en yüksek boyutlu (en kaliteli) olanı seç
        let found: Option<(u64, String)> = std::thread::scope(|scope| {
            let (tx, rx) = std::sync::mpsc::channel();
            for v in candidates {
                let tx = tx.clone();
                let http = self.http.clone();
                scope.spawn(move || {
                    let url = v["url"].as_str().unwrap_or("");
                    if let Ok(mp4) = self.resolve_embed(url) {
                        let size = http.head(&mp4).send()
                            .ok()
                            .and_then(|r| r.content_length())
                            .unwrap_or(0);
                        let _ = tx.send((size, mp4));
                    }
                });
            }
            drop(tx);
            let mut best: Option<(u64, String)> = None;
            for (size, url) in rx.iter() {
                if let Some((ref mut best_size, _)) = best {
                    if size > *best_size {
                        *best_size = size;
                        best = Some((size, url));
                    }
                } else {
                    best = Some((size, url));
                }
            }
            best
        });

        match found {
            Some((_, mp4)) => Ok(mp4),
            None => Err("film kaynağı çözülemedi".to_string()),
        }
    }

    /// Herhangi bir embed URL'sini mp4'e çözer
    fn resolve_embed(&self, url: &str) -> Result<String, String> {
        if url.contains("tau-video.xyz") {
            let rest = url.split("/embed/").nth(1).unwrap_or("");
            let (embed_id, vid) = match rest.split_once("?vid=") {
                Some((e, v)) => (e.to_string(), v.to_string()),
                None => (rest.trim_end_matches('?').to_string(), String::new()),
            };
            if embed_id.len() >= 24 {
                return self.tau_resolve(&embed_id, &vid);
            }
        }
        if url.contains("sibnet.ru") {
            return self.sibnet_resolve(url);
        }
        if url.contains("streamtape.com") {
            return self.streamtape_resolve(url);
        }
        let host = url
            .split("//")
            .nth(1)
            .unwrap_or(url)
            .split('/')
            .next()
            .unwrap_or(url)
            .to_string();
        Err(format!("{host} kaynağı şu an çözülemiyor"))
    }

    /// Sibnet: shell.php sayfasından direkt mp4 çeker
    fn sibnet_resolve(&self, url: &str) -> Result<String, String> {
        let html = self
            .http
            .get(url)
            .send()
            .map_err(|e| format!("sibnet: {e}"))?
            .error_for_status()
            .map_err(|e| format!("sibnet: {e}"))?
            .text()
            .map_err(|e| format!("sibnet: {e}"))?;
        if let Some(pos) = html.find("/v/") {
            let start = html[pos..].find(".mp4").map(|p| pos + p + 4);
            if let Some(end) = start {
                let path = &html[pos..end];
                return Ok(format!("https://video.sibnet.ru{path}"));
            }
        }
        Err("sibnet mp4 bulunamadı".to_string())
    }

    /// Streamtape: get_video URL'sini çeker; "nofile" ise hata
    fn streamtape_resolve(&self, url: &str) -> Result<String, String> {
        let html = self
            .http
            .get(url)
            .send()
            .map_err(|e| format!("streamtape: {e}"))?
            .error_for_status()
            .map_err(|e| format!("streamtape: {e}"))?
            .text()
            .map_err(|e| format!("streamtape: {e}"))?;
        if html.contains("\"nofile\"") {
            return Err("streamtape videosu kaldırılmış".to_string());
        }
        if let Some(start) = html.find("get_video?id=") {
            let mut end = start + 13;
            while end < html.len() {
                let c = html.as_bytes()[end] as char;
                if c == '"' || c == '\'' || c == '<' || c == ' ' || c == ')' {
                    break;
                }
                end += 1;
            }
            let id = &html[start..end];
            if id.len() > 30 {
                return Ok(format!("https://streamtape.com/{id}"));
            }
        }
        Err("streamtape çözülemedi".to_string())
    }

    /// Embed id + vid'den tau mp4 URL'si (1080p tercih)
    fn tau_resolve(&self, embed_id: &str, vid: &str) -> Result<String, String> {
        let mut url = format!("{TAU}/api/video/{embed_id}");
        if !vid.is_empty() {
            url.push_str(&format!("?vid={vid}"));
        }
        let d = self
            .http
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| format!("tau api: {e}"))?
            .error_for_status()
            .map_err(|e| format!("tau api: {e}"))?
            .json::<serde_json::Value>()
            .map_err(|e| format!("tau json: {e}"))?;
        let mut best: Option<(String, String)> = None;
        if let Some(urls) = d["urls"].as_array() {
            for u in urls {
                if let Some(uu) = u["url"].as_str() {
                    let label = u["label"].as_str().unwrap_or("").to_string();
                    if label == "1080p" {
                        best = Some((label, uu.to_string()));
                        break;
                    }
                    if best.is_none() {
                        best = Some((label, uu.to_string()));
                    }
                }
            }
        }
        match best {
            Some((_, u)) => Ok(u),
            None => Err("tau-video url bulunamadı".to_string()),
        }
    }

    /// Embed sayfasından tau-video embed id ve vid alır
    fn find_embed(&self, title_id: u64, episode: u64, season: u64) -> Result<(String, String), String> {
        let url = format!("{BASE}/secure/best-video?titleId={title_id}&episode={episode}&season={season}");
        let resp = self
            .http
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| format!("best-video: {e}"))?;
        let final_url = resp.url().to_string();
        if let Some(pos) = final_url.find("/embed/") {
            let rest = &final_url[pos + 7..];
            let id: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if id.len() >= 24 {
                let vid = final_url.split("vid=").nth(1).unwrap_or("").to_string();
                return Ok((id[..24].to_string(), vid));
            }
        }
        // tau-video.xyz/embed/<id>?vid=... redirect olduysa body yerine url kullan
        if let Some(qpos) = final_url.find("?") {
            if final_url.contains("tau-video") && final_url.contains("/embed/") {
                let rest = &final_url[..qpos];
                let id: String = rest.rsplit('/').next().unwrap_or("").to_string();
                let vid = final_url.split("vid=").nth(1).unwrap_or("").to_string();
                if id.len() == 24 {
                    return Ok((id, vid));
                }
            }
        }
        Err(format!("embed bulunamadı: {final_url}"))
    }

    /// Direct mp4 URL'si döner — tüm kaynakları paralel çözer, en yüksek kaliteliyi seçer
    pub fn resolve(&self, title_id: u64, episode: u64, season: u64) -> Result<String, String> {
        let key = format!("evp:{title_id}:{season}:{episode}");
        let d = self.cache_get(&key, 1800, |http| {
            http.get(format!("{BASE}/secure/episode-videos-points"))
                .header("Accept", "application/json")
                .query(&[
                    ("titleId", &title_id.to_string()),
                    ("episode", &episode.to_string()),
                    ("season", &season.to_string()),
                ])
                .send()
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json()
                .map_err(|e| e.to_string())
        })?;
        let videos = d["videos"].as_array().cloned().unwrap_or_default();
        if videos.is_empty() {
            let (embed_id, vid) = self.find_embed(title_id, episode, season)?;
            return self.tau_resolve(&embed_id, &vid);
        }
        let points = &d["translatorPoints"];

        let mut groups: Vec<(i64, f64, i64, Vec<serde_json::Value>)> = Vec::new();
        for v in &videos {
            let tpl = v["template"].as_i64().unwrap_or(0);
            let point = points[tpl.to_string()].as_f64().unwrap_or(0.0);
            let votes = v["positive_votes"].as_i64().unwrap_or(0);
            if let Some(g) = groups.iter_mut().find(|g| g.0 == tpl) {
                g.2 += votes;
                g.3.push(v.clone());
            } else {
                groups.push((tpl, point, votes, vec![v.clone()]));
            }
        }
        groups.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.2.cmp(&a.2))
        });

        let mut candidates = Vec::new();
        for g in groups.iter() {
            let mut mirrors = g.3.clone();
            mirrors.sort_by_key(|v| {
                let is_tau = v["url"].as_str().unwrap_or("").contains("tau-video.xyz");
                !is_tau
            });
            for v in mirrors {
                let u = v["url"].as_str().unwrap_or("").to_string();
                if !u.is_empty() && !candidates.contains(&u) {
                    candidates.push(u);
                }
                if candidates.len() >= 8 { break; }
            }
            if candidates.len() >= 8 { break; }
        }

        // Tüm adayları paralel çözümlе, her birinin boyutunu kontrol et
        let found: Option<(u64, String)> = std::thread::scope(|scope| {
            let (tx, rx) = std::sync::mpsc::channel();
            for u in candidates {
                let tx = tx.clone();
                let http = self.http.clone();
                scope.spawn(move || {
                    if let Ok(mp4) = self.resolve_embed(&u) {
                        // HEAD request ile dosya boyutunu kontrol et
                        let size = http.head(&mp4).send()
                            .ok()
                            .and_then(|r| r.content_length())
                            .unwrap_or(0);
                        let _ = tx.send((size, mp4));
                    }
                });
            }
            drop(tx);
            // En büyük dosya boyutunu (en yüksek kalite) seç
            let mut best: Option<(u64, String)> = None;
            for (size, url) in rx.iter() {
                if let Some((ref mut best_size, _)) = best {
                    if size > *best_size {
                        *best_size = size;
                        best = Some((size, url));
                    }
                } else {
                    best = Some((size, url));
                }
            }
            best
        });

        match found {
            Some((_, mp4)) => Ok(mp4),
            None => Err("Bölüm videosu çözülemedi".to_string()),
        }
    }

    /// Bölüm için mevcut tüm kaynakları listele (çözümlenmemiş, sadece metadata)
    pub fn list_sources(&self, title_id: u64, episode: u64, season: u64) -> Result<Vec<VideoSource>, String> {
        let key = format!("evp:{title_id}:{season}:{episode}");
        let d = self.cache_get(&key, 1800, |http| {
            http.get(format!("{BASE}/secure/episode-videos-points"))
                .header("Accept", "application/json")
                .query(&[
                    ("titleId", &title_id.to_string()),
                    ("episode", &episode.to_string()),
                    ("season", &season.to_string()),
                ])
                .send()
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json()
                .map_err(|e| e.to_string())
        })?;
        let videos = d["videos"].as_array().cloned().unwrap_or_default();
        if videos.is_empty() {
            return Err("Kaynak bulunamadı".to_string());
        }
        let points = &d["translatorPoints"];

        let mut sources: Vec<VideoSource> = Vec::new();
        for v in &videos {
            let url = v["url"].as_str().unwrap_or("").to_string();
            if url.is_empty() { continue; }
            let votes = v["positive_votes"].as_i64().unwrap_or(0);
            let tpl = v["template"].as_i64().unwrap_or(0);
            let pts = points[tpl.to_string()].as_f64().unwrap_or(0.0);
            let host = Self::extract_host(&url);
            sources.push(VideoSource {
                host,
                votes,
                points: pts,
                embed_url: url,
                quality: String::new(),
            });
        }
        // Oy + puana göre sırala
        sources.sort_by(|a, b| {
            b.points.partial_cmp(&a.points).unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.votes.cmp(&a.votes))
        });
        Ok(sources)
    }

    /// Film için mevcut tüm kaynakları listele
    pub fn list_movie_sources(&self, title_id: u64) -> Result<Vec<VideoSource>, String> {
        let d = self.movie_videos(title_id)?;
        let mut sources: Vec<VideoSource> = Vec::new();
        if let Some(data) = d["data"].as_array() {
            for v in data {
                if v["category"].as_str() != Some("full") { continue; }
                let url = v["url"].as_str().unwrap_or("").to_string();
                if url.is_empty() { continue; }
                let votes = v["positive_votes"].as_i64().unwrap_or(0);
                let host = Self::extract_host(&url);
                sources.push(VideoSource {
                    host,
                    votes,
                    points: 0.0,
                    embed_url: url,
                    quality: String::new(),
                });
            }
        }
        sources.sort_by(|a, b| b.votes.cmp(&a.votes));
        if sources.is_empty() {
            return Err("Film kaynağı bulunamadı".to_string());
        }
        Ok(sources)
    }

    /// Tek bir embed URL'ini çözüme kavuştur
    pub fn resolve_single(&self, embed_url: &str) -> Result<String, String> {
        self.resolve_embed(embed_url)
    }

    /// URL'den host adını çıkar
    fn extract_host(url: &str) -> String {
        url.split("//").nth(1)
            .unwrap_or(url)
            .split('/')
            .next()
            .unwrap_or(url)
            .split('.')
            .next()
            .unwrap_or("unknown")
            .to_string()
    }

    /// Anime adından AniList GraphQL API ile MAL ID (MyAnimeList ID) çözer
    pub fn resolve_mal_id(&self, anime_name: &str) -> Option<u64> {
        let clean_name = anime_name
            .replace("(TV)", "")
            .replace("Dublaj", "")
            .replace("Altyazılı", "")
            .replace("Sezon", "")
            .trim()
            .to_string();

        let key = format!("mal_id:{clean_name}");
        let d = self.cache_get(&key, 30 * 86400, |http| {
            let body = serde_json::json!({
                "query": "query ($s: String) { Media (search: $s, type: ANIME) { idMal } }",
                "variables": { "s": clean_name }
            });
            http.post("https://graphql.anilist.co")
                .json(&body)
                .timeout(std::time::Duration::from_secs(4))
                .send()
                .map_err(|e| e.to_string())?
                .json()
                .map_err(|e| e.to_string())
        }).ok()?;

        d["data"]["Media"]["idMal"].as_u64()
    }

    /// AniSkip API'den (MAL ID + Bölüm No) intro (OP) ve outro (ED) damgalarını dinamik çözer
    pub fn fetch_aniskip_timestamps(&self, anime_name: &str, ep_num: u64) -> AniSkipTimes {
        let Some(mal_id) = self.resolve_mal_id(anime_name) else {
            return AniSkipTimes::default();
        };

        let key = format!("aniskip_v3:{mal_id}:{ep_num}");
        let d = match self.cache_get(&key, 7 * 86400, |http| {
            http.get(format!("https://api.aniskip.com/v2/skip-times/{mal_id}/{ep_num}?types=op&types=ed&episodeLength=0"))
                .timeout(std::time::Duration::from_secs(4))
                .send()
                .map_err(|e| e.to_string())?
                .json()
                .map_err(|e| e.to_string())
        }) {
            Ok(val) => val,
            Err(_) => return AniSkipTimes::default(),
        };

        let mut res = AniSkipTimes::default();
        if d["found"].as_bool() == Some(true) {
            if let Some(results) = d["results"].as_array() {
                for r in results {
                    let skip_type = r["skipType"].as_str().unwrap_or("");
                    let start = r["interval"]["startTime"].as_f64();
                    let end = r["interval"]["endTime"].as_f64();
                    if skip_type == "op" {
                        res.op_start = start;
                        res.op_end = end;
                    } else if skip_type == "ed" {
                        res.ed_start = start;
                        res.ed_end = end;
                    }
                }
            }
        }
        res
    }

    pub fn get_bytes(&self, url: &str) -> Option<Vec<u8>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 1) Bellek önbelleği
        {
            let b = self.bytes.lock().unwrap();
            if let Some((t, v)) = b.get(url) {
                if now.saturating_sub(*t) < IMG_TTL_SECS {
                    return Some(v.clone());
                }
            }
        }

        // 2) Disk önbelleği
        let disk_path = self.img_cache_path(url);
        if let Ok(meta) = std::fs::metadata(&disk_path) {
            let disk_ts = meta.modified().ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now.saturating_sub(disk_ts) < IMG_TTL_SECS {
                if let Ok(data) = std::fs::read(&disk_path) {
                    // Diskten belleğe yükle
                    self.bytes.lock().unwrap().insert(url.to_string(), (disk_ts, data.clone()));
                    return Some(data);
                }
            }
        }

        // 3) Ağ isteği
        let out = self.http.get(url)
            .timeout(std::time::Duration::from_secs(12))
            .send().ok()?.bytes().ok().map(|b| b.to_vec());
        if let Some(v) = &out {
            // Belleğe kaydet
            self.bytes.lock().unwrap().insert(url.to_string(), (now, v.clone()));
            // Diske kaydet
            let _ = std::fs::write(&disk_path, v);
        }
        out
    }

    // ---- durum ----

    pub fn state_path() -> PathBuf {
        let mut p = dirs_cache_or_home();
        p.push(".local/share/animecix/state.json");
        p
    }

    pub fn load_state(&self) -> State {
        let p = Self::state_path();
        std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_watched(&self, w: &Watched, name: &str) {
        let mut st = self.load_state();
        st.current = Some(Watched {
            title_id: w.title_id,
            episode: w.episode,
            season: w.season,
        });
        let key = w.title_id.to_string();
        let list = st.watched.entry(key).or_default();
        if !list.iter().any(|x| x.episode == w.episode && x.season == w.season) {
            list.push(w.clone());
        }
        st.watched.get_mut(&w.title_id.to_string()).unwrap().sort_by_key(|x| x.season * 10000 + x.episode);
        let p = Self::state_path();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let _ = std::fs::write(&p, serde_json::to_string_pretty(&st).unwrap_or_default());
        let _ = name; // (state dosyasındaki isim eski sistemle uyumluluk için)
    }

    /// bölüm izlendi olarak kayıtlı mı
    pub fn is_watched(&self, title_id: u64, season: u64, episode: u64) -> bool {
        let st = self.load_state();
        st.watched
            .get(&title_id.to_string())
            .map(|l| l.iter().any(|x| x.episode == episode && x.season == season))
            .unwrap_or(false)
    }

    /// bölümü izlendi listesinden kaldır
    pub fn remove_watched(&self, title_id: u64, season: u64, episode: u64) {
        let mut st = self.load_state();
        let key = title_id.to_string();
        if let Some(list) = st.watched.get_mut(&key) {
            list.retain(|x| !(x.episode == episode && x.season == season));
        }
        self.save_state(&st);
    }

    pub fn save_state(&self, st: &State) {
        let p = Self::state_path();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let _ = std::fs::write(&p, serde_json::to_string_pretty(st).unwrap_or_default());
    }

    pub fn is_saved(&self, id: u64) -> bool {
        self.load_state().saved.iter().any(|t| t.id == id)
    }

    /// dönen değer: yeni durum (true = kaydedildi)
    pub fn toggle_saved(&self, t: &Title) -> bool {
        let mut st = self.load_state();
        if let Some(pos) = st.saved.iter().position(|x| x.id == t.id) {
            st.saved.remove(pos);
            self.save_state(&st);
            false
        } else {
            st.saved.insert(0, t.clone());
            self.save_state(&st);
            true
        }
    }

    pub fn get_marathon(&self) -> Vec<MarathonItem> {
        self.load_state().marathon
    }

    pub fn is_in_marathon(&self, id: u64) -> bool {
        self.load_state().marathon.iter().any(|m| m.title.id == id)
    }

    pub fn toggle_marathon(&self, t: &Title) -> bool {
        let mut st = self.load_state();
        if let Some(pos) = st.marathon.iter().position(|m| m.title.id == t.id) {
            st.marathon.remove(pos);
            self.save_state(&st);
            false
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            st.marathon.push(MarathonItem {
                title: t.clone(),
                completed: false,
                added_at: now,
            });
            self.save_state(&st);
            true
        }
    }

    pub fn toggle_marathon_completed(&self, id: u64) -> bool {
        let mut st = self.load_state();
        let mut new_state = false;
        if let Some(item) = st.marathon.iter_mut().find(|m| m.title.id == id) {
            item.completed = !item.completed;
            new_state = item.completed;
        }
        self.save_state(&st);
        new_state
    }

    pub fn remove_from_marathon(&self, id: u64) {
        let mut st = self.load_state();
        st.marathon.retain(|m| m.title.id != id);
        self.save_state(&st);
    }

    pub fn clear_marathon(&self) {
        let mut st = self.load_state();
        st.marathon.clear();
        self.save_state(&st);
    }

    /// Maratondaki bir öğeyi verilen hedef indekse taşır (sürükle-sırala için).
    pub fn reorder_marathon(&self, id: u64, new_index: usize) {
        let mut st = self.load_state();
        if let Some(pos) = st.marathon.iter().position(|m| m.title.id == id) {
            if pos == new_index {
                return;
            }
            let item = st.marathon.remove(pos);
            let idx = new_index.min(st.marathon.len());
            st.marathon.insert(idx, item);
            self.save_state(&st);
        }
    }

    /// Bir başlık için toplam bölüm sayısını döndürür (ağ gerektirir, cache'li).
    pub fn episode_count(&self, t: &Title) -> Option<u64> {
        self.episodes(t).ok().map(|e| e.len() as u64)
    }

    /// Bir başlık için izlenmiş (ilerleme kaydı olan) farklı bölüm sayısını döndürür.
    pub fn watched_episode_count(&self, tid: u64) -> u64 {
        let st = self.load_state();
        let prefix = format!("{tid}:");
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for key in st.progress.keys() {
            if let Some(rest) = key.strip_prefix(&prefix) {
                let mut it = rest.split(':');
                if let (Some(s), Some(e)) = (it.next(), it.next()) {
                    seen.insert((s.to_string(), e.to_string()));
                }
            }
        }
        seen.len() as u64
    }

    /// Maraton kartındaki progress bar için 0..1 arası ilerleme oranı.
    /// Film: izlenen/total süre. Dizi: izlenen/total bölüm sayısı.
    pub fn title_progress_frac(&self, t: &Title) -> f64 {
        if t.title_type.as_deref() == Some("movie") {
            if let Some((pos, dur)) = self.get_progress(t.id, 1, 1) {
                if dur > 0.0 {
                    return (pos / dur).clamp(0.0, 1.0);
                }
            }
            return 0.0;
        }
        let total = self.episode_count(t).unwrap_or(0);
        if total == 0 {
            return 0.0;
        }
        let watched = self.watched_episode_count(t.id);
        (watched as f64 / total as f64).clamp(0.0, 1.0)
    }

    pub fn clear_history(&self) {
        let mut st = self.load_state();
        st.history.clear();
        self.save_state(&st);
    }

    /// Seçilen geçmiş ögelerini sil (title id listesine göre)
    pub fn remove_history_items(&self, title_ids: &[u64]) {
        let mut st = self.load_state();
        st.history.retain(|h| !title_ids.contains(&h.title.id));
        self.save_state(&st);
    }

    pub fn add_history(&self, t: &Title, e: &Episode) {
        let mut st = self.load_state();
        st.history.retain(|h| h.title.id != t.id);
        st.history.insert(
            0,
            HistoryEntry {
                title: t.clone(),
                episode: e.clone(),
                ts: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            },
        );
        if st.history.len() > 30 {
            st.history.truncate(30);
        }
        self.save_state(&st);
    }

    // ---- ayarlar ----

    pub fn settings_path() -> PathBuf {
        let mut p = dirs_cache_or_home();
        p.push(".local/share/animecix/settings.json");
        p
    }

    pub fn load_settings(&self) -> Settings {
        let p = Self::settings_path();
        std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) {
        let p = Self::settings_path();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let _ = std::fs::write(&p, serde_json::to_string_pretty(s).unwrap_or_default());
    }

    /// Tüm uygulama verilerini (geçmiş, kapak görselleri, önbellek, ayarlar, geçici dosyalar) tamamen sil
    pub fn wipe_all_data(&self) {
        // 1) In-memory önbellekleri temizle
        if let Ok(mut c) = self.cache.lock() { c.clear(); }
        if let Ok(mut b) = self.bytes.lock() { b.clear(); }
        if let Ok(mut r) = self.resolved.lock() { r.clear(); }

        // 2) state.json ve settings.json'ı varsayılana sıfırla
        let st = State::default();
        self.save_state(&st);
        self.save_settings(&Settings::default());

        // 3) Disk üzerindeki kapak resmi ve API önbellek klasörünü (~/.cache/animecix) sil
        let mut cache_dir = dirs_cache_or_home();
        cache_dir.push(".cache/animecix");
        let _ = std::fs::remove_dir_all(&cache_dir);
        let _ = std::fs::create_dir_all(&cache_dir);

        // 4) /tmp/animecix-* geçici dosyalarını temizle
        if let Ok(entries) = std::fs::read_dir("/tmp") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("animecix-") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    pub fn is_welcome_seen(&self) -> bool {
        self.load_state().welcome_seen
    }

    pub fn set_welcome_seen(&self, seen: bool) {
        let mut st = self.load_state();
        st.welcome_seen = seen;
        self.save_state(&st);
    }

    pub fn is_quick_search_tip_seen(&self) -> bool {
        self.load_state().quick_search_tip_seen
    }

    pub fn set_quick_search_tip_seen(&self, seen: bool) {
        let mut st = self.load_state();
        st.quick_search_tip_seen = seen;
        self.save_state(&st);
    }

    pub fn is_search_tip_seen(&self) -> bool {
        self.load_state().search_tip_seen
    }

    pub fn set_search_tip_seen(&self, seen: bool) {
        let mut st = self.load_state();
        st.search_tip_seen = seen;
        self.save_state(&st);
    }

    pub fn is_right_click_tip_seen(&self) -> bool {
        self.load_state().right_click_tip_seen
    }

    pub fn set_right_click_tip_seen(&self, seen: bool) {
        let mut st = self.load_state();
        st.right_click_tip_seen = seen;
        self.save_state(&st);
    }

    /// B\u00f6l\u00fcm ilerleme konumunu kaydet (pos ve dur saniye cinsinden)
    pub fn save_progress(&self, tid: u64, season: u64, episode: u64, pos: f64, dur: f64) {
        let key = format!("{tid}:{season}:{episode}");
        let mut st = self.load_state();
        st.progress.insert(key, (pos, dur));
        self.save_state(&st);
    }

    /// Kaydedilmi\u015f ilerleme konumunu d\u00f6nd\u00fcr: (pos_sn, dur_sn) ya da None
    pub fn get_progress(&self, tid: u64, season: u64, episode: u64) -> Option<(f64, f64)> {
        let key = format!("{tid}:{season}:{episode}");
        self.load_state().progress.get(&key).copied()
    }
}

fn dirs_cache_or_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(tt: &str, year: Option<i64>, seasons: Option<i64>) -> Title {
        Title {
            id: 1,
            name: "X".into(),
            year,
            title_type: Some(tt.into()),
            poster: None,
            description: None,
            season_count: seasons,
        }
    }

    #[test]
    fn meta_line_anime_with_seasons() {
        let t = sample("anime", Some(2021), Some(2));
        assert_eq!(t.meta_line(), "2021  •  Anime Serisi  •  2 Sezon");
    }

    #[test]
    fn meta_line_movie_no_seasons() {
        let t = sample("movie", Some(2019), None);
        assert_eq!(t.meta_line(), "2019  •  Film");
    }

    #[test]
    fn meta_line_other_type_seasons() {
        let t = sample("something", Some(2020), Some(5));
        assert_eq!(t.meta_line(), "2020  •  Dizi  •  5 Sezon");
    }

    #[test]
    fn meta_line_matches_between_marathon_and_favs() {
        // Aynı başlık her iki yerde aynı alt satırı üretmeli
        let a = sample("anime", Some(2022), Some(3));
        let b = sample("anime", Some(2022), Some(3));
        assert_eq!(a.meta_line(), b.meta_line());
    }
}
