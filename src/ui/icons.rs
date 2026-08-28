use gtk::prelude::*;
use lucide_icons::Icon;

pub fn install_lucide_font() {
    let provider = gtk::CssProvider::new();
    let font_path = locate_font_path();
    if let Some(p) = font_path {
        let css = format!(
            "@font-face {{ font-family: 'Lucide'; src: url('file://{}'); }}\n",
            p
        );
        provider.load_from_string(&css);
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
    }
}

fn locate_font_path() -> Option<String> {
    if let Some(p) = std::env::var_os("ANIMECIX_LUCIDE_FONT") {
        return Some(p.to_string_lossy().to_string());
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for cand in [
                dir.join("usr/share/animecix/assets/lucide/lucide.ttf"),
                dir.join("usr/share/animecix/assets/lucide.ttf"),
                dir.join("assets/lucide/lucide.ttf"),
                dir.join("../share/animecix/assets/lucide/lucide.ttf"),
                dir.join("lucide.ttf"),
            ] {
                if cand.exists() {
                    return Some(cand.to_string_lossy().to_string());
                }
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        for cand in [
            format!("{home}/.local/share/animecix/assets/lucide/lucide.ttf"),
            format!("{home}/.local/share/animecix/assets/lucide.ttf"),
        ] {
            if std::path::Path::new(&cand).exists() {
                return Some(cand);
            }
        }
    }
    for cand in [
        "/usr/share/animecix/assets/lucide/lucide.ttf",
        "/app/share/animecix/assets/lucide/lucide.ttf",
    ] {
        if std::path::Path::new(cand).exists() {
            return Some(cand.to_string());
        }
    }
    None
}

pub fn lucide_label(icon: Icon, size: i32) -> gtk::Label {
    let l = gtk::Label::new(Some(&icon.unicode().to_string()));
    l.set_xalign(0.5);
    l.set_yalign(0.5);
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_string(&format!(
        "label {{ font-family: 'Lucide'; font-size: {size}px; }}"
    ));
    l.style_context().add_provider(&css_provider, gtk::STYLE_PROVIDER_PRIORITY_USER);
    l
}

pub fn lucide_button(icon: Icon, label: Option<&str>, size: i32) -> gtk::Button {
    let h = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    h.set_halign(gtk::Align::Center);
    h.set_valign(gtk::Align::Center);
    h.set_margin_start(4);
    h.set_margin_end(4);
    h.set_margin_top(2);
    h.set_margin_bottom(2);
    let ico = lucide_label(icon, size);
    h.append(&ico);
    if let Some(text) = label {
        let l = gtk::Label::with_mnemonic(text);
        h.append(&l);
    }
    let b = gtk::Button::new();
    b.set_child(Some(&h));
    b
}
