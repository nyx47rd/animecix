use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const BASE: &str = "https://animecix.tv";
const TAU: &str = "https://tau-video.xyz";
const API_TTL_SECS: u64 = 3 * 3600;
const IMG_TTL_SECS: u64 = 7 * 24 * 3600;

const MAX_IMG_CACHE_ENTRIES: usize = 150;

fn trim_img_cache(map: &mut HashMap<String, (u64, Vec<u8>)>) {
    if map.len() <= MAX_IMG_CACHE_ENTRIES {
        return;
    }
    let mut by_time: Vec<(u64, String)> =
        map.iter().map(|(k, (t, _))| (*t, k.clone())).collect();
    by_time.sort_unstable();
    let excess = map.len() - MAX_IMG_CACHE_ENTRIES;
    for (_, k) in by_time.into_iter().take(excess) {
        map.remove(&k);
    }
}
const CACHE_VERSION: &str = "2";

pub struct Client {
    http: crate::http::Http,
    cache: std::sync::Mutex<HashMap<String, (u64, serde_json::Value)>>,
    bytes: std::sync::Mutex<HashMap<String, (u64, Vec<u8>)>>,
    resolved: std::sync::Mutex<HashMap<String, (u64, String)>>,
    cache_dir: PathBuf,
    translators: std::sync::Mutex<HashMap<i64, TranslatorMeta>>,
}

#[derive(Clone, Debug)]
pub struct TranslatorMeta {
    pub id: i64,
    pub name: String,
    pub long_name: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AniSkipTimes {
    pub op_start: Option<f64>,
    pub op_end: Option<f64>,
    pub ed_start: Option<f64>,
    pub ed_end: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FansubInfo {
    pub template_id: i64,
    pub name: String,
    pub rating: f64,
    pub total_votes: i64,
    pub language: String,
    pub approved_only: bool,
    pub mirror_count: usize,
    pub hosts: Vec<String>,
    pub mirrors: Vec<FansubMirror>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FansubMirror {
    pub id: u64,
    pub host: String,
    pub url: String,
    pub quality: Option<String>,
    pub approved: bool,
}

#[derive(Clone, Debug)]
pub struct VideoSource {
    pub host: String,
    pub votes: i64,
    pub points: f64,
    pub embed_url: String,
    pub quality: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Title {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub year: Option<i64>,
    #[serde(default)]
    pub title_type: Option<String>,
    #[serde(default)]
    pub poster: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub season_count: Option<i64>,
    #[serde(default)]
    pub genres: Option<Vec<String>>,
    #[serde(default)]
    pub runtime: Option<i64>,
    #[serde(default)]
    pub episode_count: Option<i64>,
    #[serde(default)]
    pub release_date: Option<String>,
}

impl Title {
    pub fn from_value(r: &serde_json::Value) -> Option<Title> {
        let id = r["id"].as_u64().or_else(|| r["title_id"].as_u64())?;
        let tt = r["title_type"].as_str().unwrap_or("").to_string();
        let genres = r["genres"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|g| {
                    if g.is_string() {
                        g.as_str().map(|s| s.to_string())
                    } else {
                        g["display_name"]
                            .as_str()
                            .or_else(|| g["name"].as_str())
                            .map(|s| s.to_string())
                    }
                })
                .collect()
        });
        Some(Title {
            id,
            name: r["name"].as_str().unwrap_or("").to_string(),
            year: r["year"].as_i64(),
            title_type: if tt.is_empty() { None } else { Some(tt) },
            poster: r["poster"].as_str().map(|s| s.to_string()),
            description: r["description"].as_str().map(|s| s.to_string()),
            season_count: r["season_count"].as_i64(),
            genres,
            runtime: r["runtime"].as_i64(),
            episode_count: r["episode_count"].as_i64(),
            release_date: r["release_date"].as_str().map(|s| s.to_string()),
        })
    }

    pub fn display_name(&self) -> String {
        match self.year {
            Some(y) => format!("{} ({})", self.name, y),
            None => self.name.clone(),
        }
    }

    fn tr_genre(s: &str) -> String {
        let t = match s.trim().to_lowercase().as_str() {
            "drama" => "Dram",
            "animation" => "Animasyon",
            "animasyon" => "Animasyon",
            "comedy" => "Komedi",
            "documentary" => "Belgesel",
            "family" => "Aile",
            "kids" => "Çocuk",
            "news" => "Haber",
            "reality" => "Gerçeklik",
            "soap" => "Pembe Dizi",
            "talk" => "Talk Show",
            "war & politics" => "Savaş & Politika",
            "western" => "Western",
            "crime" => "Suç",
            "mystery" => "Gizem",
            "thriller" => "Gerilim",
            "horror" => "Korku",
            "romance" => "Romantik",
            "music" => "Müzik",
            "war" => "Savaş",
            "fantasy" => "Fantezi",
            "science fiction" => "Bilim Kurgu",
            "sci-fi & fantasy" => "Bilim Kurgu & Fantezi",
            "sci-fi" => "Bilim Kurgu",
            "action & adventure" => "Aksiyon & Macera",
            "action" => "Aksiyon",
            "adventure" => "Macera",
            _ => return s.to_string(),
        };
        t.to_string()
    }

    pub fn genre_line(&self) -> Option<String> {
        let gens = self.genres.as_ref()?;
        let mut parts: Vec<String> = gens
            .iter()
            .map(|g| Self::tr_genre(g))
            .filter(|g| g != "Animasyon")
            .collect();
        parts.dedup();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("  •  "))
        }
    }

    fn fmt_release_date(s: &str) -> Option<String> {
        let p: Vec<&str> = s.split('-').collect();
        if p.len() == 3 {
            if let (Ok(y), Ok(m), Ok(d)) = (p[0].parse::<i64>(), p[1].parse::<i64>(), p[2].parse::<i64>()) {
                return Some(format!("{}/{}/{}", d, m, y));
            }
        }
        None
    }

    pub fn detail_facts(&self) -> Vec<String> {
        let mut facts: Vec<String> = Vec::new();
        if let Some(rt) = self.runtime {
            if rt > 0 {
                facts.push(format!("{} dakika", rt));
            }
        }
        match self.episode_count {
            Some(ec) if ec > 0 => facts.push(format!("{} bölüm", ec)),
            _ if self.title_type.as_deref() == Some("movie") => facts.push("Film".to_string()),
            _ => {}
        }
        if let Some(rd) = &self.release_date {
            if let Some(d) = Self::fmt_release_date(rd) {
                facts.push(format!("Yayın: {d}"));
            }
        }
        facts
    }

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
    #[serde(default)]
    pub preferred_source: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_loading")]
    pub loading_style: String,
    #[serde(default = "default_quick_search")]
    pub quick_search_enabled: bool,
    #[serde(default = "default_shortcut")]
    pub quick_search_shortcut: String,
    #[serde(default = "default_search_shortcut")]
    pub search_shortcut: String,
    #[serde(default = "default_true")]
    pub auto_fullscreen: bool,
    #[serde(default = "default_true")]
    pub aniskip_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_update: bool,
    #[serde(default = "default_true")]
    pub notify_uptodate: bool,
    #[serde(default = "default_upscale")]
    pub upscale: String,
    #[serde(default)]
    pub light_mode: bool,
    #[serde(default = "default_patience")]
    pub source_patience_secs: u64,
    #[serde(default)]
    pub default_fansub_template: Option<i64>,
    #[serde(default = "default_true")]
    pub fansub_ask_each_time: bool,
    #[serde(default)]
    pub aicix_api_key: Option<String>,
    #[serde(default = "default_aicix_model")]
    pub aicix_model: String,
}
fn default_loading() -> String { "overlay".into() }
fn default_quick_search() -> bool { true }
fn default_shortcut() -> String { "/".into() }
fn default_search_shortcut() -> String { "Ctrl+S".into() }
fn default_true() -> bool { true }
fn default_upscale() -> String { "hafif".into() }
fn default_patience() -> u64 { 20 }
fn default_aicix_model() -> String { "qwen/qwen3.8-27b".to_string() }

pub(crate) fn upscale_mpv_args(upscale: &str, shader_path: Option<&str>) -> Vec<String> {
    match upscale {
        "sharp" => vec![
            "--scale=ewa_lanczossharp".into(),
            "--cscale=ewa_lanczossharp".into(),
        ],
        "hafif" => match shader_path {
            Some(p) => vec![format!("--glsl-shaders={p}")],
            None => Vec::new(),
        },
        "ultra" => match shader_path {
            Some(p) => vec![format!("--glsl-shaders={p}")],
            None => Vec::new(),
        },
        "hafif_keskin" => match shader_path {
            Some(p) => vec![
                format!("--glsl-shaders={p}"),
                "--vf=unsharp=5:5:1.0:5:5:0.4".into(),
            ],
            None => vec!["--vf=unsharp=5:5:1.0:5:5:0.4".into()],
        },
        _ => Vec::new(),
    }
}

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
            notify_uptodate: default_true(),
            upscale: default_upscale(),
            light_mode: false,
            source_patience_secs: default_patience(),
            default_fansub_template: None,
            fansub_ask_each_time: true,
            aicix_api_key: None,
            aicix_model: default_aicix_model(),
        }
    }
}

impl Client {
    pub fn new() -> Self {
        let proxy = if crate::vpn::port_alive() {
            Some(format!("socks5h://127.0.0.1:{}", crate::vpn::PROXY_PORT))
        } else {
            None
        };
        let http = crate::http::Http::new(proxy.as_deref())
            .unwrap_or_else(|e| panic!("HTTP istemcisi baslatilamadi: {e}"));

        let cache_dir = {
            let mut p = dirs_cache_or_home();
            p.push(".cache/animecix");
            p
        };
        std::fs::create_dir_all(cache_dir.join("api")).ok();
        std::fs::create_dir_all(cache_dir.join("covers")).ok();

        #[cfg(not(test))]
        {
            let ver_path = cache_dir.join("api").join(".cache_version");
            if std::fs::read_to_string(&ver_path).ok().as_deref() != Some(CACHE_VERSION) {
                let _ = std::fs::remove_dir_all(cache_dir.join("api"));
                let _ = std::fs::create_dir_all(cache_dir.join("api"));
                let _ = std::fs::write(&ver_path, CACHE_VERSION);
            }
        }

        Self {
            http,
            cache: std::sync::Mutex::new(HashMap::new()),
            bytes: std::sync::Mutex::new(HashMap::new()),
            resolved: std::sync::Mutex::new(HashMap::new()),
            cache_dir,
            translators: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn load_translators(&self) -> Result<(), String> {
        if !self.translators.lock().unwrap().is_empty() {
            return Ok(());
        }
        let d = self.cache_get("translators:list", 86400, |http| {
            http.get(format!("{BASE}/secure/translators"))
                .header("Accept", "application/json")
                .send()
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json()
                .map_err(|e| e.to_string())
        })?;
        let arr = d.as_array().cloned().unwrap_or_default();
        let mut map = self.translators.lock().unwrap();
        map.clear();
        for t in arr {
            let id = t["id"].as_i64().unwrap_or(0);
            if id == 0 {
                continue;
            }
            let name = t["name"].as_str().unwrap_or("").to_string();
            let long_name = t["translator"].as_str().unwrap_or("").to_string();
            let url = t["translator_url"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string());
            let display = if !name.is_empty() {
                name
            } else {
                long_name.clone()
            };
            map.insert(
                id,
                TranslatorMeta {
                    id,
                    name: display,
                    long_name,
                    url,
                },
            );
        }
        eprintln!("[TRANSLATORS] {} çevirmen yüklendi", map.len());
        Ok(())
    }

    pub fn translator_name(&self, template_id: i64) -> Option<String> {
        self.translators
            .lock()
            .unwrap()
            .get(&template_id)
            .map(|t| t.name.clone())
    }

    pub fn list_translators(&self) -> Vec<TranslatorMeta> {
        self.translators
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }


    fn api_cache_path(&self, key: &str) -> PathBuf {
        let safe: String = key.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        self.cache_dir.join("api").join(format!("{safe}.json"))
    }

    fn img_cache_path(&self, url: &str) -> PathBuf {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in url.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let ext = url.split('?').next().unwrap_or("").rsplit('.').next()
            .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_alphanumeric()))
            .unwrap_or("bin");
        self.cache_dir.join("covers").join(format!("{h:x}.{ext}"))
    }

    pub fn sweep_expired_covers(&self) {
        const COVER_DISK_MAX_BYTES: u64 = 200 * 1024 * 1024;
        let dir = self.cache_dir.join("covers");
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let Ok(entries) = std::fs::read_dir(&dir) else { return; };

        let mut files: Vec<(u64, u64, PathBuf)> = Vec::new(); // (mtime, size, path)
        let mut removed_expired = 0u32;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let meta = match entry.metadata() { Ok(m) => m, Err(_) => continue };
            let mtime = meta.modified().ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(now);
            let size = meta.len();
            if now.saturating_sub(mtime) >= IMG_TTL_SECS {
                let _ = std::fs::remove_file(&path);
                removed_expired += 1;
            } else {
                files.push((mtime, size, path));
            }
        }

        let mut total: u64 = files.iter().map(|f| f.1).sum();
        let mut removed_overcap = 0u32;
        if total > COVER_DISK_MAX_BYTES {
            files.sort_unstable_by_key(|f| f.0); // en eskiden
            for (_, size, path) in &files {
                if total <= COVER_DISK_MAX_BYTES { break; }
                if std::fs::remove_file(path).is_ok() {
                    total = total.saturating_sub(*size);
                    removed_overcap += 1;
                }
            }
        }
        if removed_expired + removed_overcap > 0 {
            eprintln!("[SWEEP] kapak önbelleği: {} süresi dolmuş, {} tavan fazlası silindi", removed_expired, removed_overcap);
        }
    }

    fn disk_api_load(&self, key: &str) -> Option<(u64, serde_json::Value)> {
        let path = self.api_cache_path(key);
        let text = std::fs::read_to_string(&path).ok()?;
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
        loader: impl FnOnce(&crate::http::Http) -> Result<serde_json::Value, String>,
    ) -> Result<serde_json::Value, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        {
            let mem = self.cache.lock().unwrap();
            if let Some((t, v)) = mem.get(key) {
                if now.saturating_sub(*t) < ttl {
                    return Ok(v.clone());
                }
            }
        }

        if let Some((t, v)) = self.disk_api_load(key) {
            if now.saturating_sub(t) < ttl {
                self.cache.lock().unwrap().insert(key.to_string(), (t, v.clone()));
                return Ok(v);
            }
            let stale = v.clone();
            let t0 = std::time::Instant::now();
            let net_res = loader(&self.http);
            if std::env::var_os("ANIMECIX_BENCH").is_some() {
                eprintln!("[bench] api {key} -> {:.1?}ms", t0.elapsed());
            }
            match net_res {
                Ok(fresh) => {
                    self.cache.lock().unwrap().insert(key.to_string(), (now, fresh.clone()));
                    self.disk_api_save(key, now, &fresh);
                    return Ok(fresh);
                }
                Err(_) => {
                    self.cache.lock().unwrap().insert(key.to_string(), (now, stale.clone()));
                    return Ok(stale);
                }
            }
        }

        let t0 = std::time::Instant::now();
        let v = loader(&self.http)?;
        if std::env::var_os("ANIMECIX_BENCH").is_some() {
            eprintln!("[bench] api {key} -> {:.1?}ms", t0.elapsed());
        }
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
                        if it["id"].as_u64().or_else(|| it["title_id"].as_u64()).is_some() {
                            if let Some(t) = Title::from_value(it) {
                                items.push(t);
                            }
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
                if let Some(t) = Title::from_value(r) {
                    out.push(t);
                }
            }
        }
        Ok(out)
    }

    pub fn enrich_title(&self, t: &Title) -> Title {
        if t.genres.is_some() || t.runtime.is_some() || t.release_date.is_some() {
            return t.clone();
        }
        if let Ok(cats) = self.home_lists() {
            for cat in &cats {
                for it in &cat.items {
                    if it.id == t.id {
                        return it.clone();
                    }
                }
            }
        }
        if let Ok(results) = self.search(&t.id.to_string()) {
            if let Some(src) = results.into_iter().find(|x| x.id == t.id) {
                return src;
            }
        }
        if !t.name.is_empty() {
            if let Ok(results) = self.search(&t.name) {
                if let Some(src) = results.into_iter().find(|x| x.id == t.id) {
                    return src;
                }
            }
        }
        t.clone()
    }

    pub fn episodes(&self, t: &Title) -> Result<Vec<Episode>, String> {
        if t.title_type.as_deref() == Some("movie") {
            return self.movie_episodes(t.id);
        }
        let seasons = t.season_count.unwrap_or(1).max(1) as u64;

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

    fn sibnet_resolve(&self, url: &str) -> Result<String, String> {
        let html = self
            .http
            .get(url)
            .timeout(4)
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

    fn streamtape_resolve(&self, url: &str) -> Result<String, String> {
        let html = self
            .http
            .get(url)
            .timeout(4)
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

    fn tau_resolve(&self, embed_id: &str, vid: &str) -> Result<String, String> {
        let mut url = format!("{TAU}/api/video/{embed_id}");
        if !vid.is_empty() {
            url.push_str(&format!("?vid={vid}"));
        }
        let d = self
            .http
            .get(&url)
            .header("Accept", "application/json")
            .timeout(4)
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

    pub fn episode_candidates(&self, title_id: u64, episode: u64, season: u64) -> Result<Vec<String>, String> {
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
            return Ok(Vec::new());
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
            mirrors.sort_by(|a, b| {
                let ta = a["url"].as_str().unwrap_or("").contains("tau-video.xyz");
                let tb = b["url"].as_str().unwrap_or("").contains("tau-video.xyz");
                ta.cmp(&tb).then_with(|| {
                    let va = a["positive_votes"].as_i64().unwrap_or(0);
                    let vb = b["positive_votes"].as_i64().unwrap_or(0);
                    vb.cmp(&va)
                })
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
        Ok(candidates)
    }

    pub fn list_fansubs(&self, title_id: u64, episode: u64, season: u64) -> Result<Vec<FansubInfo>, String> {
        let _ = self.load_translators();
        let key = format!("evp:fansubs:{title_id}:{season}:{episode}");
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
        let points = &d["translatorPoints"];

        let mut groups: std::collections::BTreeMap<i64, (String, f64, i64, bool, Vec<serde_json::Value>)> =
            std::collections::BTreeMap::new();
        for v in &videos {
            let tpl = v["template"].as_i64().unwrap_or(0);
            let translator_name = self
                .translator_name(tpl)
                .unwrap_or_else(|| clean_fansub_name(v["extra"].as_str().unwrap_or("")));
            let approved = v["approved"].as_bool().unwrap_or(false);
            let point = points[tpl.to_string()].as_f64().unwrap_or(0.0);
            let votes = v["positive_votes"].as_i64().unwrap_or(0);
            let g = groups
                .entry(tpl)
                .or_insert((translator_name, 0.0, 0, true, Vec::new()));
            if point > g.1 {
                g.1 = point;
            }
            g.2 += votes;
            if !approved {
                g.3 = false;
            }
            g.4.push(v.clone());
        }

        let mut out: Vec<FansubInfo> = groups
            .into_iter()
            .map(|(tpl, (name, rating, total_votes, approved_only, mut vids))| {
                vids.sort_by(|a, b| {
                    let aa = a["approved"].as_bool().unwrap_or(false);
                    let ab = b["approved"].as_bool().unwrap_or(false);
                    ab.cmp(&aa)
                        .then_with(|| {
                            let ta = a["url"].as_str().unwrap_or("").contains("tau-video.xyz");
                            let tb = b["url"].as_str().unwrap_or("").contains("tau-video.xyz");
                            tb.cmp(&ta)
                        })
                        .then_with(|| {
                            let va = a["positive_votes"].as_i64().unwrap_or(0);
                            let vb = b["positive_votes"].as_i64().unwrap_or(0);
                            vb.cmp(&va)
                        })
                });
                let language = vids
                    .first()
                    .and_then(|v| v["language"].as_str())
                    .unwrap_or("tr")
                    .to_string();
                let mirrors: Vec<FansubMirror> = vids
                    .iter()
                    .map(|v| {
                        let url = v["url"].as_str().unwrap_or("").to_string();
                        FansubMirror {
                            id: v["id"].as_u64().unwrap_or(0),
                            host: Self::source_host_hint(&url).to_string(),
                            url,
                            quality: v["quality"].as_str().map(|s| s.to_string()),
                            approved: v["approved"].as_bool().unwrap_or(false),
                        }
                    })
                    .collect();
                let hosts: Vec<String> = mirrors.iter().map(|m| m.host.clone()).collect();
                FansubInfo {
                    template_id: tpl,
                    name,
                    rating,
                    total_votes,
                    language,
                    approved_only,
                    mirror_count: mirrors.len(),
                    hosts,
                    mirrors,
                }
            })
            .collect();

        out.sort_by(|a, b| {
            b.approved_only
                .cmp(&a.approved_only)
                .then_with(|| b.rating.partial_cmp(&a.rating).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| b.total_votes.cmp(&a.total_votes))
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(out)
    }

    pub fn resolve_top(
        &self,
        title_id: u64,
        episode: u64,
        season: u64,
        k: usize,
        preferred_host: Option<&str>,
    ) -> Result<Vec<(String, String)>, String> {
        let candidates = match self.episode_candidates(title_id, episode, season) {
            Ok(c) if !c.is_empty() => c,
            _ => {
                let all = self.resolve_all(title_id, episode, season)?;
                return Ok(all.into_iter().map(|u| (u, String::new())).collect());
            }
        };
        self.resolve_urls_from(candidates, k, preferred_host)
    }

    pub fn resolve_urls(&self, urls: &[String], k: usize) -> Result<Vec<(String, String)>, String> {
        let candidates: Vec<String> = urls.iter().take(k.max(1)).cloned().collect();
        let n = candidates.len();
        self.resolve_urls_from(candidates, n, None)
    }

    fn resolve_urls_from(
        &self,
        mut candidates: Vec<String>,
        k: usize,
        preferred_host: Option<&str>,
    ) -> Result<Vec<(String, String)>, String> {
        let t0 = std::time::Instant::now();
        if let Some(pref) = preferred_host.filter(|p| !p.is_empty()) {
            candidates.sort_by_key(|u| if Self::source_host_hint(u) == pref { 0u8 } else { 1u8 });
        }
        let tier: Vec<String> = candidates.into_iter().take(k.max(1)).collect();

        let found: Vec<(u64, String, String)> = std::thread::scope(|scope| {
            let (tx, rx) = std::sync::mpsc::channel();
            for u in &tier {
                let tx = tx.clone();
                let http = self.http.clone();
                let u = u.clone();
                scope.spawn(move || {
                    if let Ok(mp4) = self.resolve_embed(&u) {
                        let size = if mp4.contains("video.sibnet.ru") {
                            0
                        } else {
                            http.head(&mp4).timeout(3).send()
                                .ok()
                                .and_then(|r| r.content_length())
                                .unwrap_or(0)
                        };
                        let _ = tx.send((size, mp4, u));
                    }
                });
            }
            drop(tx);
            rx.iter().collect()
        });

        if found.is_empty() {
            return Err("Bölüm videosu çözülemedi".to_string());
        }
        let mut found = found;
        found.sort_by(|a, b| b.0.cmp(&a.0));
        if let Some(pref) = preferred_host.filter(|p| !p.is_empty()) {
            found.sort_by_key(|(_, _, emb)| {
                if Self::source_host_hint(emb) == pref { 0u8 } else { 1u8 }
            });
        }
        eprintln!(
            "[RESOLVE] hızlı kademe: {} aday / {} hazır, {:.2}s{}",
            tier.len(), found.len(), t0.elapsed().as_secs_f64(),
            preferred_host.map(|p| format!(" (tercih: {})", p)).unwrap_or_default()
        );
        Ok(found.into_iter().map(|(_, mp4, emb)| (mp4, emb)).collect())
    }

    fn resolve_ranked(&self, title_id: u64, episode: u64, season: u64) -> Result<Vec<(u64, String)>, String> {
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
            let url = self.tau_resolve(&embed_id, &vid)?;
            return Ok(vec![(0, url)]);
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
            mirrors.sort_by(|a, b| {
                let ta = a["url"].as_str().unwrap_or("").contains("tau-video.xyz");
                let tb = b["url"].as_str().unwrap_or("").contains("tau-video.xyz");
                ta.cmp(&tb).then_with(|| {
                    let va = a["positive_votes"].as_i64().unwrap_or(0);
                    let vb = b["positive_votes"].as_i64().unwrap_or(0);
                    vb.cmp(&va)
                })
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

        let found: Vec<(u64, String)> = std::thread::scope(|scope| {
            let (tx, rx) = std::sync::mpsc::channel();
            for u in candidates {
                let tx = tx.clone();
                let http = self.http.clone();
                scope.spawn(move || {
                    if let Ok(mp4) = self.resolve_embed(&u) {
                        let size = if mp4.contains("video.sibnet.ru") {
                            0
                        } else {
                            http.head(&mp4).send()
                                .ok()
                                .and_then(|r| r.content_length())
                                .unwrap_or(0)
                        };
                        let _ = tx.send((size, mp4));
                    }
                });
            }
            drop(tx);
            rx.iter().collect()
        });

        if found.is_empty() {
            Err("Bölüm videosu çözülemedi".to_string())
        } else {
            Ok(found)
        }
    }

    pub fn resolve(&self, title_id: u64, episode: u64, season: u64) -> Result<String, String> {
        let mut found = self.resolve_ranked(title_id, episode, season)?;
        found.sort_by(|a, b| b.0.cmp(&a.0));
        found.into_iter().next().map(|(_, url)| url).ok_or_else(|| "Bölüm videosu çözülemedi".to_string())
    }

    pub fn resolve_all(&self, title_id: u64, episode: u64, season: u64) -> Result<Vec<String>, String> {
        let mut found = self.resolve_ranked(title_id, episode, season)?;
        found.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(found.into_iter().map(|(_, url)| url).collect())
    }

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
        sources.sort_by(|a, b| {
            b.points.partial_cmp(&a.points).unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.votes.cmp(&a.votes))
        });
        Ok(sources)
    }

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

    pub fn resolve_single(&self, embed_url: &str) -> Result<String, String> {
        self.resolve_embed(embed_url)
    }

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

    pub fn resolve_mal_id(&self, anime_name: &str) -> Option<u64> {
        let mut base = anime_name
            .replace("(TV)", " ")
            .replace("Türkçe", " ")
            .replace("Dublaj", " ")
            .replace("Altyazılı", " ")
            .replace("Çizgi Film", " ")
            .replace("(TV)", " ");
        if let Some(p) = base.find('(') {
            if let Some(q) = base.find(')').and_then(|q| if q > p { Some(q) } else { None }) {
                base.replace_range(p..=q, " ");
            }
        }
        let tokens: Vec<&str> = base
            .split_whitespace()
            .filter(|t| {
                let low = t.to_lowercase();
                let stem = low.trim_end_matches('.');
                !(low.contains("sezon")
                    || low.contains("season")
                    || low.contains("bölüm")
                    || stem.chars().all(|c| c.is_ascii_digit())
                    || matches!(stem, "i" | "ii" | "iii" | "iv" | "v" | "part"))
            })
            .collect();
        let cleaned = tokens.join(" ");

        let mut cands: Vec<String> = Vec::new();
        if !cleaned.is_empty() {
            cands.push(cleaned.clone());
        }
        let words: Vec<&str> = cleaned.split_whitespace().collect();
        if words.len() > 2 {
            let short = words[..2].join(" ");
            cands.push(short);
        }

        for cand in &cands {
            let key = format!("mal_id:{cand}");
            let cand_c = cand.clone();
            if let Ok(d) = self.cache_get(&key, 30 * 86400, move |http| {
                let body = serde_json::json!({
                    "query": "query ($s: String) { Media (search: $s, type: ANIME) { idMal } }",
                    "variables": { "s": cand_c }
                });
                http.post("https://graphql.anilist.co")
                    .json(&body)
                    .timeout(4)
                    .send()
                    .map_err(|e| e.to_string())?
                    .json()
                    .map_err(|e| e.to_string())
            }) {
                if let Some(id) = d["data"]["Media"]["idMal"].as_u64() {
                    return Some(id);
                }
            }
        }
        None
    }

    pub fn fetch_aniskip_timestamps(&self, anime_name: &str, ep_num: u64) -> AniSkipTimes {
        let Some(mal_id) = self.resolve_mal_id(anime_name) else {
            return AniSkipTimes::default();
        };

        let key = format!("aniskip_v4:{mal_id}:{ep_num}");
        let d = match self.cache_get(&key, 6 * 3600, |http| {
            let fetch = |http: &crate::http::Http| -> Result<serde_json::Value, String> {
                http.get(format!("https://api.aniskip.com/v2/skip-times/{mal_id}/{ep_num}?types=op&types=ed&episodeLength=0"))
                    .timeout(8)
                    .send()
                    .map_err(|e| e.to_string())?
                    .error_for_status()
                    .map_err(|e| e.to_string())?
                    .json()
                    .map_err(|e| e.to_string())
            };
            match fetch(http) {
                Ok(v) => Ok(v),
                Err(first) => {
                    let mut last = first;
                    for _ in 0..2 {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                        match fetch(http) {
                            Ok(v) => return Ok(v),
                            Err(e) => last = e,
                        }
                    }
                    Err(last)
                }
            }
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
        eprintln!(
            "[ANISKIP] '{}' E{} mal={} op={:?}-{:?} ed={:?}-{:?}",
            anime_name, ep_num, mal_id, res.op_start, res.op_end, res.ed_start, res.ed_end
        );
        res
    }

    pub fn get_bytes(&self, url: &str) -> Option<Vec<u8>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        {
            let b = self.bytes.lock().unwrap();
            if let Some((t, v)) = b.get(url) {
                if now.saturating_sub(*t) < IMG_TTL_SECS {
                    return Some(v.clone());
                }
            }
        }

        let disk_path = self.img_cache_path(url);
        if let Ok(meta) = std::fs::metadata(&disk_path) {
            let disk_ts = meta.modified().ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now.saturating_sub(disk_ts) < IMG_TTL_SECS {
                if let Ok(data) = std::fs::read(&disk_path) {
                    let mut m = self.bytes.lock().unwrap();
                    m.insert(url.to_string(), (disk_ts, data.clone()));
                    trim_img_cache(&mut m);
                    return Some(data);
                }
            }
        }

        let t0 = std::time::Instant::now();
        let out = self.http.get(url)
            .timeout(12)
            .send().ok()?.bytes().ok().map(|b| b.to_vec());
        if std::env::var_os("ANIMECIX_BENCH").is_some() {
            let host = url.split('/').nth(2).unwrap_or(url);
            eprintln!("[bench] img {} -> {:.1?}ms", host, t0.elapsed());
        }
        if let Some(ref data) = out {
            let _ = std::fs::write(&disk_path, data);
        }
        out
    }

    pub fn warmup(&self) {
        let _ = self.http.get(BASE)
            .header("Accept", "application/json")
            .timeout(5)
            .send();
        let _ = self.http.get("https://image.tmdb.org/t/p/w185/")
            .timeout(5)
            .send();
    }


    pub fn state_path() -> PathBuf {
        let mut p = if let Ok(d) = std::env::var("ANIMECIX_STATE_DIR") {
            PathBuf::from(d)
        } else {
            dirs_cache_or_home()
        };
        p.push("state.json");
        p
    }

    pub fn load_state(&self) -> State {
        let p = Self::state_path();
        let mut st = std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(|v| Self::migrate_state(&v))
            .unwrap_or_default();
        let mut changed = false;
        for m in &mut st.marathon {
            if self.hydrate_title(&mut m.title) {
                changed = true;
            }
        }
        for s in &mut st.saved {
            if self.hydrate_title(s) {
                changed = true;
            }
        }
        for h in &mut st.history {
            if self.hydrate_title(&mut h.title) {
                changed = true;
            }
        }
        if changed {
            self.save_state(&st);
        }
        st
    }

    fn val_to_title(v: &serde_json::Value) -> Option<Title> {
        match v {
            serde_json::Value::Object(_) => {
                let id = v
                    .get("id")
                    .and_then(|x| x.as_u64())
                    .or_else(|| v.get("title_id").and_then(|x| x.as_u64()))
                    .unwrap_or(0);
                let name = v
                    .get("name")
                    .and_then(|x| x.as_str())
                    .or_else(|| v.get("title").and_then(|x| x.as_str()))
                    .unwrap_or("")
                    .to_string();
                let title_type = v
                    .get("title_type")
                    .and_then(|x| x.as_str())
                    .or_else(|| v.get("type").and_then(|x| x.as_str()))
                    .map(|s| s.to_string());
                let poster = v
                    .get("poster")
                    .and_then(|x| x.as_str())
                    .or_else(|| v.get("image").and_then(|x| x.as_str()))
                    .or_else(|| v.get("poster_url").and_then(|x| x.as_str()))
                    .map(|s| s.to_string());
                let year = v.get("year").and_then(|x| x.as_i64());
                let season_count = v
                    .get("season_count")
                    .and_then(|x| x.as_i64())
                    .or_else(|| v.get("seasons").and_then(|x| x.as_i64()));
                let description = v
                    .get("description")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                Some(Title {
                    id,
                    name,
                    year,
                    title_type,
                    poster,
                    description,
                    season_count,
                    ..Default::default()
                })
            }
            serde_json::Value::Number(n) => Some(Title {
                id: n.as_u64().unwrap_or(0),
                ..Default::default()
            }),
            serde_json::Value::String(s) => Some(Title {
                name: s.clone(),
                ..Default::default()
            }),
            _ => None,
        }
    }

    fn migrate_state(v: &serde_json::Value) -> State {
        let obj = match v.as_object() {
            Some(o) => o,
            None => return State::default(),
        };
        let mut st = State::default();
        if let Some(c) = obj.get("current") {
            if let Ok(w) = serde_json::from_value::<Watched>(c.clone()) {
                st.current = Some(w);
            }
        }
        if let Some(w) = obj.get("watched") {
            if let Ok(m) = serde_json::from_value::<HashMap<String, Vec<Watched>>>(w.clone()) {
                st.watched = m;
            }
        }
        if let Some(s) = obj.get("saved") {
            if let Some(arr) = s.as_array() {
                for it in arr {
                    if let Some(t) = Self::val_to_title(it) {
                        st.saved.push(t);
                    }
                }
            }
        }
        if let Some(h) = obj.get("history") {
            if let Some(arr) = h.as_array() {
                for it in arr {
                    let title = it.get("title").and_then(Self::val_to_title);
                    let episode =
                        it.get("episode")
                            .and_then(|x| serde_json::from_value::<Episode>(x.clone()).ok());
                    let ts = it.get("ts").and_then(|x| x.as_u64()).unwrap_or(0);
                    if let (Some(title), Some(episode)) = (title, episode) {
                        st.history.push(HistoryEntry { title, episode, ts });
                    }
                }
            }
        }
        if let Some(m) = obj.get("marathon") {
            if let Some(arr) = m.as_array() {
                for it in arr {
                    let title = it.get("title").and_then(Self::val_to_title);
                    let completed =
                        it.get("completed").and_then(|x| x.as_bool()).unwrap_or(false);
                    let added_at =
                        it.get("added_at").and_then(|x| x.as_u64()).unwrap_or(0);
                    if let Some(title) = title {
                        st.marathon.push(MarathonItem {
                            title,
                            completed,
                            added_at,
                        });
                    }
                }
            }
        }
        st.welcome_seen = obj.get("welcome_seen").and_then(|x| x.as_bool()).unwrap_or(false);
        if let Some(m) = obj.get("preferred_source") {
            if let Some(o) = m.as_object() {
                for (k, v) in o {
                    if let Some(s) = v.as_str() {
                        st.preferred_source.insert(k.clone(), s.to_string());
                    }
                }
            }
        }
        if let Some(p) = obj.get("progress") {
            if let Ok(m) = serde_json::from_value::<HashMap<String, (f64, f64)>>(p.clone()) {
                st.progress = m;
            }
        }
        st.quick_search_tip_seen = obj
            .get("quick_search_tip_seen")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        st.search_tip_seen = obj.get("search_tip_seen").and_then(|x| x.as_bool()).unwrap_or(false);
        st.right_click_tip_seen = obj
            .get("right_click_tip_seen")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        st
    }

    fn hydrate_title(&self, t: &mut Title) -> bool {
        let complete = !t.name.is_empty()
            && t.title_type.is_some()
            && t.year.is_some()
            && t.season_count.is_some();
        if complete {
            return false;
        }
        let mut changed = false;
        if let Some(src) = self.cached_title_by_id(t.id) {
            if t.name.is_empty() {
                t.name = src.name;
                changed = true;
            }
            if t.poster.is_none() {
                t.poster = src.poster;
            }
            if t.title_type.is_none() {
                t.title_type = src.title_type;
            }
            if t.year.is_none() {
                t.year = src.year;
            }
            if t.season_count.is_none() {
                t.season_count = src.season_count;
            }
            if t.description.is_none() {
                t.description = src.description;
            }
            return changed;
        }
        if t.name.is_empty() {
            if let Ok(results) = self.search(&t.id.to_string()) {
                if let Some(src) = results.into_iter().find(|x| x.id == t.id) {
                    t.name = src.name;
                    if t.poster.is_none() {
                        t.poster = src.poster;
                    }
                    if t.title_type.is_none() {
                        t.title_type = src.title_type;
                    }
                    if t.year.is_none() {
                        t.year = src.year;
                    }
                    if t.season_count.is_none() {
                        t.season_count = src.season_count;
                    }
                    if t.description.is_none() {
                        t.description = src.description;
                    }
                    changed = true;
                }
            }
        }
        changed
    }

    fn cached_title_by_id(&self, id: u64) -> Option<Title> {
        let value = {
            let mem = self.cache.lock().unwrap();
            if let Some((_, v)) = mem.get("lists") {
                Some(v.clone())
            } else {
                self.disk_api_load("lists").map(|(_, v)| v)
            }
        }?;
        let mut found = None;
        if let Some(lists) = value.get("lists").and_then(|l| l.as_array()) {
            for lst in lists {
                if let Some(items) = lst.get("items").and_then(|i| i.as_array()) {
                    for it in items {
                        let tid = it
                            .get("id")
                            .and_then(|x| x.as_u64())
                            .or_else(|| it.get("title_id").and_then(|x| x.as_u64()));
                        if tid == Some(id) {
                            found = Some(Title {
                                id,
                                name: it
                                    .get("name")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                year: it.get("year").and_then(|x| x.as_i64()),
                                title_type: it
                                    .get("title_type")
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.to_string()),
                                poster: it
                                    .get("poster")
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.to_string()),
                                description: it
                                    .get("description")
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.to_string()),
                                season_count: it.get("season_count").and_then(|x| x.as_i64()),
                                ..Default::default()
                            });
                            break;
                        }
                    }
                }
                if found.is_some() {
                    break;
                }
            }
        }
        found
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

    pub fn is_watched(&self, title_id: u64, season: u64, episode: u64) -> bool {
        let st = self.load_state();
        st.watched
            .get(&title_id.to_string())
            .map(|l| l.iter().any(|x| x.episode == episode && x.season == season))
            .unwrap_or(false)
    }

    pub fn set_current(&self, w: &Watched) {
        let mut st = self.load_state();
        st.current = Some(Watched {
            title_id: w.title_id,
            episode: w.episode,
            season: w.season,
        });
        self.save_state(&st);
    }

    pub fn played_enough(pos: f64, dur: f64) -> bool {
        dur > 0.0 && (pos / dur) >= 0.9
    }

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

    pub fn episode_count(&self, t: &Title) -> Option<u64> {
        self.episodes(t).ok().map(|e| e.len() as u64)
    }

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


    pub fn settings_path() -> PathBuf {
        let mut p = dirs_cache_or_home();
        p.push(".local/share/animecix/settings.json");
        p
    }

    pub fn source_host_hint(url: &str) -> &'static str {
        if url.contains("tau-video") { "tau-video" }
        else if url.contains("sibnet.ru") { "sibnet.ru" }
        else if url.contains("streamtape") { "streamtape" }
        else if url.contains("vudeo") { "vudeo" }
        else if url.contains("streamsb") || url.contains("sbplay") { "streamsb" }
        else if url.contains("dood") { "dood" }
        else if url.contains("ok.ru") || url.contains("odnoklassniki") { "ok.ru" }
        else if url.contains("drive.google") { "gdrive" }
        else { "" }
    }

    pub fn get_preferred_host(&self, title_id: u64) -> Option<String> {
        let st = self.load_state();
        st.preferred_source.get(&title_id.to_string()).cloned()
    }

    pub fn set_preferred_host(&self, title_id: u64, host_hint: &str) {
        if host_hint.is_empty() { return; }
        let mut st = self.load_state();
        st.preferred_source.insert(title_id.to_string(), host_hint.to_string());
        self.save_state(&st);
        eprintln!("[PREF] tid={} tercih edilen kaynak kaydedildi: {}", title_id, host_hint);
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

    pub fn wipe_all_data(&self) {
        if let Ok(mut c) = self.cache.lock() { c.clear(); }
        if let Ok(mut b) = self.bytes.lock() { b.clear(); }
        if let Ok(mut r) = self.resolved.lock() { r.clear(); }

        let st = State::default();
        self.save_state(&st);
        self.save_settings(&Settings::default());

        let mut cache_dir = dirs_cache_or_home();
        cache_dir.push(".cache/animecix");
        let _ = std::fs::remove_dir_all(&cache_dir);
        let _ = std::fs::create_dir_all(&cache_dir);

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

    pub fn save_progress(&self, tid: u64, season: u64, episode: u64, pos: f64, dur: f64) {
        let key = format!("{tid}:{season}:{episode}");
        let mut st = self.load_state();
        st.progress.insert(key, (pos, dur));
        self.save_state(&st);
    }

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

    static STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn use_isolated_state() {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let dir = std::env::temp_dir().join(format!("animecix-test-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            std::env::set_var("ANIMECIX_STATE_DIR", &dir);
        });
    }

    fn sample(tt: &str, year: Option<i64>, seasons: Option<i64>) -> Title {
        Title {
            id: 1,
            name: "X".into(),
            year,
            title_type: Some(tt.into()),
            poster: None,
            description: None,
            season_count: seasons,
            ..Default::default()
        }
    }

    #[test]
    fn warmup_is_safe_offline() {
        let c = Client::new();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c.warmup();
        }));
        assert!(r.is_ok(), "warmup panic etmemeli");
    }

    #[test]
    fn cache_get_caches_and_reuses() {
        let c = Client::new();
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let key = format!("test:cache_reuse:{}:{}", std::process::id(), n);
        let v1 = c.cache_get(&key, 60, |_h| Ok(serde_json::json!({"n": n}))).unwrap();
        let v2 = c.cache_get(&key, 60, |_h| panic!("loader tekrar çağrılmamalı (cache miss)")).unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn upscale_mpv_args_modes() {
        assert!(super::upscale_mpv_args("off", None).is_empty());
        assert!(super::upscale_mpv_args("kapali", None).is_empty());
        let sharp = super::upscale_mpv_args("sharp", None);
        assert!(sharp.iter().any(|a| a == "--scale=ewa_lanczossharp"));
        assert!(sharp.iter().any(|a| a == "--cscale=ewa_lanczossharp"));
        assert!(!sharp.iter().any(|a| a.contains("dsharpen")));
        assert!(super::upscale_mpv_args("hafif", None).is_empty());
        assert!(super::upscale_mpv_args("ultra", None).is_empty());
        let hk_none = super::upscale_mpv_args("hafif_keskin", None);
        assert!(hk_none.iter().any(|a| a.starts_with("--vf=unsharp")));
        let ak = super::upscale_mpv_args("ultra", Some("/p/Anime4K.glsl"));
        assert_eq!(ak, vec!["--glsl-shaders=/p/Anime4K.glsl".to_string()]);
        let ak2 = super::upscale_mpv_args("hafif", Some("/p/Anime4K.glsl"));
        assert_eq!(ak2, vec!["--glsl-shaders=/p/Anime4K.glsl".to_string()]);
        let ak3 = super::upscale_mpv_args("hafif_keskin", Some("/p/Anime4K.glsl"));
        assert!(ak3.iter().any(|a| a == "--glsl-shaders=/p/Anime4K.glsl"));
        assert!(ak3.iter().any(|a| a.starts_with("--vf=unsharp")));
    }

    #[test]
    fn meta_line_anime_with_seasons() {
        let t = sample("anime", Some(2021), Some(2));
        assert_eq!(t.meta_line(), "2021  •  Anime Serisi  •  2 Sezon");
    }

    #[test]
    fn detail_meta_parses_real_shape() {
        let v = serde_json::json!({
            "id": 11130,
            "name": "Sousou no Frieren",
            "year": 2023,
            "title_type": "anime",
            "runtime": 24,
            "episode_count": 38,
            "release_date": "2023-09-29",
            "genres": [
                {"name": "drama", "display_name": "Dram"},
                {"name": "animation", "display_name": "Animasyon"},
                {"name": "sci-fi-fantasy", "display_name": "Sci-Fi & Fantasy"},
                {"name": "action-adventure", "display_name": "Action & Adventure"}
            ]
        });
        let t = Title::from_value(&v).unwrap();
        assert_eq!(t.display_name(), "Sousou no Frieren (2023)");
        assert_eq!(
            t.genre_line(),
            Some("Dram  •  Bilim Kurgu & Fantezi  •  Aksiyon & Macera".to_string())
        );
        assert_eq!(
            t.detail_facts(),
            vec![
                "24 dakika".to_string(),
                "38 bölüm".to_string(),
                "Yayın: 29/9/2023".to_string()
            ]
        );
    }

    #[test]
    fn enrich_title_short_circuits_when_populated() {
        let c = Client::new();
        let mut t = sample("anime", Some(2021), Some(2));
        t.genres = Some(vec!["Dram".to_string()]);
        let e = c.enrich_title(&t);
        assert_eq!(e.id, t.id);
        assert_eq!(e.genres, Some(vec!["Dram".to_string()]));
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
        let a = sample("anime", Some(2022), Some(3));
        let b = sample("anime", Some(2022), Some(3));
        assert_eq!(a.meta_line(), b.meta_line());
    }

    #[test]
    fn marathon_persists_full_title_and_meta() {
        use_isolated_state();
        let _g = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let c = Client::new();
        let before = c.get_marathon().len();
        let t = Title {
            id: 4242,
            name: "Deneme Anime".into(),
            year: Some(2023),
            title_type: Some("anime".into()),
            poster: Some("http://x/p.png".into()),
            description: None,
            season_count: Some(3),
            ..Default::default()
        };
        assert!(c.toggle_marathon(&t));
        let items = c.get_marathon();
        assert_eq!(items.len(), before + 1, "maraton öğesi eklenmeli");
        let mine = items
            .iter()
            .find(|m| m.title.id == 4242)
            .expect("eklenen öğe bulunmalı");
        assert_eq!(mine.title.name, "Deneme Anime", "isim korunmalı");
        assert!(!mine.title.meta_line().is_empty(), "meta korunmalı");
        c.toggle_marathon(&t); // temizlik
    }

    #[test]
    fn marathon_item_serde_roundtrip_keeps_name() {
        let item = MarathonItem {
            title: Title {
                id: 7,
                name: "Foo".into(),
                year: Some(2000),
                title_type: Some("movie".into()),
                poster: None,
                description: None,
                season_count: None,
                ..Default::default()
            },
            completed: true,
            added_at: 123,
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: MarathonItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back.title.name, "Foo");
        assert_eq!(back.title.meta_line(), "2000  •  Film");
    }

    #[test]
    #[test]
    fn settings_source_patience_roundtrips() {
        let mut s = Settings::default();
        assert_eq!(s.source_patience_secs, 20);
        s.source_patience_secs = 40;
        let json = serde_json::to_string_pretty(&s).expect("serialize");
        assert!(json.contains("source_patience_secs"), "alan JSON'da olmalı");
        let back: Settings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.source_patience_secs, 40, "sabır korunmalı");
    }

    fn migrate_state_recovers_bare_id_and_alias_fields() {
        let value: serde_json::Value = serde_json::json!({
            "marathon": [
                { "title": 777, "completed": false, "added_at": 1 }
            ],
            "saved": [
                { "title": "Eski Favori", "id": 999 }
            ]
        });
        let st = Client::migrate_state(&value);
        assert_eq!(st.marathon.len(), 1, "bozuk kayıt state'i çökertmemeli");
        assert_eq!(st.marathon[0].title.id, 777, "düz id'den Title.id çıkarılmalı");
        assert_eq!(st.saved.len(), 1);
        assert_eq!(st.saved[0].name, "Eski Favori", "eski `title` alanı isim olarak okunmalı");
        assert_eq!(st.saved[0].id, 999);
    }

    #[test]
    fn marathon_hydrates_missing_name_from_disk_cache() {
        use_isolated_state();
        let _g = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let c = Client::new();
        let mut dir = dirs_cache_or_home();
        dir.push(".cache/animecix/api");
        std::fs::create_dir_all(&dir).ok();
        let lists_path = dir.join("lists.json");
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let json = r#"{"lists":[{"name":"Popüler","items":[{"id":777,"name":"Hidratasyon Testi","title_type":"anime","year":2021,"season_count":2,"poster":null}]}]}"#;
        std::fs::write(&lists_path, format!("{ts}\n{json}")).ok();

        let mut st = c.load_state();
        st.marathon.push(MarathonItem {
            title: Title {
                id: 777,
                name: String::new(),
                year: None,
                title_type: None,
                poster: None,
                description: None,
                season_count: None,
                ..Default::default()
            },
            completed: false,
            added_at: 0,
        });
        c.save_state(&st);

        let items = c.get_marathon();
        let m = items.iter().find(|x| x.title.id == 777).expect("öğe olmalı");
        assert_eq!(m.title.name, "Hidratasyon Testi", "isim önbellekten doldurulmalı");
        assert_eq!(m.title.title_type.as_deref(), Some("anime"));

        let mut st2 = c.load_state();
        st2.marathon.retain(|x| x.title.id != 777);
        c.save_state(&st2);
        let _ = std::fs::remove_file(&lists_path);
    }

    #[test]
    fn next_episode_not_marked_watched_before_playing() {
        use_isolated_state();
        let _g = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let c = Client::new();
        let tid: u64 = 999_991;
        let w1 = Watched { title_id: tid, episode: 1, season: 0 };
        let w2 = Watched { title_id: tid, episode: 2, season: 0 };

        c.set_current(&w1);
        assert!(!c.is_watched(tid, 0, 1), "açılan bölüm henüz izlenmedi sayılmamalı");

        assert!(Client::played_enough(95.0, 100.0), ">=%90 izlendi sayılmalı");
        assert!(!Client::played_enough(45.0, 100.0), "<%90 izlenmedi sayılmalı");
        assert!(!Client::played_enough(0.0, 0.0), "süresiz içerik izlenmiş sayılmaz");

        c.save_watched(&w1, "");
        assert!(c.is_watched(tid, 0, 1), "eşik aşılınca izlendi sayılmalı");

        c.set_current(&w2);
        assert!(!c.is_watched(tid, 0, 2), "sonraki bölüm geçişte izlenmeden tamamlanmamalı");

        c.save_watched(&w2, "");
        assert!(c.is_watched(tid, 0, 2), "sonraki bölüm izlenince işaretlenmeli");

        c.remove_watched(tid, 0, 1);
        c.remove_watched(tid, 0, 2);
        let mut st = c.load_state();
        st.current = None;
        c.save_state(&st);
    }

    #[test]
    fn resolve_embed_drops_unknown_hosts() {
        let c = Client::new();
        assert!(c.resolve_embed("https://odnoklassniki.ru/videoembed/2099802475096").is_err(), "ok.ru elenmeli");
        assert!(c.resolve_embed("https://drive.google.com/file/d/x/preview").is_err(), "gdrive elenmeli");
    }

    #[test]
    #[ignore] // canlı ağ testi: bazı ağlarda hedef hostlar engelli olabilir.
    fn resolve_all_live_returns_playable_sibnet() {
        let _g = STATE_LOCK.lock().unwrap();
        let c = Client::new();
        let mut urls = None;
        for _ in 0..3 {
            if let Ok(u) = c.resolve_all(7354, 7, 1) { urls = Some(u); break; }
            std::thread::sleep(std::time::Duration::from_millis(700));
        }
        let urls = urls.expect("resolve_all 3 denemede de başarısız (ağ?)");
        eprintln!("[live] çözülen kaynaklar: {urls:?}");
        assert!(!urls.is_empty(), "en az bir kaynak çözülmeli");
        assert!(
            urls.iter().any(|u| u.contains("video.sibnet.ru/v/") && u.ends_with(".mp4")),
            "ep7 sibnet mp4'ü çözümlenmiş olmalı; gelen: {urls:?}"
        );
    }

    #[test]
    fn aniskip_live_finds_non_non_biyori_ep9() {
        let c = Client::new();
        let t = c.fetch_aniskip_timestamps("Non Non Biyori", 9);
        eprintln!("[live] NNB E9 aniskip: {t:?}");
        assert!(t.op_end.is_some(), "ep9 intro (op) zamanları bulunmalı; gelen: {t:?}");
        assert!(t.ed_start.is_some(), "ep9 outro (ed) zamanları bulunmalı; gelen: {t:?}");
        assert!(t.op_end.unwrap() < 360.0, "op_end makul olmalı");
    }

    #[test]
    #[ignore]
    fn resolve_top_live_fast_tier() {
        let _g = STATE_LOCK.lock().unwrap();
        let c = Client::new();
        let mut pairs: Option<Vec<(String, String)>> = None;
        for _ in 0..3 {
            let t0 = std::time::Instant::now();
            if let Ok(p) = c.resolve_top(7354, 7, 1, 3, None) {
                eprintln!("[live] resolve_top: {} kaynak, {:.2}s", p.len(), t0.elapsed().as_secs_f64());
                pairs = Some(p);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(700));
        }
        let pairs = pairs.expect("resolve_top 3 denemede de başarısız (ağ?)");
        assert!(!pairs.is_empty());
    }

    #[test]
    #[ignore]
    fn resolve_top_preferred_host_comes_first() {
        let _g = STATE_LOCK.lock().unwrap();
        let c = Client::new();
        let mut pairs: Option<Vec<(String, String)>> = None;
        for _ in 0..3 {
            if let Ok(p) = c.resolve_top(7354, 11, 1, 3, Some("sibnet.ru")) {
                pairs = Some(p);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(700));
        }
        let pairs = pairs.expect("resolve_top(ep11) 3 denemede de başarısız (ağ?)");
        eprintln!("[live] ep11 tercihli: {:?}", pairs.iter().map(|(_,e)| Client::source_host_hint(e)).collect::<Vec<_>>());
        assert!(!pairs.is_empty());
        let first_hint = Client::source_host_hint(&pairs[0].1);
        assert_eq!(first_hint, "sibnet.ru", "tercih edilen host ilk sırada olmalı; gelen: {pairs:?}");
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    #[test]
    #[ignore]
    fn home_lists_live() {
        let c = Client::new();
        let cats = c.home_lists().expect("home_lists basarisiz");
        println!("kategoriler: {}", cats.len());
        assert!(!cats.is_empty());
    }

    #[test]
    #[ignore]
    fn search_live() {
        let c = Client::new();
        let r = c.search("one piece").expect("search basarisiz");
        println!("sonuc: {}", r.len());
        assert!(!r.is_empty());
    }
}

pub(crate) fn clean_fansub_name(raw: &str) -> String {
    let s = raw.trim().trim_start_matches('|').trim();
    if s.is_empty() {
        return "Bilinmeyen".to_string();
    }

    let lower = s.to_lowercase();

    let known = [
        ("raionsubs", "RaionSubs"),
        ("gachaflex", "GachaFlex Fansub"),
        ("shyphic - surui", "shyphic - Surui"),
        ("shyphic", "shyphic"),
        ("surui", "Surui"),
        ("kirigana", "Kirigana"),
        ("wolwead", "Wolwead"),
        ("ugurmaden", "ugurmaden"),
        ("puzzlefansub", "PuzzleFansub"),
        ("aoisubs", "AoiSubs"),
        ("anikeyf", "AniKeyf"),
        ("xerneas", "xerneas"),
    ];
    for (key, label) in known {
        if lower.contains(key) {
            return label.to_string();
        }
    }

    let stripped = s
        .strip_prefix("Çevirmen:")
        .or_else(|| s.strip_prefix("Çeviri:"))
        .unwrap_or(s)
        .trim();

    let pipe_end = stripped.find('|').unwrap_or(stripped.len());
    let part = &stripped[..pipe_end];

    let lower2 = part.to_lowercase();
    let cut = lower2
        .find("redakte")
        .or_else(|| lower2.find("encode"))
        .unwrap_or(part.len());
    let name = part[..cut].trim();
    if name.is_empty() {
        part.trim().to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod fansub_tests {
    use super::*;

    #[test]
    fn cleans_wolwead_credit() {
        assert_eq!(clean_fansub_name("Çevirmen: zafhy | Encoder: Wolwead"), "Wolwead");
    }

    #[test]
    fn cleans_plain_name() {
        assert_eq!(clean_fansub_name("ugurmaden"), "ugurmaden");
        assert_eq!(clean_fansub_name(" PuzzleFansub"), "PuzzleFansub");
    }

    #[test]
    fn handles_empty() {
        assert_eq!(clean_fansub_name(""), "Bilinmeyen");
        assert_eq!(clean_fansub_name("   "), "Bilinmeyen");
    }

    #[test]
    fn cleans_ceviriri_prefix() {
        assert_eq!(clean_fansub_name("Çeviri: xerneas"), "xerneas");
    }

    #[test]
    fn cleans_redakte_suffix() {
        assert_eq!(
            clean_fansub_name("Çeviri: xerneas Redakte: Shauna Encoded: y"),
            "xerneas"
        );
    }

    #[test]
    fn cleans_multiple_prefixes() {
        assert_eq!(clean_fansub_name("Encoder: Wolwead | Sürüm: 2"), "Wolwead");
    }

    #[test]
    fn cleans_raionsubs_full_credit() {
        assert_eq!(
            clean_fansub_name("Çeviri: xerneas Redakte: Shauna Encode: YerliOyuncu919 / www.raionsubs.com"),
            "RaionSubs"
        );
    }

    #[test]
    fn keeps_shyphic_pair() {
        assert_eq!(clean_fansub_name("shyphic - Surui"), "shyphic - Surui");
    }
}
