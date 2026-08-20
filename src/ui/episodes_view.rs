use gtk::prelude::*;
use crate::api::Title;

/// Detay başlık kartı: Poster, Başlık, Yıl, Açıklama, Sezon Bilgisi ve Favori Butonu
pub fn create_title_detail_header(
    title: &Title,
    poster_widget: &gtk::Picture,
    bookmark_btn: &gtk::Button,
    marathon_btn: &gtk::Button,
) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    card.add_css_class("card");
    card.add_css_class("title-detail-card");
    card.set_margin_top(8);
    card.set_margin_bottom(12);
    card.set_margin_start(12);
    card.set_margin_end(12);
    card.append(poster_widget);

    let info_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    info_box.set_hexpand(true);
    info_box.set_valign(gtk::Align::Center);

    // Başlık + Favori + Maraton butonu üst satırı
    let name_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name_lbl = gtk::Label::new(Some(&title.name));
    name_lbl.add_css_class("title-1");
    name_lbl.set_xalign(0.0);
    name_lbl.set_wrap(true);
    name_lbl.set_hexpand(true);

    name_row.append(&name_lbl);
    name_row.append(bookmark_btn);
    name_row.append(marathon_btn);
    info_box.append(&name_row);

    // Yıl, Tür, Sezon sayısı rozet satırı
    let meta_parts: Vec<String> = [
        title.year.map(|y| y.to_string()),
        title.title_type.as_deref().map(|t| match t {
            "anime" => "Anime Serisi",
            "movie" => "Film",
            _ => "Dizi",
        }.to_string()),
        title.season_count.map(|s| format!("{s} Sezon")),
    ].into_iter().flatten().collect();

    if !meta_parts.is_empty() {
        let meta_lbl = gtk::Label::new(Some(&meta_parts.join("  •  ")));
        meta_lbl.add_css_class("dim-label");
        meta_lbl.add_css_class("title-4");
        meta_lbl.set_xalign(0.0);
        info_box.append(&meta_lbl);
    }

    // Açıklama paragrafı
    if let Some(desc) = &title.description {
        let clean_desc = desc.trim();
        if !clean_desc.is_empty() {
            let desc_lbl = gtk::Label::new(Some(clean_desc));
            desc_lbl.add_css_class("dim-label");
            desc_lbl.set_xalign(0.0);
            desc_lbl.set_wrap(true);
            desc_lbl.set_max_width_chars(60);
            desc_lbl.set_lines(3);
            desc_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
            info_box.append(&desc_lbl);
        }
    }

    card.append(&info_box);
    card
}

/// Tek seferlik bilgilendirme kartı (Kısayol ipucu)
pub fn create_quick_search_tip_banner(shortcut_str: &str, on_dismiss: impl Fn() + 'static) -> gtk::Box {
    let banner = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    banner.add_css_class("tip-banner");

    let icon = gtk::Image::from_icon_name("dialog-information-symbolic");
    icon.set_icon_size(gtk::IconSize::Normal);
    icon.set_valign(gtk::Align::Center);

    let text = gtk::Label::new(Some(&format!(
        "Klavyeden '{}' kısayoluna basarak bölüm listesinde hızlıca arama yapabilirsiniz.",
        shortcut_str
    )));
    text.add_css_class("tip-banner-text");
    text.set_xalign(0.0);
    text.set_wrap(true);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);

    let dismiss_btn = gtk::Button::with_label("Anladım");
    dismiss_btn.add_css_class("flat");
    dismiss_btn.add_css_class("pill");
    dismiss_btn.set_valign(gtk::Align::Center);
    let banner_clone = banner.clone();
    dismiss_btn.connect_clicked(move |_| {
        banner_clone.set_visible(false);
        on_dismiss();
    });

    banner.append(&icon);
    banner.append(&text);
    banner.append(&dismiss_btn);
    banner
}

/// Tek seferlik bilgilendirme kartı (Sağ Tık İzlendi ipucu)
pub fn create_right_click_tip_banner(on_dismiss: impl Fn() + 'static) -> gtk::Box {
    let banner = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    banner.add_css_class("tip-banner");

    let icon = gtk::Image::from_icon_name("input-mouse-symbolic");
    icon.set_icon_size(gtk::IconSize::Normal);
    icon.set_valign(gtk::Align::Center);

    let text = gtk::Label::new(Some(
        "Bir bölüme sağ tıklayarak o bölümü izlendi / izlenmedi olarak manuel işaretleyebilirsiniz.",
    ));
    text.add_css_class("tip-banner-text");
    text.set_xalign(0.0);
    text.set_wrap(true);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);

    let dismiss_btn = gtk::Button::with_label("Anladım");
    dismiss_btn.add_css_class("flat");
    dismiss_btn.add_css_class("pill");
    dismiss_btn.set_valign(gtk::Align::Center);
    let banner_clone = banner.clone();
    dismiss_btn.connect_clicked(move |_| {
        banner_clone.set_visible(false);
        on_dismiss();
    });

    banner.append(&icon);
    banner.append(&text);
    banner.append(&dismiss_btn);
    banner
}

/// Özel Film Detay Görünümü: Büyük "Filmi İzle 🎬" butonu ve film bilgileri
/// Returns (view, progress_bar, time_label) — progress_bar ve time_label
/// caller tarafından progress_bars HashMap'ine kaydedilmeli ki live güncellenebilsin.
pub fn create_movie_detail_view(
    title: &Title,
    poster_widget: &gtk::Picture,
    bookmark_btn: &gtk::Button,
    marathon_btn: &gtk::Button,
    progress: Option<(f64, f64)>,
    on_play: impl Fn() + 'static,
) -> (gtk::Box, gtk::ProgressBar, gtk::Label) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
    root.set_margin_top(16);
    root.set_margin_bottom(24);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let card = gtk::Box::new(gtk::Orientation::Horizontal, 20);
    card.add_css_class("card");
    card.add_css_class("title-detail-card");
    card.append(poster_widget);

    let info_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
    info_box.set_hexpand(true);
    info_box.set_valign(gtk::Align::Center);

    let name_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name_lbl = gtk::Label::new(Some(&title.name));
    name_lbl.add_css_class("title-1");
    name_lbl.set_xalign(0.0);
    name_lbl.set_wrap(true);
    name_lbl.set_hexpand(true);

    name_row.append(&name_lbl);
    name_row.append(bookmark_btn);
    name_row.append(marathon_btn);
    info_box.append(&name_row);

    let meta_parts: Vec<String> = [
        title.year.map(|y| y.to_string()),
        Some("Film 🎬".to_string()),
    ].into_iter().flatten().collect();

    let meta_lbl = gtk::Label::new(Some(&meta_parts.join("  •  ")));
    meta_lbl.add_css_class("dim-label");
    meta_lbl.add_css_class("title-4");
    meta_lbl.set_xalign(0.0);
    info_box.append(&meta_lbl);

    if let Some(desc) = &title.description {
        let clean_desc = desc.trim();
        if !clean_desc.is_empty() {
            let desc_lbl = gtk::Label::new(Some(clean_desc));
            desc_lbl.add_css_class("dim-label");
            desc_lbl.set_xalign(0.0);
            desc_lbl.set_wrap(true);
            desc_lbl.set_max_width_chars(60);
            desc_lbl.set_lines(5);
            desc_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
            info_box.append(&desc_lbl);
        }
    }

    let fmt_t = |s: f64| -> String {
        let s = s as u64;
        let h = s / 3600;
        let m = (s % 3600) / 60;
        let sec = s % 60;
        if h > 0 { format!("{h}:{:02}:{:02}", m, sec) }
        else { format!("{m}:{:02}", sec) }
    };

    // Her zaman progress bar ve label oluştur (ilk başta gizli olabilir)
    let pb = gtk::ProgressBar::new();
    pb.add_css_class("episode-progress");
    pb.set_margin_top(6);
    let lbl = gtk::Label::new(None);
    lbl.add_css_class("dim-label");
    lbl.set_xalign(0.0);

    if let Some((pos, dur)) = progress {
        if dur > 0.0 {
            pb.set_fraction((pos / dur).clamp(0.0, 1.0));
            lbl.set_text(&format!("İzlendi: {} / {}", fmt_t(pos), fmt_t(dur)));
            lbl.set_visible(true);
        } else {
            pb.set_visible(false);
            lbl.set_visible(false);
        }
    } else {
        pb.set_visible(false);
        lbl.set_visible(false);
    }

    info_box.append(&pb);
    info_box.append(&lbl);

    let play_btn = gtk::Button::with_label("Filmi İzle 🎬");
    play_btn.add_css_class("suggested-action");
    play_btn.add_css_class("pill");
    play_btn.add_css_class("title-3");
    play_btn.set_halign(gtk::Align::Start);
    play_btn.set_margin_top(8);
    play_btn.connect_clicked(move |_| {
        on_play();
    });
    info_box.append(&play_btn);

    card.append(&info_box);
    root.append(&card);
    (root, pb, lbl)
}
