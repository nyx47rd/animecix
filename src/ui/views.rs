use gtk::prelude::*;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use crate::api::{Client, HistoryEntry, Settings, Title};
use crate::ui::episodes_view;

/// İzleme Maratonu (To-Do List) Sayfası
pub struct MarathonView;

impl MarathonView {
    pub fn build(
        client: std::sync::Arc<Client>,
        on_item_click: impl Fn(Title) + 'static,
        on_toggle_completed: impl Fn(u64) + 'static,
        on_remove_item: impl Fn(u64) + 'static,
        on_clear_all: impl Fn() + 'static,
        on_reorder: impl Fn(u64, usize) + 'static,
        cover_loader: impl Fn(Option<&str>, &gtk::Picture, i32, i32) + 'static,
    ) -> gtk::Box {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(16);
        root.set_margin_end(16);

        let marathon_items = client.get_marathon();
        if marathon_items.is_empty() {
            let sp = crate::ui::components::create_status_page(
                "İzleme Maratonunuz Boş 🏃‍♂️",
                "Gelecekte izleyeceğiniz anime veya dizileri detay sayfasındaki maraton butonuna (🏁) tıklayarak ekleyebilirsiniz.",
                "media-playlist-repeat-symbolic",
            );
            root.append(&sp);
            return root;
        }

        let total_count = marathon_items.len();
        let completed_count = marathon_items.iter().filter(|m| m.completed).count();
        let percent = if total_count > 0 { (completed_count * 100) / total_count } else { 0 };

        // --- İlerleme ve Özet Kartı ---
        let summary_card = gtk::Box::new(gtk::Orientation::Vertical, 10);
        summary_card.add_css_class("marathon-summary-card");

        let top_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let header_lbl = gtk::Label::new(Some("🏃‍♂️ İzleme Maratonu İlerlemesi"));
        header_lbl.add_css_class("title-2");
        header_lbl.set_xalign(0.0);
        header_lbl.set_hexpand(true);

        let clear_btn = gtk::Button::with_label("Tümünü Temizle");
        clear_btn.add_css_class("destructive-action");
        clear_btn.add_css_class("pill");
        let on_clear_all_rc = Rc::new(on_clear_all);
        let on_clear_c = on_clear_all_rc.clone();
        clear_btn.connect_clicked(move |_| on_clear_c());

        top_row.append(&header_lbl);
        top_row.append(&clear_btn);

        let stats_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let stats_lbl = gtk::Label::new(Some(&format!(
            "{} / {} Anime Tamamlandı", completed_count, total_count
        )));
        stats_lbl.add_css_class("title-4");
        stats_lbl.add_css_class("dim-label");
        stats_lbl.set_xalign(0.0);
        stats_lbl.set_hexpand(true);

        let percent_pill = gtk::Label::new(Some(&format!("%{}", percent)));
        percent_pill.add_css_class("marathon-percent-pill");

        stats_row.append(&stats_lbl);
        stats_row.append(&percent_pill);

        let pbar = gtk::ProgressBar::new();
        pbar.add_css_class("episode-progress");
        pbar.set_fraction((completed_count as f64 / total_count as f64).clamp(0.0, 1.0));

        summary_card.append(&top_row);
        summary_card.append(&stats_row);
        summary_card.append(&pbar);

        root.append(&summary_card);

        // --- Liste ---
        let list_box = gtk::Box::new(gtk::Orientation::Vertical, 8);

        let on_item_click_rc = Rc::new(on_item_click);
        let on_toggle_rc = Rc::new(on_toggle_completed);
        let on_remove_rc = Rc::new(on_remove_item);
        let on_reorder_rc = Rc::new(on_reorder);
        let cover_loader_rc = Rc::new(cover_loader);

        for (idx, item) in marathon_items.iter().enumerate() {
            let card_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            card_box.add_css_class("marathon-item-card");

            // Sıra numarası
            let num = gtk::Label::new(Some(&format!("{}", idx + 1)));
            num.add_css_class("marathon-index");
            num.set_xalign(0.5);
            num.set_yalign(0.5);
            num.set_valign(gtk::Align::Center);
            card_box.append(&num);

            // --- Sürükle-bırak ile sıralama (kartın tamamı sürüklenebilir) ---
            let drag = gtk::DragSource::new();
            drag.set_actions(gtk::gdk::DragAction::MOVE);
            // Sürükleme ikonu: kartın kendisi, hotspot kartın MERKEZİNDE.
            // Böylece kart nereden tutulursa tutulsun imleç kartın ortasında kalır.
            let card_for_icon = card_box.clone();
            let src_id = item.title.id;
            drag.connect_prepare(move |drag, _, _| {
                let alloc = card_for_icon.allocation();
                let paintable = gtk::WidgetPaintable::new(Some(&card_for_icon));
                drag.set_icon(Some(&paintable), alloc.width() / 2, alloc.height() / 2);
                Some(gtk::gdk::ContentProvider::for_value(
                    &glib::Value::from(src_id.to_string()),
                ))
            });
            // Sürüklerken kaynak kartın opaklığını düşür (yerinde görsel ipucu)
            let card_dim = card_box.clone();
            drag.connect_drag_begin(move |_, _| {
                card_dim.set_opacity(0.35);
            });
            let card_restore = card_box.clone();
            drag.connect_drag_end(move |_, _, _| {
                card_restore.set_opacity(1.0);
            });
            card_box.add_controller(drag);

            let drop = gtk::DropTarget::new(glib::Type::STRING, gtk::gdk::DragAction::MOVE);
            let self_id = item.title.id;
            let on_r = on_reorder_rc.clone();
            drop.connect_drop(move |_, value, _, _| {
                if let Ok(s) = value.get::<String>() {
                    if let Ok(src_id) = s.parse::<u64>() {
                        if src_id != self_id {
                            on_r(src_id, idx);
                        }
                    }
                }
                true
            });
            card_box.add_controller(drop);

            // Checkbox
            let chk = gtk::CheckButton::new();
            chk.set_active(item.completed);
            chk.set_valign(gtk::Align::Center);
            chk.set_tooltip_text(Some(if item.completed { "Tamamlandı olarak işaretli" } else { "Tamamlandı olarak işaretle" }));
            let tid = item.title.id;
            let on_t_c = on_toggle_rc.clone();
            chk.connect_toggled(move |_| { on_t_c(tid); });

            // Poster
            let pic = gtk::Picture::new();
            pic.set_width_request(48);
            pic.set_height_request(72);
            pic.set_hexpand(false);
            pic.set_vexpand(false);
            pic.set_can_shrink(true);
            pic.set_content_fit(gtk::ContentFit::Cover);
            pic.set_css_classes(&["cover", "cover-thumb"]);
            pic.set_valign(gtk::Align::Center);
            cover_loader_rc(item.title.poster.as_deref(), &pic, 48, 72);

            // Metin
            let info_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
            info_box.set_valign(gtk::Align::Center);
            info_box.set_hexpand(true);
            let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let name_lbl = gtk::Label::new(Some(&item.title.name));
            name_lbl.add_css_class("title-3");
            name_lbl.set_xalign(0.0);
            name_lbl.set_wrap(false);
            name_lbl.set_single_line_mode(true);
            name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
            if item.completed { name_lbl.add_css_class("dim-label"); }
            let status_badge = gtk::Label::new(Some(if item.completed { "🏁 Tamamlandı" } else { "⏳ Devam Ediyor" }));
            status_badge.add_css_class(if item.completed { "status-badge-completed" } else { "status-badge-progress" });
            title_row.append(&name_lbl);
            title_row.append(&status_badge);
            info_box.append(&title_row);
            episodes_view::append_title_submeta(&info_box, &item.title);

            // İlerleme çubuğu
            let prog = gtk::ProgressBar::new();
            prog.add_css_class("episode-progress");
            prog.set_margin_top(4);
            prog.set_valign(gtk::Align::Center);
            info_box.append(&prog);
            let client_c = client.clone();
            let t_c = item.title.clone();
            let (tx, rx) = std::sync::mpsc::channel::<f64>();
            std::thread::spawn(move || { let _ = tx.send(client_c.title_progress_frac(&t_c)); });
            glib::idle_add_local(move || match rx.try_recv() {
                Ok(frac) => { prog.set_fraction(frac); glib::ControlFlow::Break }
                Err(_)   => glib::ControlFlow::Continue,
            });

            // Butonlar
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            actions.set_valign(gtk::Align::Center);
            let play_btn = gtk::Button::with_label("▶ İzle");
            play_btn.add_css_class("suggested-action");
            play_btn.add_css_class("pill");
            let on_ic = on_item_click_rc.clone();
            let t_clone = item.title.clone();
            play_btn.connect_clicked(move |_| on_ic(t_clone.clone()));
            let del_btn = gtk::Button::from_icon_name("user-trash-symbolic");
            del_btn.add_css_class("flat");
            del_btn.add_css_class("circular");
            del_btn.add_css_class("destructive-action");
            del_btn.set_tooltip_text(Some("Maratondan Kaldır"));
            let on_rem = on_remove_rc.clone();
            del_btn.connect_clicked(move |_| on_rem(tid));
            actions.append(&play_btn);
            actions.append(&del_btn);

            card_box.append(&chk);
            card_box.append(&pic);
            card_box.append(&info_box);
            card_box.append(&actions);
            list_box.append(&card_box);
        }

        root.append(&list_box);
        root
    }
}

/// Geçmiş Sayfası (Checkbox ile seçmeli ve toplu silme aksiyonlu)
pub struct HistoryView;

impl HistoryView {
    pub fn build(
        _client: &Client,
        history: &[HistoryEntry],
        on_delete_selected: impl Fn(Vec<u64>) + 'static,
        on_clear_all: impl Fn() + 'static,
        on_item_click: impl Fn(HistoryEntry) + 'static,
        cover_loader: impl Fn(Option<&str>, &gtk::Picture, i32, i32) + 'static,
    ) -> gtk::Box {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.set_margin_top(8);
        root.set_margin_bottom(8);
        root.set_margin_start(12);
        root.set_margin_end(12);

        if history.is_empty() {
            let sp = crate::ui::components::create_status_page(
                "İzleme Geçmişi Boş",
                "Henüz bir bölüm veya film izlemediniz.",
                "document-open-recent-symbolic",
            );
            root.append(&sp);
            return root;
        }

        // --- Aksiyon Çubuğu ---
        let action_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        action_bar.set_margin_bottom(6);

        let select_all_chk = gtk::CheckButton::with_label("Tümünü Seç");
        select_all_chk.set_valign(gtk::Align::Center);

        let delete_sel_btn = gtk::Button::with_label("Seçilenleri Sil (0)");
        delete_sel_btn.add_css_class("destructive-action");
        delete_sel_btn.add_css_class("pill");
        delete_sel_btn.set_sensitive(false);
        delete_sel_btn.set_valign(gtk::Align::Center);

        let clear_all_btn = gtk::Button::with_label("Tümünü Temizle");
        clear_all_btn.add_css_class("flat");
        clear_all_btn.add_css_class("pill");
        clear_all_btn.set_valign(gtk::Align::Center);

        action_bar.append(&select_all_chk);
        action_bar.append(&delete_sel_btn);

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        action_bar.append(&spacer);
        action_bar.append(&clear_all_btn);

        root.append(&action_bar);

        // --- Liste ---
        let list_box = gtk::Box::new(gtk::Orientation::Vertical, 5);
        list_box.set_vexpand(false);

        let selected_ids = Rc::new(RefCell::new(Vec::<u64>::new()));
        let check_buttons = Rc::new(RefCell::new(Vec::<(u64, gtk::CheckButton)>::new()));
        let on_item_click_rc = Rc::new(on_item_click);

        let update_delete_btn = {
            let selected_ids = selected_ids.clone();
            let delete_sel_btn = delete_sel_btn.clone();
            move || {
                let count = selected_ids.borrow().len();
                delete_sel_btn.set_label(&format!("Seçilenleri Sil ({count})"));
                delete_sel_btn.set_sensitive(count > 0);
            }
        };

        for h in history {
            let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row_box.add_css_class("history-item-card");
            row_box.set_vexpand(false);
            row_box.set_height_request(84);

            let chk = gtk::CheckButton::new();
            chk.set_valign(gtk::Align::Center);

            let tid = h.title.id;
            let sel_clone = selected_ids.clone();
            let upd_clone = update_delete_btn.clone();
            chk.connect_toggled(move |b| {
                let mut ids = sel_clone.borrow_mut();
                if b.is_active() {
                    if !ids.contains(&tid) { ids.push(tid); }
                } else {
                    ids.retain(|&id| id != tid);
                }
                drop(ids);
                upd_clone();
            });
            check_buttons.borrow_mut().push((tid, chk.clone()));

            let pic = crate::covers::new_sized_picture(48, 72);
            pic.set_valign(gtk::Align::Center);
            cover_loader(h.title.poster.as_deref(), &pic, 48, 72);

            let text_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
            text_box.set_vexpand(true);
            text_box.set_valign(gtk::Align::Center);
            text_box.set_hexpand(true);

            let name = gtk::Label::new(Some(&h.title.name));
            name.set_xalign(0.0);
            name.set_wrap(false);
            name.set_single_line_mode(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            name.add_css_class("title-3");

            let sub = gtk::Label::new(Some(&format!(
                "S{:02} E{:02} · {}",
                h.episode.season, h.episode.episode, h.episode.name
            )));
            sub.set_xalign(0.0);
            sub.set_wrap(false);
            sub.set_single_line_mode(true);
            sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
            sub.add_css_class("dim-label");

            text_box.append(&name);
            text_box.append(&sub);
            episodes_view::append_title_submeta(&text_box, &h.title);

            let click_btn = gtk::Button::with_label("▶ İzle");
            click_btn.add_css_class("suggested-action");
            click_btn.add_css_class("pill");
            click_btn.set_valign(gtk::Align::Center);

            let h_clone = h.clone();
            let on_ic = on_item_click_rc.clone();
            click_btn.connect_clicked(move |_| {
                on_ic(h_clone.clone());
            });

            row_box.append(&chk);
            row_box.append(&pic);
            row_box.append(&text_box);
            row_box.append(&click_btn);

            list_box.append(&row_box);
        }

        let check_buttons_clone = check_buttons.clone();
        select_all_chk.connect_toggled(move |b| {
            let active = b.is_active();
            for (_, chk) in check_buttons_clone.borrow().iter() {
                chk.set_active(active);
            }
        });

        let selected_ids_clone = selected_ids.clone();
        let on_del = Rc::new(on_delete_selected);
        delete_sel_btn.connect_clicked(move |_| {
            let ids = selected_ids_clone.borrow().clone();
            if !ids.is_empty() {
                on_del(ids);
            }
        });

        let on_ca = Rc::new(on_clear_all);
        clear_all_btn.connect_clicked(move |_| {
            on_ca();
        });

        root.append(&list_box);
        root
    }
}

/// Ayarlar Sayfası
pub struct SettingsView;

impl SettingsView {
    pub fn build(
        settings: &Settings,
        on_save: impl Fn(Settings) + 'static,
        on_wipe: impl Fn(bool) + 'static,
    ) -> gtk::Box {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(16);
        root.set_margin_end(16);

        let ep_group = adw::PreferencesGroup::new();
        ep_group.set_title("Hızlı Bölüm Arama");

        let search_toggle_row = adw::SwitchRow::new();
        search_toggle_row.set_title("Aktif");
        search_toggle_row.set_subtitle("Bölüm ekranında klavye kısayolu ile hızlı bölüm arama çubuğunu aktif et");
        search_toggle_row.set_active(settings.quick_search_enabled);

        let shortcut_row = adw::ComboRow::new();
        shortcut_row.set_title("Kısayol Tuşu");
        shortcut_row.set_subtitle("Bölüm sayfasında aramayı başlatacak klavye kısayolu");
        let ep_shortcuts = &["/", "Ctrl+F", "F3", "Ctrl+K"];
        let ep_shortcut_model = gtk::StringList::new(ep_shortcuts);
        shortcut_row.set_model(Some(&ep_shortcut_model));
        let current_ep_sc = ep_shortcuts.iter().position(|&s| s == settings.quick_search_shortcut).unwrap_or(0);
        shortcut_row.set_selected(current_ep_sc as u32);
        shortcut_row.set_sensitive(settings.quick_search_enabled);

        ep_group.add(&search_toggle_row);
        ep_group.add(&shortcut_row);
        root.append(&ep_group);

        let search_group = adw::PreferencesGroup::new();
        search_group.set_title("Anime / Dizi Arama Kısayolu");

        let search_sc_row = adw::ComboRow::new();
        search_sc_row.set_title("Kısayol Tuşu");
        search_sc_row.set_subtitle("Ana ekranda arama çubuğunu açacak klavye kısayolu");
        let search_shortcuts = &["Ctrl+S", "Ctrl+K", "F2", "/"];
        let search_sc_model = gtk::StringList::new(search_shortcuts);
        search_sc_row.set_model(Some(&search_sc_model));
        let current_sc = search_shortcuts.iter().position(|&s| s == settings.search_shortcut).unwrap_or(0);
        search_sc_row.set_selected(current_sc as u32);
        search_group.add(&search_sc_row);
        root.append(&search_group);

        let player_group = adw::PreferencesGroup::new();
        player_group.set_title("Oynatıcı Ayarları");

        let fs_row = adw::SwitchRow::new();
        fs_row.set_title("MPV Otomatik Tam Ekran");
        fs_row.set_subtitle("Video başladığında MPV'yi otomatik tam ekran modunda açar");
        fs_row.set_active(settings.auto_fullscreen);
        player_group.add(&fs_row);

        let aniskip_row = adw::SwitchRow::new();
        aniskip_row.set_title("AniSkip Otomatik İntro Atlama Entegrasyonu");
        aniskip_row.set_subtitle("AniSkip API üzerinden 's' kısayol tuşu ile intro bitişine otomatik atlar");
        aniskip_row.set_active(settings.aniskip_enabled);
        player_group.add(&aniskip_row);
        root.append(&player_group);

        // Performans grubu: hafif mod (cairo renderer, düşük RAM)
        let perf_group = adw::PreferencesGroup::new();
        perf_group.set_title("Performans");

        let light_row = adw::SwitchRow::new();
        light_row.set_title("Hafif Mod (Düşük RAM)");
        light_row.set_subtitle("Arayüzü CPU ile çizer, bellek kullanımını ~%35 azaltır. Uygulamayı yeniden başlatınca geçerli olur.");
        light_row.set_active(settings.light_mode);
        perf_group.add(&light_row);
        root.append(&perf_group);

        // Görüntü iyileştirme (upscale)
        let img_group = adw::PreferencesGroup::new();
        img_group.set_title("Görüntü İyileştirme");
        let upscale_row = adw::ComboRow::new();
        upscale_row.set_title("Görüntü İyileştirme");
        upscale_row.set_subtitle("Düşük çözünürlüklü kaynağı yukarı ölçekler");
        let upscale_model = gtk::StringList::new(&[
            "Kapalı",
            "Keskinleştir",
            "Anime4K (hafif)",
            "Anime4K (normal)",
            "Anime4K (ultra)",
        ]);
        upscale_row.set_model(Some(&upscale_model));
        upscale_row.set_selected(match settings.upscale.as_str() {
            "anime4k_ultra" => 4,
            "anime4k_normal" => 3,
            "anime4k_light" | "anime4k" => 2,
            "sharp" => 1,
            _ => 0,
        });
        img_group.add(&upscale_row);

        // libadwaita 0.6 subtitle etrafında sarılmaz (yalnızca üç nokta ile keser);
        // uzun açıklamayı ayrı bir wrapping etiketle alt satıra geçiriyoruz.
        let upscale_desc = gtk::Label::new(Some(
            "Yalnızca kaynak çözünürlüğü ekrandan küçükse etki eder. Anime4K: hafif (DTD, iGPU dostu) / normal (klasik) / ultra (ağır CNN, GPU tüketir).",
        ));
        upscale_desc.set_wrap(true);
        upscale_desc.set_xalign(0.0);
        upscale_desc.set_margin_top(2);
        upscale_desc.set_margin_bottom(8);
        upscale_desc.set_margin_start(14);
        upscale_desc.set_selectable(false);
        upscale_desc.add_css_class("dim-label");
        img_group.add(&upscale_desc);
        root.append(&img_group);

        let update_group = adw::PreferencesGroup::new();
        update_group.set_title("Güncelleme");

        // on_save'i Rc'ye sar (hem kaydet hem "Bir Daha Gösterme" bastırma için kullanılacak)
        let on_save = Rc::new(on_save);

        let auto_update_row = adw::SwitchRow::new();
        auto_update_row.set_title("Otomatik Güncelleme");
        auto_update_row.set_subtitle("Başlatmada yeni sürümü kontrol eder ve AppImage'i kendisi günceller");
        auto_update_row.set_active(settings.auto_update);
        auto_update_row.set_sensitive(crate::update::is_appimage());
        update_group.add(&auto_update_row);

        let notify_row = adw::SwitchRow::new();
        notify_row.set_title("Güncel Sürüm Bildirimi");
        notify_row.set_subtitle("Başlatmada güncel sürümdeyken bilgilendirme göster");
        notify_row.set_active(settings.notify_uptodate);
        notify_row.set_sensitive(crate::update::is_appimage());
        update_group.add(&notify_row);

        let check_btn = gtk::Button::with_label("Şimdi Güncelle");
        check_btn.add_css_class("flat");
        check_btn.add_css_class("pill");
        check_btn.set_margin_top(4);
        check_btn.set_sensitive(crate::update::is_appimage());
        let check_btn_c = check_btn.clone();
        let settings_for_suppress = settings.clone();
        let on_save_suppress = on_save.clone();
        check_btn.connect_clicked(move |_| {
            if let Some(win) = check_btn_c.root().and_downcast::<gtk::Window>() {
                let cur = settings_for_suppress.clone();
                let suppress = on_save_suppress.clone();
                crate::update::check_and_prompt(&win, true, move || {
                    let mut sup = cur.clone();
                    sup.notify_uptodate = false;
                    suppress(sup);
                });
            }
        });
        update_group.add(&check_btn);
        root.append(&update_group);

        let shortcut_row_c = shortcut_row.clone();
        search_toggle_row.connect_active_notify(move |r| {
            shortcut_row_c.set_sensitive(r.is_active());
        });

        let s_base = settings.clone();

        let save_all = {
            let st_r = search_toggle_row.clone();
            let sc_r = shortcut_row.clone();
            let ssc_r = search_sc_row.clone();
            let fs_r = fs_row.clone();
            let ani_r = aniskip_row.clone();
            let au_r = auto_update_row.clone();
            let notify_r = notify_row.clone();
            let up_r = upscale_row.clone();
            let light_r = light_row.clone();
            let s = s_base.clone();
            let on_save = on_save.clone();
            Rc::new(move || {
                let mut updated = s.clone();
                updated.quick_search_enabled = st_r.is_active();
                updated.quick_search_shortcut = match sc_r.selected() {
                    1 => "Ctrl+F".into(),
                    2 => "F3".into(),
                    3 => "Ctrl+K".into(),
                    _ => "/".into(),
                };
                updated.search_shortcut = match ssc_r.selected() {
                    1 => "Ctrl+K".into(),
                    2 => "F2".into(),
                    3 => "/".into(),
                    _ => "Ctrl+S".into(),
                };
                updated.auto_fullscreen = fs_r.is_active();
                updated.aniskip_enabled = ani_r.is_active();
                updated.auto_update = au_r.is_active();
                updated.notify_uptodate = notify_r.is_active();
                updated.upscale = match up_r.selected() {
                    1 => "sharp".into(),
                    2 => "anime4k_light".into(),
                    3 => "anime4k_normal".into(),
                    4 => "anime4k_ultra".into(),
                    _ => "off".into(),
                };
                updated.light_mode = light_r.is_active();
                on_save(updated);
            })
        };

        let sa1 = save_all.clone();
        search_toggle_row.connect_active_notify(move |_| sa1());
        let sa2 = save_all.clone();
        shortcut_row.connect_selected_notify(move |_| sa2());
        let sa3 = save_all.clone();
        search_sc_row.connect_selected_notify(move |_| sa3());
        let sa4 = save_all.clone();
        fs_row.connect_active_notify(move |_| sa4());
        let sa5 = save_all.clone();
        aniskip_row.connect_active_notify(move |_| sa5());
        let sa6 = save_all.clone();
        auto_update_row.connect_active_notify(move |_| sa6());
        let sa7 = save_all.clone();
        notify_row.connect_active_notify(move |_| sa7());
        let sa8 = save_all.clone();
        upscale_row.connect_selected_notify(move |_| sa8());
        let sa9 = save_all.clone();
        light_row.connect_active_notify(move |_| sa9());

        let data_group = adw::PreferencesGroup::new();
        data_group.set_title("Veri Yönetimi");

        let uninstall_row = adw::SwitchRow::new();
        uninstall_row.set_title("Uygulamayı ve Başlatıcıyı da Sistemden Kaldır");
        uninstall_row.set_subtitle("Sıfırlama ile birlikte uygulama binary dosyasını ve masaüstü kısayollarını tamamen siler");
        uninstall_row.set_active(true);
        data_group.add(&uninstall_row);

        let wipe_btn = gtk::Button::with_label("Tüm Verileri Sıfırla ve Temizle");
        wipe_btn.add_css_class("destructive-action");
        wipe_btn.set_margin_top(8);
        let on_wipe = Rc::new(on_wipe);
        let un_c = uninstall_row.clone();

        wipe_btn.connect_clicked(move |btn| {
            let remove_app = un_c.is_active();
            let parent_win = btn.root().and_downcast::<gtk::Window>();

            let dialog = adw::MessageDialog::builder()
                .heading("Kalıcı Sıfırlama Onayı ⚠️")
                .body(if remove_app {
                    "Tüm izleme geçmişiniz, ayarlarınız, kapak önbelleği ve UYGULAMA DOSYALARI sisteminizden kalıcı olarak silinecek. Emin misiniz?"
                } else {
                    "Tüm izleme geçmişiniz, ayarlarınız ve kapak önbelleği sıfırlanacak. Emin misiniz?"
                })
                .close_response("cancel")
                .default_response("cancel")
                .build();

            if let Some(win) = parent_win.as_ref() {
                dialog.set_transient_for(Some(win));
            }

            dialog.add_response("cancel", "İptal");
            dialog.add_response("wipe", "Evet, Kalıcı Olarak Sil");
            dialog.set_response_appearance("wipe", adw::ResponseAppearance::Destructive);

            let on_wipe_c = on_wipe.clone();
            dialog.connect_response(None, move |_, resp| {
                if resp == "wipe" {
                    on_wipe_c(remove_app);
                }
            });

            dialog.present();
        });
        data_group.add(&wipe_btn);
        root.append(&data_group);

        let info_group = adw::PreferencesGroup::new();
        info_group.set_title("Uygulama Bilgisi");

        let ver_row = adw::ActionRow::new();
        ver_row.set_title("Sürüm Numarası");
        ver_row.set_subtitle(&format!("AnimeciX Masaüstü İstemcisi  •  v{}", env!("CARGO_PKG_VERSION")));

        let ver_badge = gtk::Label::new(Some("Güncel ✓"));
        ver_badge.add_css_class("status-badge-completed");
        ver_badge.set_valign(gtk::Align::Center);
        ver_row.add_suffix(&ver_badge);
        info_group.add(&ver_row);

        let reinstall_btn = gtk::Button::with_label("Masaüstü Başlatıcısını Sistemime Kur / Güncelle");
        reinstall_btn.add_css_class("flat");
        reinstall_btn.add_css_class("pill");
        reinstall_btn.set_margin_top(4);
        reinstall_btn.connect_clicked(|_| {
            let _ = crate::install_desktop_entry();
        });
        info_group.add(&reinstall_btn);

        root.append(&info_group);

        root
    }
}
