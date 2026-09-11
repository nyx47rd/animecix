//! Marka ikonları: gömülü SVG + sistem teması yedeği.
//! Repo-yolu dosya okuma AppImage'ta çalışmaz; baytlar binary'ye gömülüdür
//! (font.rs / covers.rs deseni).

use gtk::prelude::*;

const GITHUB_MARK_SVG: &[u8] = include_bytes!("../../assets/icons/github-mark-white.svg");

/// SVG baytlarından boyanabilir doku üretir.
pub fn texture_from_svg(bytes: &[u8]) -> Option<gtk::gdk::Texture> {
    let loader = gdk_pixbuf::PixbufLoader::new();
    loader.write(bytes).ok()?;
    loader.close().ok()?;
    let pb = loader.pixbuf()?;
    Some(gtk::gdk::Texture::for_pixbuf(&pb))
}

/// GitHub işareti: gömülü beyaz mark (5 tema da koyu; sistem ikonuna bel bağlanmaz).
pub fn github_paintable() -> Option<gtk::gdk::Paintable> {
    texture_from_svg(GITHUB_MARK_SVG).map(|t| t.upcast::<gtk::gdk::Paintable>())
}

/// Satır öneki için hazır Image (bozuk-ikon yerine her zaman bir şey döner).
pub fn github_image(pixel: i32) -> gtk::Image {
    let img = gtk::Image::new();
    img.set_pixel_size(pixel);
    if let Some(p) = github_paintable() {
        img.set_paintable(Some(&p));
    }
    img.set_valign(gtk::Align::Center);
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_svg_embeds_and_parses() {
        assert!(!GITHUB_MARK_SVG.is_empty(), "svg gömülü olmalı");
        let s = std::str::from_utf8(GITHUB_MARK_SVG).expect("utf8");
        assert!(s.contains("<svg"), "svg kökü: {s}");
        assert!(s.contains("<path"), "mark yolu yok");
    }
}
