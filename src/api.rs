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
/// API disk cache şema sürümü. Title yapısı yeni alanlar (genres/runtime/...)
/// kazandığında bu değer artırılır; böylece eski önbellek otomatik temizlenir.
const CACHE_VERSION: &str = "2";

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
    /// Genre display_name listesi (API'den "genres[].display_name")
    #[serde(default)]
    pub genres: Option<Vec<String>>,
    /// Bölüm/dakika uzunluğu (API'den "runtime")
    #[serde(default)]
    pub runtime: Option<i64>,
    /// Toplam bölüm sayısı (API'den "episode_count")
    #[serde(default)]
    pub episode_count: Option<i64>,
    /// Yayın tarihi "YYYY-MM-DD" (API'den "release_date")
    #[serde(default)]
    pub release_date: Option<String>,
}

impl Title {
    /// JSON değerinden Title üretir (id, ad, yıl, tür, poster, açıklama, sezon,
    /// genres, runtime, episode_count ve release_date dahil).
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

    /// Başlık satırı: "Ad (YIL)" veya yıl yoksa sadece ad.
    pub fn display_name(&self) -> String {
        match self.year {
            Some(y) => format!("{} ({})", self.name, y),
            None => self.name.clone(),
        }
    }

    /// TMDB genre adını Türkçe'ye çevirir (bilinmeyen olduğu gibi döner).
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

    /// Çevrilmiş tür satırı: "Dram  •  Bilim Kurgu & Fantezi  •  Aksiyon & Macera".
    /// "Animasyon" gereksiz olduğu için düşürülür.
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

    /// "YYYY-MM-DD" -> "D/M/YYYY" (başına sıfır yok)
    fn fmt_release_date(s: &str) -> Option<String> {
        let p: Vec<&str> = s.split('-').collect();
        if p.len() == 3 {
            if let (Ok(y), Ok(m), Ok(d)) = (p[0].parse::<i64>(), p[1].parse::<i64>(), p[2].parse::<i64>()) {
                return Some(format!("{}/{}/{}", d, m, y));
            }
        }
        None
    }

    /// Detay kartında rozet olarak gösterilecek bilgi parçaları:
    /// ["24 dakika", "38 bölüm", "Yayın: 29/9/2023"]
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
    /// Başlatmada güncel sürümdeyken "Güncel" bildirimini göster
    #[serde(default = "default_true")]
    pub notify_uptodate: bool,
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
            notify_uptodate: default_true(),
        }
    }
}

impl Client {
    pub fn new() -> Self {
        let http = reqwest::blocking::Client::builder()
            .user_agent(UA)
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(3))
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(std::time::Duration::from_secs(60))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .build()
            .expect("http client");

        let cache_dir = {
            let mut p = dirs_cache_or_home();
            p.push(".cache/animecix");
            p
        };
        std::fs::create_dir_all(cache_dir.join("api")).ok();
        std::fs::create_dir_all(cache_dir.join("covers")).ok();

        // Şema sürümü değiştiyse eski API önbelleğini sıfırla (yeni Title alanları
        // için: genres/runtime/episode_count/release_date).
        let ver_path = cache_dir.join("api").join(".cache_version");
        if std::fs::read_to_string(&ver_path).ok().as_deref() != Some(CACHE_VERSION) {
            let _ = std::fs::remove_dir_all(cache_dir.join("api"));
            let _ = std::fs::create_dir_all(cache_dir.join("api"));
            let _ = std::fs::write(&ver_path, CACHE_VERSION);
        }

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
                    // İnternet yoksa stale veriyi kullan
                    self.cache.lock().unwrap().insert(key.to_string(), (now, stale.clone()));
                    return Ok(stale);
                }
            }
        }

        // 3) Ağ isteği
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

    /// state.json'dan yüklenen (eski şemalı) başlıklar yeni alanlara (genres,
    /// runtime, episode_count, release_date) sahip olmayabilir. Bu yöntem, başlık
    /// verisini yalnızca önbellekten (bellek/disk) tazeleyerek zengin metadata'yı
    /// doldurur — ek ağ isteği YAPMAZ (hız regresyonu olmasın). Zaten doluysa veya
    /// önbellekte bulunamazsa olduğu gibi döner.
    pub fn enrich_title(&self, t: &Title) -> Title {
        if t.genres.is_some() || t.runtime.is_some() || t.release_date.is_some() {
            return t.clone();
        }
        // Yalnızca ana sayfa listeleri önbelleğinde (bellek+disk, 3s) id eşleşmesi ara.
        // Ağ çağrısı yok; ana sayfa zaten gezildiyse neredeyse anlık döner.
        if let Ok(cats) = self.home_lists() {
            for cat in &cats {
                for it in &cat.items {
                    if it.id == t.id {
                        return it.clone();
                    }
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

    /// Bölüm için tüm çözülmüş kaynakları (boyut = kalite sırasıyla, en iyi önce) döner.
    /// `resolve` yalnızca en iyisini, `resolve_all` hepsini (fallback için) döndürür.
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

        // Tüm adayları paralel çözümle, her birinin boyutunu kontrol et
        let found: Vec<(u64, String)> = std::thread::scope(|scope| {
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
            rx.iter().collect()
        });

        if found.is_empty() {
            Err("Bölüm videosu çözülemedi".to_string())
        } else {
            Ok(found)
        }
    }

    /// Direct mp4 URL'si döner — tüm kaynakları paralel çözer, en yüksek kaliteliyi seçer
    pub fn resolve(&self, title_id: u64, episode: u64, season: u64) -> Result<String, String> {
        let mut found = self.resolve_ranked(title_id, episode, season)?;
        found.sort_by(|a, b| b.0.cmp(&a.0));
        found.into_iter().next().map(|(_, url)| url).ok_or_else(|| "Bölüm videosu çözülemedi".to_string())
    }

    /// Tüm çözülmüş kaynakları kalite sırasıyla (en iyi önce) döner — fallback için
    pub fn resolve_all(&self, title_id: u64, episode: u64, season: u64) -> Result<Vec<String>, String> {
        let mut found = self.resolve_ranked(title_id, episode, season)?;
        found.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(found.into_iter().map(|(_, url)| url).collect())
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
        let t0 = std::time::Instant::now();
        let out = self.http.get(url)
            .timeout(std::time::Duration::from_secs(12))
            .send().ok()?.bytes().ok().map(|b| b.to_vec());
        if std::env::var_os("ANIMECIX_BENCH").is_some() {
            let host = url.split('/').nth(2).unwrap_or(url);
            eprintln!("[bench] img {} -> {:.1?}ms", host, t0.elapsed());
        }
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

    /// state.json eski sürümle ya da bozuk/yanlış tipte alanlarla yazılmış
    /// olabilir (örn. maraton öğesinin `title` alanı bir nesne yerine düz bir
    /// id sayısıydı). Bütün dosyayı doğrudan `State`'e deserialize edersek TEK
    /// bir bozuk alan tüm state'i çöpe çıkarır (unwrap_or_default → boş State).
    /// Bu yüzden önce `Value` olarak okuyup bölüm bölüm, alan alan toleranslı
    /// migrate ediyoruz; bir kayıt bozuksa sadece o atlanır, geri kalanı kalır.
    pub fn load_state(&self) -> State {
        let p = Self::state_path();
        let mut st = std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(|v| Self::migrate_state(&v))
            .unwrap_or_default();
        // Eski/eksik kayıtlı başlıkları önbelleklenmiş liste verisiyle tamamla.
        // (isim/tür alanları boşsa maraton, favoriler ve geçmiş sayfaları isimsiz görünür)
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
        // Doldurulan başlık bilgisini diske yaz (bir daha ağ/önbellek araması gerekmesin)
        if changed {
            self.save_state(&st);
        }
        st
    }

    /// Bir JSON değerinden `Title` üretir. Eski şemaları da kapsar:
    /// - nesne: `id`/`title_id`, `name`/`title`, `title_type`/`type`,
    ///   `poster`/`image`, `year`, `season_count`/`seasons`, `description`
    /// - düz sayı: sadece id (hidratasyon ile tamamlanır)
    /// - düz string: sadece isim
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

    /// state.json'ı bölüm bölüm toleranslı şekilde `State`'e dönüştürür.
    /// Tek bir bozuk kayıt tüm state'i çökertmez; sadece o kayıt atlanır.
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

    /// Başlık bilgisi eksikse (isim/tür/yıl/sezon) önce önbelleklenmiş ana liste
    /// verisinden, yoksa ağ üzerinden arama ile tamamlar. Maraton/favoriler/
    /// geçmiş sayfalarının isimsiz görünmesini engeller. Değişiklik yaptıysa true döner.
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
        // Önbellekte yoksa ağ üzerinden id ile ara
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

    /// Önbelleklenmiş (bellek + disk) ana liste verisinden id'ye göre başlığı bulur.
    /// Ağ isteği yapmaz; sadece mevcut önbelleği kullanır.
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

    /// bölüm izlendi olarak kayıtlı mı
    pub fn is_watched(&self, title_id: u64, season: u64, episode: u64) -> bool {
        let st = self.load_state();
        st.watched
            .get(&title_id.to_string())
            .map(|l| l.iter().any(|x| x.episode == episode && x.season == season))
            .unwrap_or(false)
    }

    /// Yalnızca "şu an izlenen" (continue watching) durumunu günceller; izlendi listesine
    /// EKLEMEZ. Böylece bir bölüme geçildiğinde izlenmeden "tamamlandı" işaretlenmez.
    pub fn set_current(&self, w: &Watched) {
        let mut st = self.load_state();
        st.current = Some(Watched {
            title_id: w.title_id,
            episode: w.episode,
            season: w.season,
        });
        self.save_state(&st);
    }

    /// Bir bölümün "izlendi" sayılması için yeterince izlenip izlenmediğini döner.
    /// İlerleme sürenin %90'ına ulaştıysa tamamlandı kabul edilir.
    pub fn played_enough(pos: f64, dur: f64) -> bool {
        dur > 0.0 && (pos / dur) >= 0.9
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
            ..Default::default()
        }
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
        // Zaten genre dolu -> ağ sorgusu yapmadan aynen dönmeli
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
        // Aynı başlık her iki yerde aynı alt satırı üretmeli
        let a = sample("anime", Some(2022), Some(3));
        let b = sample("anime", Some(2022), Some(3));
        assert_eq!(a.meta_line(), b.meta_line());
    }

    #[test]
    fn marathon_persists_full_title_and_meta() {
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
    fn migrate_state_recovers_bare_id_and_alias_fields() {
        // Eski şema: maraton öğesinin `title` alanı düz bir id sayısı, ve favoriler
        // `title` anahtarıyla isim tutuyor olabilir. migrate_state bunları çökertmeden
        // (boş State döndürmeden) kurtarmalı.
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
        let c = Client::new();
        // Sahte "lists" disk önbelleği yaz (ağ gerektirmez, deterministik)
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

        // Boş isimli maraton öğesi kaydet
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

        // temizlik
        let mut st2 = c.load_state();
        st2.marathon.retain(|x| x.title.id != 777);
        c.save_state(&st2);
        let _ = std::fs::remove_file(&lists_path);
    }

    #[test]
    fn next_episode_not_marked_watched_before_playing() {
        // Sonraki bölüme geçiş (set_current) izlenmeden "tamamlandı" işaretlememeli;
        // izlendi işareti yalnızca izlenme eşiği aşılınca konmalı.
        let c = Client::new();
        let tid: u64 = 999_991;
        let w1 = Watched { title_id: tid, episode: 1, season: 0 };
        let w2 = Watched { title_id: tid, episode: 2, season: 0 };

        // Açılışta yalnızca "şu an izlenen" güncellenir
        c.set_current(&w1);
        assert!(!c.is_watched(tid, 0, 1), "açılan bölüm henüz izlenmedi sayılmamalı");

        // İzlenme eşiği politikası
        assert!(Client::played_enough(95.0, 100.0), ">=%90 izlendi sayılmalı");
        assert!(!Client::played_enough(45.0, 100.0), "<%90 izlenmedi sayılmalı");
        assert!(!Client::played_enough(0.0, 0.0), "süresiz içerik izlenmiş sayılmaz");

        // Eşik aşılınca işaretlenir
        c.save_watched(&w1, "");
        assert!(c.is_watched(tid, 0, 1), "eşik aşılınca izlendi sayılmalı");

        // Sonraki bölüme geçiş izlenmeden tamamlamamalı
        c.set_current(&w2);
        assert!(!c.is_watched(tid, 0, 2), "sonraki bölüm geçişte izlenmeden tamamlanmamalı");

        // Sonraki bölüm de izlenince işaretlenir
        c.save_watched(&w2, "");
        assert!(c.is_watched(tid, 0, 2), "sonraki bölüm izlenince işaretlenmeli");

        // temizlik: gerçek state dosyasını kirletmeyelim
        c.remove_watched(tid, 0, 1);
        c.remove_watched(tid, 0, 2);
        let mut st = c.load_state();
        st.current = None;
        c.save_state(&st);
    }
}
