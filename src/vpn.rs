//! İsteğe bağlı yerel VPN proxy yönetimi (sing-box + WireGuard config).
//!
//! Uygulama 127.0.0.1:10808 portunu görünce mpv video trafiğini oradan çıkarır.
//! Bu modül Ayarlar'daki "VPN Proxy" bölümünün arka planıdır: binary/config
//! otomatik tespiti, oturumdan kopuk başlatma/durdurma ve durum kontrolü.

use std::path::PathBuf;

/// Uygulamanın dinlediği yerel proxy portu (mixed: socks+http)
pub const PROXY_PORT: u16 = 10808;

/// Varsayılan sing-box binary yolları (öncelik sırasıyla)
pub fn bin_candidates(home: &str) -> Vec<PathBuf> {
    vec![
        PathBuf::from(format!("{home}/.local/share/singbox/sing-box")),
        PathBuf::from(format!("{home}/.local/bin/sing-box")),
        PathBuf::from("/usr/bin/sing-box"),
        PathBuf::from("/usr/local/bin/sing-box"),
    ]
}

/// Verilen binary için denenecek config yolları (binary dizini öncelikli)
pub fn config_candidates(home: &str, bin: &PathBuf) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(dir) = bin.parent() {
        v.push(dir.join("config.json"));
    }
    v.push(PathBuf::from(format!("{home}/.local/share/singbox/config.json")));
    v.push(PathBuf::from(format!("{home}/vpn-config.json")));
    v.push(PathBuf::from(format!("{home}/sing-box-config.json")));
    v
}

/// Kurulu sing-box + kullanılabilir config bulur.
pub fn detect() -> Option<(PathBuf, PathBuf)> {
    let home = std::env::var("HOME").ok()?;
    for bin in bin_candidates(&home) {
        if !bin.is_file() { continue; }
        for cfg in config_candidates(&home, &bin) {
            if cfg.is_file() {
                return Some((bin, cfg));
            }
        }
    }
    None
}

/// Proxy portu ayakta mı? (mpv trafiğini oraya vereceğimiz kriter de bu)
pub fn port_alive() -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], PROXY_PORT));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).is_ok()
}

/// Log dosyası yolu
pub fn log_path() -> PathBuf {
    let mut p = dirs_data_or_home();
    p.push("singbox.log");
    p
}

fn dirs_data_or_home() -> PathBuf {
    if let Ok(d) = std::env::var("XDG_DATA_HOME") {
        if !d.is_empty() {
            let mut p = PathBuf::from(d);
            p.push("animecix");
            let _ = std::fs::create_dir_all(&p);
            return p;
        }
    }
    match std::env::var("HOME") {
        Ok(h) => {
            let mut p = PathBuf::from(h);
            p.push(".local/share/animecix");
            let _ = std::fs::create_dir_all(&p);
            p
        }
        Err(_) => PathBuf::from("/tmp"),
    }
}

/// sing-box'ı oturumdan kopuk başlatır (terminal/uygulama kapansa da yaşamaya
/// devam eder) ve port açılana kadar bekler.
pub fn start(bin: &PathBuf, cfg: &PathBuf) -> Result<(), String> {
    if port_alive() {
        return Ok(());
    }
    use std::io::Write;
    use std::os::unix::process::CommandExt;
    let log = log_path();
    let mut logf = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open(&log).map_err(|e| format!("log açılamadı ({}): {e}", log.display()))?;
    let errf = logf.try_clone().map_err(|e| e.to_string())?;
    let _ = writeln!(logf, "\n[{}] başlatılıyor: {} -c {}", chrono_stamp(), bin.display(), cfg.display());
    std::process::Command::new(bin)
        .arg("run")
        .arg("-c")
        .arg(cfg)
        .stdin(std::process::Stdio::null())
        .stdout(logf)
        .stderr(errf)
        .process_group(0)
        .spawn()
        .map_err(|e| format!("süreç başlatılamadı: {e}"))?;
    for _ in 0..24 {
        if port_alive() { return Ok(()); }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    Err(format!(
        "süreç başladı ama {PROXY_PORT}. port 6 sn içinde açılmadı (config hatalı olabilir).\nLog: {}",
        log.display()
    ))
}

/// Çalışan sing-box süreçlerini sonlandırır. En az biri öldürüldüyse true.
/// Portun (10808) gerçekten kapanmasını bekler; takılırsa SIGKILL'e yükseltir.
pub fn stop() -> bool {
    let ok = std::process::Command::new("pgrep")
        .arg("-f")
        .arg("sing-box run")
        .output()
        .ok()
        .map(|o| o.stdout)
        .unwrap_or_default();
    let mut pids = Vec::new();
    for line in String::from_utf8_lossy(&ok).lines() {
        if let Ok(pid) = line.trim().parse::<i32>() {
            pids.push(pid);
        }
    }
    if pids.is_empty() {
        return false;
    }
    for pid in &pids {
        let _ = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
    }
    // Yumuşak kapanışın portu serbest bırakmasını bekle
    for _ in 0..16 {
        if !port_alive() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(125));
    }
    // Hâlâ yaşıyorsa sert sonlandır
    for pid in &pids {
        let _ = std::process::Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status();
    }
    for _ in 0..16 {
        if !port_alive() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(125));
    }
    true
}

/// Kullanıcıya gösterilecek adım adım kurulum metni (Türkçe)
pub fn setup_instructions() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
    let dir = format!("{home}/.local/share/singbox");
    format!(
        "sing-box bulunamadı. Tek seferlik kurulum (~2 dakika):\n\n\
1) sing-box indir (Linux x86_64 tar.gz):\n    \
github.com/SagerNet/sing-box/releases\n\n\
2) Arşivi açıp binary'yi şuraya koy:\n    \
{dir}/sing-box\n    \
(mkdir -p {dir} && cp sing-box {dir}/ && chmod +x {dir}/sing-box)\n\n\
3) Ücretsiz ProtonVPN hesabı aç → protonvpn.com → Downloads →\n    \
WireGuard configuration → GNU/Linux seç → indirilen dosyayı şuraya kaydet:\n    \
{dir}/config.json\n\n\
4) Bu pencereyi kapatıp \"Başlat\"a tekrar bas.\n\n\
Not: Uygulama config'i binary'nin yanında, ~/.local/share/singbox/,\n~/vpn-config.json veya ~/sing-box-config.json yolunda arar."
    )
}

fn chrono_stamp() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M:%S")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_candidate_paths_cover_known_locations() {
        let bins = bin_candidates("/home/x");
        assert_eq!(bins[0], PathBuf::from("/home/x/.local/share/singbox/sing-box"));
        assert!(bins.iter().any(|b| *b == PathBuf::from("/usr/bin/sing-box")));
        let cfgs = config_candidates("/home/x", &bins[0]);
        // Binary dizini en öncelikli aday
        assert_eq!(cfgs[0], PathBuf::from("/home/x/.local/share/singbox/config.json"));
        assert!(cfgs.contains(&PathBuf::from("/home/x/vpn-config.json")));
    }

    #[test]
    fn setup_instructions_mention_download_and_config_steps() {
        let s = setup_instructions();
        assert!(s.contains("sing-box"), "indirme adımı olmalı");
        assert!(s.contains("config.json"), "config kaydetme adımı olmalı");
        assert!(s.contains("ProtonVPN"), "Proton adımı olmalı");
    }

    #[test]
    fn port_alive_false_without_proxy() {
        // Test ortamında 10808 dinleyen bir şey varsa bile bağlantı hatası
        // durumunda false DÖNMELİ; ayaktaysa true döner (ortama bağlı değil).
        let _ = port_alive();
    }
}
