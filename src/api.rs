use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const BASE: &str = "https://animecix.tv";
const TAU: &str = "https://tau-video.xyz";
const TTL_SECS: u64 = 600;

pub struct Client {
    http: reqwest::blocking::Client,
    cache: std::sync::Mutex<HashMap<String, (u64, serde_json::Value)>>,
    bytes: std::sync::Mutex<HashMap<String, (u64, Vec<u8>)>>,
    resolved: std::sync::Mutex<HashMap<String, (u64, String)>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Title {
    pub id: u64,
    pub name: String,
    pub year: Option<i64>,
    pub title_type: Option<String>,
    pub poster: Option<String>,
    pub description: Option<String>,
    pub season_count: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct Category {
    pub name: String,
    pub items: Vec<Title>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct State {
    pub current: Option<Watched>,
    pub watched: HashMap<String, Vec<Watched>>,
    #[serde(default)]
    pub saved: Vec<Title>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
}

impl Client {
    pub fn new() -> Self {
        let http = reqwest::blocking::Client::builder()
            .user_agent(UA)
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("http client");
        Self {
            http,
            cache: std::sync::Mutex::new(HashMap::new()),
            bytes: std::sync::Mutex::new(HashMap::new()),
            resolved: std::sync::Mutex::new(HashMap::new()),
        }
    }

    fn cache_get(
        &self,
        key: &str,
        loader: impl FnOnce(&reqwest::blocking::Client) -> Result<serde_json::Value, String>,
    ) -> Result<serde_json::Value, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        {
            let cache = self.cache.lock().unwrap();
            if let Some((t, v)) = cache.get(key) {
                if now - *t < TTL_SECS {
                    return Ok(v.clone());
                }
            }
        }
        let v = loader(&self.http)?;
        self.cache.lock().unwrap().insert(key.to_string(), (now, v.clone()));
        Ok(v)
    }

    pub fn home_lists(&self) -> Result<Vec<Category>, String> {
        let d = self.cache_get("lists", |http| {
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
        let d = self.cache_get(&key, |http| {
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
        let mut all: Vec<Episode> = Vec::new();
        for s in 1..=seasons {
            let key = format!("ep:{tid}:{s}", tid = t.id);
            let d = self.cache_get(&key, |http| {
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
            if let Some(vids) = d["videos"].as_array() {
                for v in vids {
                    let ep = v["episode_num"].as_u64().unwrap_or(0);
                    let sez = v["season_num"].as_u64().unwrap_or(0);
                    if ep == 0 || sez != s {
                        continue;
                    }
                    if all.iter().any(|x| x.season == sez && x.episode == ep) {
                        continue;
                    }
                    all.push(Episode {
                        episode: ep,
                        season: sez,
                        name: v["name"]
                            .as_str()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("Bölüm {ep}")),
                    });
                }
            }
        }
        all.sort_by_key(|e| (e.season, e.episode));
        Ok(all)
    }

    /// Tüm sayfaları gezen film/dizi video listesi (kategori: full)
    fn movie_videos(&self, title_id: u64) -> Result<serde_json::Value, String> {
        let key = format!("mvid:{title_id}");
        self.cache_get(&key, |http| {
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

    /// Film için en yüksek oylu kaynağı çözer (tau dışı hostlar da dener)
    pub fn resolve_movie(&self, title_id: u64) -> Result<String, String> {
        let d = self.movie_videos(title_id)?;
        let mut tau: Vec<(serde_json::Value, i64)> = Vec::new();
        let mut other: Vec<(serde_json::Value, i64)> = Vec::new();
        if let Some(data) = d["data"].as_array() {
            for v in data {
                if v["category"].as_str() != Some("full") {
                    continue;
                }
                let url = v["url"].as_str().unwrap_or("");
                if url.is_empty() {
                    continue;
                }
                let votes = v["positive_votes"].as_i64().unwrap_or(0);
                if url.contains("tau-video.xyz") {
                    tau.push((v.clone(), votes));
                } else {
                    other.push((v.clone(), votes));
                }
            }
        }
        tau.sort_by(|a, b| b.1.cmp(&a.1));
        other.sort_by(|a, b| b.1.cmp(&a.1));
        let mut cands: Vec<serde_json::Value> = Vec::new();
        cands.extend(tau.into_iter().take(2).map(|(v, _)| v));
        cands.extend(other.into_iter().take(3).map(|(v, _)| v));
        let mut errs = Vec::new();
        for v in cands.iter() {
            let url = v["url"].as_str().unwrap_or("");
            match self.resolve_embed(url) {
                Ok(mp4) => return Ok(mp4),
                Err(e) => errs.push(e),
            }
        }
        Err(format!(
            "film kaynağı çözülemedi: {}",
            errs.first().cloned().unwrap_or_else(|| "kaynak yok".to_string())
        ))
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
        let ckey = format!("tau:{embed_id}:{vid}");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        {
            let r = self.resolved.lock().unwrap();
            if let Some((t, u)) = r.get(&ckey) {
                if now - *t < TTL_SECS {
                    return Ok(u.clone());
                }
            }
        }
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
            Some((_, u)) => {
                self.resolved.lock().unwrap().insert(ckey, (now, u.clone()));
                Ok(u)
            }
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

    /// Direct mp4 URL'si döner (1080p tercih, yoksa ilk)
    /// Bölümün çeviri seçeneklerinden en yüksek puanlısını seçip çözer
    pub fn resolve(&self, title_id: u64, episode: u64, season: u64) -> Result<String, String> {
        let key = format!("evp:{title_id}:{season}:{episode}");
        let d = self.cache_get(&key, |http| {
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

        // çevirmen (template) bazında grupla: puan + toplam oy
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

        let mut errs = Vec::new();
        let mut attempts = 0usize;
        for g in groups.iter() {
            let mut mirrors = g.3.clone();
            mirrors.sort_by_key(|v| {
                let is_tau = v["url"]
                    .as_str()
                    .unwrap_or("")
                    .contains("tau-video.xyz");
                !is_tau
            });
            for v in mirrors.iter() {
                if attempts >= 5 {
                    break;
                }
                let url = v["url"].as_str().unwrap_or("");
                if url.is_empty() {
                    continue;
                }
                attempts += 1;
                match self.resolve_embed(url) {
                    Ok(mp4) => return Ok(mp4),
                    Err(e) => errs.push(e),
                }
            }
        }
        Err(format!(
            "bölüm kaynağı çözülemedi: {}",
            errs.first().cloned().unwrap_or_else(|| "kaynak yok".to_string())
        ))
    }

    pub fn get_bytes(&self, url: &str) -> Option<Vec<u8>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        {
            let b = self.bytes.lock().unwrap();
            if let Some((t, v)) = b.get(url) {
                if now - *t < TTL_SECS {
                    return Some(v.clone());
                }
            }
        }
        let out = self.http.get(url).send().ok()?.bytes().ok().map(|b| b.to_vec());
        if let Some(v) = &out {
            self.bytes.lock().unwrap().insert(url.to_string(), (now, v.clone()));
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

    pub fn clear_history(&self) {
        let mut st = self.load_state();
        st.history.clear();
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
}

fn dirs_cache_or_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_default()
}