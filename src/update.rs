use std::path::Path;
use std::sync::mpsc::channel;

use glib::ControlFlow;
use gtk::prelude::*;
use adw::prelude::*;
use serde::Deserialize;

const UPDATE_REPO: &str = "nyx47rd/animecix-app";
const ASSET_NAME: &str = "AnimeciX-x86_64.AppImage";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

/// Uygulama bir AppImage olarak mı çalışıyor? (`APPIMAGE` ortam değişkeni AppImage
/// runtime'ı tarafından ayarlanır; kaynaktan derlenen binary'de yoktur.)
pub fn is_appimage() -> bool {
    std::env::var("APPIMAGE").map(|v| !v.trim().is_empty()).unwrap_or(false)
}

/// Semver-benzeri karşılaştırma: `latest`, `current`'dan yeni ise true döner.
pub fn needs_update(current: &str, latest: &str) -> bool {
    fn parse(v: &str) -> Vec<u32> {
        v.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect()
    }
    let c = parse(current);
    let l = parse(latest);
    let n = c.len().max(l.len());
    for i in 0..n {
        let cv = c.get(i).copied().unwrap_or(0);
        let lv = l.get(i).copied().unwrap_or(0);
        if lv > cv {
            return true;
        }
        if lv < cv {
            return false;
        }
    }
    false
}

/// GitHub release JSON'ından (tag, asset url) çiftini çıkarır.
pub fn parse_release_json(json: &str) -> Option<(String, String)> {
    let rel: GithubRelease = serde_json::from_str(json).ok()?;
    let url = rel
        .assets
        .iter()
        .find(|a| a.name == ASSET_NAME)
        .map(|a| a.browser_download_url.clone())?;
    Some((rel.tag_name, url))
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("animecix-updater")
        .build()
        .map_err(|e| e.to_string())
}

fn latest_release() -> Result<GithubRelease, String> {
    let url = format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest");
    let client = http_client()?;
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API hatası: {}", resp.status()));
    }
    resp.json().map_err(|e| e.to_string())
}

/// İndirilen baytları geçici dosyaya yazar, çalıştırma izni verir ve hedefin üstüne taşır.
pub fn replace_target(bytes: &[u8], target: &Path) -> Result<(), String> {
    if bytes.len() < 4 || &bytes[0..4] != b"\x7fELF" {
        return Err("İndirilen dosya geçerli bir AppImage değil".into());
    }
    let tmp = std::env::temp_dir()
        .join(format!("AnimeciX-update-{}.AppImage", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp, perms).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, target).map_err(|e| e.to_string())?;
    Ok(())
}

/// Güncelleme varsa onay dialogu gösterir. `show_if_uptodate` true ise güncelken de bilgi verir.
/// `on_suppress_uptodate`, güncel sürüm bildirimi "Bir Daha Gösterme" ile kapatıldığında çağrılır.
pub fn check_and_prompt<W: IsA<gtk::Window>>(
    window: &W,
    show_if_uptodate: bool,
    on_suppress_uptodate: impl Fn() + 'static,
) {
    if !is_appimage() {
        if show_if_uptodate {
            present_info(window, "Güncelleme Kullanılamıyor", "Otomatik güncelleme yalnızca AppImage sürümünde çalışır.");
        }
        return;
    }
    // Ağ işini ayrı thread'de yap (widget'lar Send değildir); sonucu kanaldan ana thread'e taşı.
    let (tx, rx) = channel::<Result<Option<(String, String)>, String>>();
    std::thread::spawn(move || {
        let res = (|| -> Result<Option<(String, String)>, String> {
            let rel = latest_release()?;
            if !needs_update(CURRENT_VERSION, &rel.tag_name) {
                return Ok(None);
            }
            let url = rel
                .assets
                .iter()
                .find(|a| a.name == ASSET_NAME)
                .map(|a| a.browser_download_url.clone())
                .ok_or_else(|| "asset bulunamadı".to_string())?;
            Ok(Some((rel.tag_name, url)))
        })();
        let _ = tx.send(res);
    });

    // Ana thread: kanalı poll et, sonuç gelince dialog'u oluştur (widget yalnızca burada).
    let win = window.clone();
    let mut on_suppress = Some(on_suppress_uptodate);
    glib::idle_add_local(move || match rx.try_recv() {
        Ok(Ok(Some((tag, url)))) => {
            present_update_dialog(&win, &tag, &url);
            ControlFlow::Break
        }
        Ok(Ok(None)) => {
            if show_if_uptodate {
                if let Some(f) = on_suppress.take() {
                    present_uptodate(&win, f);
                }
            }
            ControlFlow::Break
        }
        Ok(Err(e)) => {
            eprintln!("Güncelleme kontrolü başarısız: {e}");
            ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => ControlFlow::Break,
    });
}

fn present_info<W: IsA<gtk::Window>>(window: &W, heading: &str, body: &str) {
    let dialog = adw::MessageDialog::builder()
        .heading(heading)
        .body(body)
        .build();
    dialog.set_transient_for(Some(window));
    dialog.present();
}

/// Güncel sürüm bildirimi: "Tamam" ile kapanır; "Bir Daha Gösterme" ile
/// `on_suppress` çağrılır (ayarlarda `notify_uptodate` kapatılır).
fn present_uptodate<W: IsA<gtk::Window>>(window: &W, on_suppress: impl Fn() + 'static) {
    let dialog = adw::MessageDialog::builder()
        .heading("Güncel")
        .body("AnimeciX güncel sürümde 🎉")
        .close_response("ok")
        .default_response("ok")
        .build();
    dialog.add_response("suppress", "Bir Daha Gösterme");
    dialog.add_response("ok", "Tamam");
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_transient_for(Some(window));
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "suppress" {
            on_suppress();
        }
        dlg.close();
    });
    dialog.present();
}

fn present_update_dialog<W: IsA<gtk::Window>>(window: &W, tag: &str, url: &str) {
    let tag = tag.trim_start_matches('v').to_string();
    let url = url.to_string();
    let target = std::env::var("APPIMAGE").unwrap_or_default();
    let dialog = adw::MessageDialog::builder()
        .heading("Yeni Sürüm Mevcut")
        .body(format!(
            "AnimeciX v{tag} yayınlandı. İndirip uygulamayı güncelleyelim mi?"
        ))
        .close_response("later")
        .default_response("later")
        .build();
    dialog.add_response("later", "Daha Sonra");
    dialog.add_response("update", "Güncelle ve Yeniden Başlat");
    dialog.set_response_appearance("update", adw::ResponseAppearance::Suggested);
    dialog.set_transient_for(Some(window));
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "update" {
            dlg.close();
            install_and_restart(&url, &target);
        }
    });
    dialog.present();
}

fn install_and_restart(url: &str, target: &str) {
    if target.is_empty() {
        return;
    }
    let url = url.to_string();
    let target = std::path::PathBuf::from(target);
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let client = http_client()?;
            let bytes = client
                .get(&url)
                .send()
                .map_err(|e| e.to_string())?
                .bytes()
                .map_err(|e| e.to_string())?;
            replace_target(&bytes, &target)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                let _ = std::process::Command::new(&target).spawn();
                std::process::exit(0);
            }
            Err(e) => eprintln!("Güncelleme başarısız: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(needs_update("3.0.0", "3.1.0"));
        assert!(needs_update("3.0.0", "4.0.0"));
        assert!(needs_update("3.0.9", "3.1.0"));
        assert!(!needs_update("3.1.0", "3.0.0"));
        assert!(!needs_update("3.0.0", "3.0.0"));
        assert!(!needs_update("v3.0.0", "3.0.0"));
        assert!(needs_update("3.0.0", "v3.0.1"));
    }

    #[test]
    fn parse_asset() {
        let json = r#"{"tag_name":"v3.1.0","assets":[{"name":"other.txt","browser_download_url":"http://x/other"},{"name":"AnimeciX-x86_64.AppImage","browser_download_url":"http://x/AnimeciX-x86_64.AppImage"}]}"#;
        let (tag, url) = parse_release_json(json).unwrap();
        assert_eq!(tag, "v3.1.0");
        assert_eq!(url, "http://x/AnimeciX-x86_64.AppImage");
        assert!(parse_release_json(r#"{"tag_name":"v1","assets":[]}"#).is_none());
    }

    #[test]
    fn replace_writes_and_renames() {
        let tmp = std::env::temp_dir().join(format!("animecix-replace-test-{}", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let elf = b"\x7fELFfakeappimage";
        replace_target(elf, &tmp).unwrap();
        assert!(tmp.exists());
        let content = std::fs::read(&tmp).unwrap();
        assert_eq!(&content[0..4], b"\x7fELF");
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn replace_rejects_non_elf() {
        let tmp = std::env::temp_dir().join(format!("animecix-replace-bad-{}", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        assert!(replace_target(b"not an appimage", &tmp).is_err());
        std::fs::remove_file(&tmp).ok();
    }
}
