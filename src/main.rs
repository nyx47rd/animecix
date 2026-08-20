mod api;
mod app;
mod covers;
mod player;
mod ui;
mod update;

use app::App;
use gtk::prelude::*;
use std::process::Command;

fn main() {
    // --goto <sayfa> seçeneğini GTK'ın "bilinmeyen seçenek" hatası vermemesi için
    // ayıkla; uygulama içinde std::env::args() ile yine okunur.
    let raw_args: Vec<String> = std::env::args().collect();
    let mut filtered: Vec<String> = Vec::new();
    let mut it = raw_args.iter();
    if let Some(p) = it.next() {
        filtered.push(p.clone());
    }
    while let Some(a) = it.next() {
        if a == "--goto" {
            let _ = it.next(); // değeri atla
        } else if a.starts_with("--goto=") {
            // --goto=deger formu, atla
        } else {
            filtered.push(a.clone());
        }
    }

    let app = adw::Application::builder()
        .application_id("tr.com.animecix")
        .build();

    app.connect_activate(|app| {
        if let Some(display) = gtk::gdk::Display::default() {
            let mut base = std::path::PathBuf::new();
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    base = dir.to_path_buf();
                }
            }
            if let Ok(ad) = std::env::var("APPDIR") {
                if !ad.is_empty() {
                    base = std::path::PathBuf::from(ad);
                }
            }

            let theme = gtk::IconTheme::for_display(&display);
            theme.add_search_path(base.join("assets"));
            theme.add_search_path(base.join("usr/share/icons"));

            let css = gtk::CssProvider::new();
            css.load_from_string(
                r#"/* === FlowBoxChild Temizliği (Çift arka plan engelleme) === */
                flowboxchild {
                    padding: 0;
                    margin: 0;
                    background: none;
                    border: none;
                    outline: none;
                    box-shadow: none;
                }
                flowboxchild:hover, flowboxchild:selected, flowboxchild:focus {
                    background: none;
                    border: none;
                    outline: none;
                    box-shadow: none;
                }

                /* === Kapak Fotoğrafları ve Kesin Boyut Sınırlamaları === */
                .cover {
                    background-color: alpha(currentColor, 0.07);
                    border-radius: 10px;
                    transition: transform 150ms ease, box-shadow 150ms ease;
                }

                .cover-thumb {
                    min-width: 48px !important;
                    max-width: 48px !important;
                    width: 48px !important;
                    min-height: 72px !important;
                    max-height: 72px !important;
                    height: 72px !important;
                }

                .cover-header {
                    min-width: 120px !important;
                    max-width: 120px !important;
                    width: 120px !important;
                    min-height: 180px !important;
                    max-height: 180px !important;
                    height: 180px !important;
                }

                .cover-movie-header {
                    min-width: 160px !important;
                    max-width: 160px !important;
                    width: 160px !important;
                    min-height: 240px !important;
                    max-height: 240px !important;
                    height: 240px !important;
                }

                .cover-shelf {
                    min-width: 140px !important;
                    max-width: 140px !important;
                    width: 140px !important;
                    min-height: 210px !important;
                    max-height: 210px !important;
                    height: 210px !important;
                }

                /* === Kart Hover Animasyonu === */
                .title-btn {
                    padding: 4px;
                    border-radius: 12px;
                    transition: transform 140ms ease, background-color 140ms ease;
                }
                .title-btn:hover {
                    transform: scale(1.04);
                    background-color: alpha(currentColor, 0.08);
                }
                .title-btn:active {
                    transform: scale(0.97);
                }

                /* === Dizi Detay Kartı === */
                .title-detail-card {
                    background-color: alpha(currentColor, 0.04);
                    border: 1px solid alpha(currentColor, 0.08);
                    border-radius: 14px;
                    padding: 14px;
                    transition: background-color 150ms ease;
                }

                /* === Tek Seferlik İpucu Kartı === */
                .tip-banner {
                    background-color: alpha(@accent_color, 0.1);
                    border: 1px solid alpha(@accent_color, 0.28);
                    border-radius: 10px;
                    padding: 10px 14px;
                    margin: 0 12px 8px 12px;
                }
                .tip-banner-text {
                    font-size: 0.9em;
                }

                /* === Kart Başlık Yazısı === */
                .card-title {
                    font-weight: 600;
                    font-size: 0.85em;
                    text-align: center;
                    margin-top: 2px;
                }

                /* === Raf Başlıkları === */
                .shelf-title {
                    font-size: 0.78em;
                    font-weight: 700;
                    letter-spacing: 0.06em;
                    text-transform: uppercase;
                    color: @accent_color;
                    opacity: 0.85;
                }

                /* === HeaderBar Navigasyon Butonları === */
                .header-nav-btn {
                    border-radius: 20px;
                    padding: 4px 10px;
                    font-size: 0.88em;
                    transition: background-color 120ms;
                }
                .header-nav-btn:hover {
                    background-color: alpha(currentColor, 0.1);
                }

                /* === Bookmark Butonu (eski floating) === */
                .lg-icon { -gtk-icon-size: 26px; }
                .bookmark-btn {
                    background-color: alpha(black, 0.55);
                    backdrop-filter: blur(8px);
                    border-radius: 20px;
                    transition: background-color 120ms, transform 120ms;
                }
                .bookmark-btn:hover {
                    background-color: alpha(black, 0.75);
                    transform: scale(1.1);
                }

                /* === Inline Bookmark Butonu (Bölüm sayfası kart içi) === */
                .inline-bookmark-btn {
                    transition: transform 120ms, color 120ms;
                }
                .inline-bookmark-btn:hover {
                    transform: scale(1.15);
                }

                /* === ListBox İçerik Satırları === */
                .content-list > row {
                    border-radius: 6px;
                    transition: background-color 120ms ease, transform 100ms ease;
                }
                .content-list > row:hover {
                    background-color: alpha(currentColor, 0.06);
                }
                .content-list > row:selected {
                    background-color: alpha(@accent_color, 0.15);
                }

                /* === İzleme Maratonu Zengin Tasarım Stilleri === */
                listbox.marathon-list-box {
                    background: transparent;
                }
                listbox.marathon-list-box > row {
                    background: transparent;
                    border: none;
                    padding: 0;
                    margin: 0;
                    box-shadow: none;
                }
                listbox.marathon-list-box > row:hover,
                listbox.marathon-list-box > row:selected,
                listbox.marathon-list-box > row:focus {
                    background: transparent;
                }

                .marathon-summary-card {
                    background-color: alpha(currentColor, 0.04);
                    border: 1px solid alpha(@accent_color, 0.25);
                    border-radius: 16px;
                    padding: 16px 20px;
                    margin-bottom: 14px;
                }

                .marathon-percent-pill {
                    background-color: alpha(@accent_color, 0.18);
                    color: @accent_color;
                    border: 1px solid alpha(@accent_color, 0.35);
                    border-radius: 14px;
                    padding: 4px 12px;
                    font-weight: 700;
                    font-size: 0.9em;
                }

                .marathon-item-card {
                    background-color: alpha(currentColor, 0.03);
                    border: 1px solid alpha(currentColor, 0.07);
                    border-radius: 14px;
                    padding: 12px 16px;
                    margin-bottom: 8px;
                    cursor: grab;
                    transition: background-color 140ms ease, border-color 140ms ease;
                }
                .marathon-item-card:hover {
                    background-color: alpha(currentColor, 0.06);
                    border-color: alpha(@accent_color, 0.3);
                }
                .marathon-item-card:active {
                    cursor: grabbing;
                }
                .marathon-item-card:drop(active) {
                    border-color: @accent_color;
                    background-color: alpha(@accent_color, 0.12);
                }

                .fav-item-card {
                    background-color: alpha(currentColor, 0.03);
                    border: 1px solid alpha(currentColor, 0.07);
                    border-radius: 14px;
                    padding: 12px 16px;
                    margin-bottom: 8px;
                    transition: background-color 140ms ease, border-color 140ms ease;
                }
                .fav-item-card:hover {
                    background-color: alpha(currentColor, 0.06);
                    border-color: alpha(@accent_color, 0.3);
                }

                .marathon-index {
                    min-width: 26px;
                    min-height: 26px;
                    padding: 0 4px;
                    color: @accent_color;
                    font-weight: 700;
                    font-size: 13px;
                }

                .history-item-card {
                    background-color: alpha(currentColor, 0.035);
                    border: 1px solid alpha(currentColor, 0.08);
                    border-radius: 12px;
                    padding: 5px 12px;
                    margin-bottom: 5px;
                    transition: background-color 140ms ease, border-color 140ms ease;
                }
                .history-item-card:hover {
                    background-color: alpha(currentColor, 0.07);
                    border-color: alpha(@accent_color, 0.35);
                }

                .status-badge-completed {
                    background-color: alpha(#2ec27e, 0.18);
                    color: #2ec27e;
                    border: 1px solid alpha(#2ec27e, 0.4);
                    border-radius: 12px;
                    padding: 2px 10px;
                    font-size: 0.82em;
                    font-weight: 600;
                }

                .status-badge-progress {
                    background-color: alpha(@accent_color, 0.18);
                    color: @accent_color;
                    border: 1px solid alpha(@accent_color, 0.35);
                    border-radius: 12px;
                    padding: 2px 10px;
                    font-size: 0.82em;
                    font-weight: 600;
                }

                /* === Durum Renkleri === */
                .success { color: #2ec27e; font-weight: bold; }
                .error   { color: #e01b24; font-weight: bold; }

                /* === Bölüm Progress Bar === */
                .episode-progress {
                    min-height: 6px;
                    border-radius: 3px;
                }
                .episode-progress trough {
                    border-radius: 3px;
                    min-height: 6px;
                    background-color: alpha(currentColor, 0.12);
                }
                .episode-progress progress {
                    border-radius: 3px;
                    background-color: @accent_color;
                }"#,
            );
            gtk::style_context_add_provider_for_display(
                &display,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            // Başlatıcı kurulu ise çalışan yeni binary otomatik güncellenir
            check_and_auto_update_installation();
        }
        let app_inst = App::new(app);
        if app_inst.settings.borrow().auto_update {
            update::check_and_prompt(&app_inst.window, false);
        }
        app_inst.window.present();
    });

    let argv: Vec<&str> = filtered.iter().map(|s| s.as_str()).collect();
    app.run_with_args(&argv);
}

pub struct DepStatus {
    pub name: &'static str,
    pub desc: &'static str,
    pub installed: bool,
    pub install_cmd: Option<String>,
}

pub fn check_all_dependencies() -> Vec<DepStatus> {
    let (distro_name, mpv_cmd) = detect_distro_info();
    let (_, pkg_cmd) = detect_package_manager_cmd();

    let has_mpv = check_command("mpv");
    let has_curl = check_command("curl");
    let has_xdg = check_command("xdg-open");

    vec![
        DepStatus {
            name: "Video Oynatıcı (mpv)",
            desc: "Anime ve filmleri sorunsuz oynatabilmek için gereklidir.",
            installed: has_mpv,
            install_cmd: Some(format!("{mpv_cmd}   # ({distro_name})")),
        },
        DepStatus {
            name: "Sistem Medya İndirici (curl)",
            desc: "Sunuculardan hızlı veri çekmek ve görsel kapakları için gereklidir.",
            installed: has_curl,
            install_cmd: Some(format!("{pkg_cmd} curl")),
        },
        DepStatus {
            name: "Masaüstü Bağlantı Araçları (xdg-utils)",
            desc: "Masaüstü entegrasyonu ve bağlantıları açmak için gereklidir.",
            installed: has_xdg,
            install_cmd: Some(format!("{pkg_cmd} xdg-utils")),
        },
    ]
}

pub fn check_desktop_entry_installed() -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let p1 = std::path::Path::new(&format!("{home}/.local/share/applications/tr.com.animecix.desktop")).exists();
    let p2 = std::path::Path::new(&format!("{home}/.local/share/applications/animecix.desktop")).exists();
    let p3 = std::path::Path::new("/usr/share/applications/tr.com.animecix.desktop").exists();
    let p4 = std::path::Path::new("/usr/share/applications/animecix.desktop").exists();
    p1 || p2 || p3 || p4
}

/// Uygulama her açıldığında: Eğer masaüstü başlatıcısı kurulu ise ve yeni bir AppImage çalıştırılıyorsa
/// arka planda ~/.local/bin/animecix ve .desktop dosyasını en güncel sürüme günceller.
pub fn check_and_auto_update_installation() {
    if check_desktop_entry_installed() {
        let _ = install_desktop_entry();
    }
}

/// Masaüstü kısayolunun `Exec` hedefi:
/// - AppImage olarak çalışıyorsak: AppImage dosyasının yolu (`APPIMAGE` ortam değişkeni).
///   Böylece kısayol AppImage runtime'ını çalıştırır, `APPIMAGE` set olur ve otomatik
///   güncelleme çalışır.
/// - Kaynak derleme ise: `~/.local/bin/animecix`
pub fn desktop_exec_target(home: &str) -> String {
    if let Some(ai) = std::env::var("APPIMAGE").ok().filter(|s| !s.trim().is_empty()) {
        ai
    } else {
        format!("{home}/.local/bin/animecix")
    }
}

pub fn install_desktop_entry() -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME klasörü bulunamadı".to_string())?;

    let exec_target = desktop_exec_target(&home);
    if std::env::var("APPIMAGE").map(|s| !s.trim().is_empty()).unwrap_or(false) {
        // Eski entegrasyondan kalmış yanlış ikili kopyasını temizle (artık kullanılmıyor)
        let _ = std::fs::remove_file(format!("{home}/.local/bin/animecix"));
    } else {
        // Kaynak derleme: ikiliyi ~/.local/bin/animecix'e kopyala
        let bin_dir = format!("{home}/.local/bin");
        let _ = std::fs::create_dir_all(&bin_dir);
        if let Ok(exe_path) = std::env::current_exe() {
            if let Ok(bytes) = std::fs::read(&exe_path) {
                let _ = std::fs::write(&exec_target, &bytes);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&exec_target, std::fs::Permissions::from_mode(0o755));
                }
            }
        }
    }

    let apps_dir = format!("{home}/.local/share/applications");
    let icons_dir = format!("{home}/.local/share/icons/hicolor/256x256/apps");
    let icons_dir_simple = format!("{home}/.local/share/icons");

    let _ = std::fs::create_dir_all(&apps_dir);
    let _ = std::fs::create_dir_all(&icons_dir);
    let _ = std::fs::create_dir_all(&icons_dir_simple);

    let target_icon = format!("{icons_dir}/tr.com.animecix.png");
    let target_icon_simple = format!("{icons_dir_simple}/tr.com.animecix.png");

    if let Ok(content) = std::fs::read("assets/hicolor/256x256/apps/tr.com.animecix.png") {
        let _ = std::fs::write(&target_icon, &content);
        let _ = std::fs::write(&target_icon_simple, &content);
    } else if let Ok(content) = std::fs::read("/usr/share/icons/hicolor/256x256/apps/tr.com.animecix.png") {
        let _ = std::fs::write(&target_icon, &content);
        let _ = std::fs::write(&target_icon_simple, &content);
    }

    let desktop_content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=AnimeciX\n\
         Comment=Türkçe Anime ve Film İzleme İstemcisi\n\
         Exec=\"{exec_target}\"\n\
         Icon=tr.com.animecix\n\
         Terminal=false\n\
         Categories=AudioVideo;Video;Network;\n\
         X-AppImage-Version=0.1.0\n"
    );

    let target_desktop = format!("{apps_dir}/tr.com.animecix.desktop");
    std::fs::write(&target_desktop, desktop_content).map_err(|e| e.to_string())?;

    let _ = std::process::Command::new("update-desktop-database")
        .arg(&apps_dir)
        .output();

    Ok(())
}

pub fn uninstall_application() {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() { return; }

    let bin_path = format!("{home}/.local/bin/animecix");
    let _ = std::fs::remove_file(&bin_path);

    let d1 = format!("{home}/.local/share/applications/tr.com.animecix.desktop");
    let d2 = format!("{home}/.local/share/applications/animecix.desktop");
    let _ = std::fs::remove_file(&d1);
    let _ = std::fs::remove_file(&d2);

    let i1 = format!("{home}/.local/share/icons/hicolor/256x256/apps/tr.com.animecix.png");
    let i2 = format!("{home}/.local/share/icons/hicolor/256x256/apps/animecix.png");
    let i3 = format!("{home}/.local/share/icons/tr.com.animecix.png");
    let i4 = format!("{home}/.local/share/icons/animecix.png");
    let _ = std::fs::remove_file(&i1);
    let _ = std::fs::remove_file(&i2);
    let _ = std::fs::remove_file(&i3);
    let _ = std::fs::remove_file(&i4);

    let apps_dir = format!("{home}/.local/share/applications");
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&apps_dir)
        .output();
}

fn check_command(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn detect_package_manager_cmd() -> (&'static str, &'static str) {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        let content_lower = content.to_lowercase();
        if content_lower.contains("ubuntu") || content_lower.contains("debian") || content_lower.contains("mint") {
            return ("APT", "sudo apt update && sudo apt install -y");
        } else if content_lower.contains("fedora") {
            return ("DNF", "sudo dnf install -y");
        } else if content_lower.contains("arch") || content_lower.contains("manjaro") {
            return ("Pacman", "sudo pacman -S --noconfirm");
        } else if content_lower.contains("suse") {
            return ("Zypper", "sudo zypper install -y");
        }
    }
    ("Paket Yöneticisi", "sudo apt install -y")
}

fn detect_distro_info() -> (&'static str, &'static str) {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        let content_lower = content.to_lowercase();
        if content_lower.contains("ubuntu") || content_lower.contains("debian") || content_lower.contains("mint") {
            return ("Ubuntu / Debian / Mint", "sudo apt update && sudo apt install mpv");
        } else if content_lower.contains("fedora") {
            return ("Fedora", "sudo dnf install mpv");
        } else if content_lower.contains("arch") || content_lower.contains("manjaro") {
            return ("Arch / Manjaro", "sudo pacman -S mpv");
        } else if content_lower.contains("suse") {
            return ("openSUSE", "sudo zypper install mpv");
        }
    }
    ("Linux", "sudo apt install mpv   # ya da dnf/pacman/zypper install mpv")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_exec_target_uses_appimage() {
        std::env::set_var("APPIMAGE", "/opt/AnimeciX-x86_64.AppImage");
        assert_eq!(
            desktop_exec_target("/home/x"),
            "/opt/AnimeciX-x86_64.AppImage"
        );
        std::env::remove_var("APPIMAGE");
    }

    #[test]
    fn desktop_exec_target_falls_back_to_local_bin() {
        std::env::remove_var("APPIMAGE");
        assert_eq!(
            desktop_exec_target("/home/x"),
            "/home/x/.local/bin/animecix"
        );
    }
}