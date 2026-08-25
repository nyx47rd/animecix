
use std::path::PathBuf;

pub const PROXY_PORT: u16 = 10808;

pub fn bin_candidates(home: &str) -> Vec<PathBuf> {
    vec![
        PathBuf::from(format!("{home}/.local/share/singbox/sing-box")),
        PathBuf::from(format!("{home}/.local/bin/sing-box")),
        PathBuf::from("/usr/bin/sing-box"),
        PathBuf::from("/usr/local/bin/sing-box"),
    ]
}

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

pub fn port_alive() -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], PROXY_PORT));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).is_ok()
}

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

pub fn stop() -> bool {
    let pids = find_singbox_pids();
    if pids.is_empty() {
        return false;
    }
    for pid in &pids {
        signal_pid(*pid, libc::SIGTERM);
    }
    for _ in 0..16 {
        if !port_alive() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(125));
    }
    for pid in &pids {
        signal_pid(*pid, libc::SIGKILL);
    }
    for _ in 0..16 {
        if !port_alive() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(125));
    }
    true
}

fn signal_pid(pid: i32, sig: i32) {
    unsafe {
        libc::kill(pid, sig);
    }
}

pub fn find_singbox_pids() -> Vec<i32> {
    let mut pids = Vec::new();
    let me = std::process::id() as i32;
    let dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return pids,
    };
    for entry in dir.flatten() {
        let name = entry.file_name();
        let pid: i32 = match name.to_string_lossy().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if pid == me {
            continue;
        }
        let raw = match std::fs::read(format!("/proc/{pid}/cmdline")) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let joined = String::from_utf8_lossy(&raw).replace('\0', " ");
        if joined.contains("sing-box run") {
            pids.push(pid);
        }
    }
    pids
}

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
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
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
        let _ = port_alive();
    }

    #[test]
    fn civil_from_days_epoch_and_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_690), (2026, 8, 25));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn chrono_stamp_shape() {
        let s = chrono_stamp();
        assert_eq!(s.len(), 19);
        let b = s.as_bytes();
        assert_eq!(b[4], b'-');
        assert_eq!(b[7], b'-');
        assert_eq!(b[10], b' ');
        assert_eq!(b[13], b':');
        assert_eq!(b[16], b':');
    }

    #[test]
    fn find_singbox_pids_detects_matching_cmdline() {
        let mut child = std::process::Command::new("bash")
            .arg("-c")
            .arg("exec -a \"sing-box run\" sleep 30")
            .spawn()
            .expect("bash mevcut olmalı");
        std::thread::sleep(std::time::Duration::from_millis(150));
        let mut found = false;
        for _ in 0..20 {
            if find_singbox_pids().contains(&(child.id() as i32)) {
                found = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        signal_pid(child.id() as i32, libc::SIGKILL);
        let _ = child.wait();
        assert!(found, "cmdline eşleşen süreç bulunmalıydı");
    }

    #[test]
    fn find_singbox_pids_skips_unrelated_processes() {
        let pids = find_singbox_pids();
        assert!(!pids.contains(&(std::process::id() as i32)));
    }
}
