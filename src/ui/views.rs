use gtk::prelude::*;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use crate::aicix;
use crate::api::{Client, HistoryEntry, Settings, Title};
use crate::ui::episodes_view;
use gio::prelude::*;

pub(crate) fn show_info_dialog(parent: Option<&gtk::Window>, heading: &str, body: &str) {
    let dialog = adw::MessageDialog::builder()
        .heading(heading)
        .body(body)
        .close_response("ok")
        .default_response("ok")
        .build();
    if let Some(win) = parent {
        dialog.set_transient_for(Some(win));
    }
    dialog.add_response("ok", "Tamam");
    dialog.present();
}

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
                "Gelecekte izleyeceğiniz anime, dizi ve filmleri detay sayfasındaki maraton butonuna (🏁) tıklayarak ekleyebilirsiniz.",
                "media-playlist-repeat-symbolic",
            );
            root.append(&sp);
            return root;
        }

        let total_count = marathon_items.len();
        let completed_count = marathon_items.iter().filter(|m| m.completed).count();
        let percent = if total_count > 0 { (completed_count * 100) / total_count } else { 0 };

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

        let list_box = gtk::Box::new(gtk::Orientation::Vertical, 8);

        let on_item_click_rc = Rc::new(on_item_click);
        let on_toggle_rc = Rc::new(on_toggle_completed);
        let on_remove_rc = Rc::new(on_remove_item);
        let on_reorder_rc = Rc::new(on_reorder);
        let cover_loader_rc = Rc::new(cover_loader);

        for (idx, item) in marathon_items.iter().enumerate() {
            let card_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            card_box.add_css_class("marathon-item-card");

            let num = gtk::Label::new(Some(&format!("{}", idx + 1)));
            num.add_css_class("marathon-index");
            num.set_xalign(0.5);
            num.set_yalign(0.5);
            num.set_valign(gtk::Align::Center);
            card_box.append(&num);

            let drag = gtk::DragSource::new();
            drag.set_actions(gtk::gdk::DragAction::MOVE);
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

            let chk = gtk::CheckButton::new();
            chk.set_active(item.completed);
            chk.set_valign(gtk::Align::Center);
            chk.set_tooltip_text(Some(if item.completed { "Tamamlandı olarak işaretli" } else { "Tamamlandı olarak işaretle" }));
            let tid = item.title.id;
            let on_t_c = on_toggle_rc.clone();
            chk.connect_toggled(move |_| { on_t_c(tid); });

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

        let perf_group = adw::PreferencesGroup::new();
        perf_group.set_title("Performans");

        let light_row = adw::SwitchRow::new();
        light_row.set_title("Hafif Mod (Düşük RAM)");
        light_row.set_subtitle("Arayüzü CPU ile çizer, bellek kullanımını ~%35 azaltır. Uygulamayı yeniden başlatınca geçerli olur.");
        light_row.set_active(settings.light_mode);
        perf_group.add(&light_row);

        let patience_row = adw::ActionRow::new();
        patience_row.set_title("Kaynak Açılış Sabrı");
        patience_row.set_subtitle("Yavaş internet için artırın. Medya hiç açılmazsa ölü kaynakta bu kadar saniye (20-120) beklenir, sonra sıradakine geçilir.");
        let patience_adj = gtk::Adjustment::new(settings.source_patience_secs as f64, 20.0, 120.0, 5.0, 10.0, 0.0);
        let patience_spin = gtk::SpinButton::new(Some(&patience_adj), 1.0, 0);
        patience_spin.set_numeric(true);
        patience_spin.set_value(settings.source_patience_secs as f64);
        patience_row.add_suffix(&patience_spin);
        perf_group.add(&patience_row);
        root.append(&perf_group);

        if !crate::vpn::in_flatpak() {
        let vpn_group = adw::PreferencesGroup::new();
        vpn_group.set_title("VPN Proxy (İsteğe Bağlı)");

        let info_btn = gtk::Button::from_icon_name("dialog-information");
        info_btn.add_css_class("flat");
        info_btn.add_css_class("circular");
        info_btn.set_tooltip_text(Some("VPN Proxy ne işe yarar? Tıkla, detaylı açıkla."));
        info_btn.set_valign(gtk::Align::Center);
        {
            let info_btn = info_btn.clone();
            info_btn.connect_clicked(move |btn| {
                let parent = btn.root().and_downcast::<gtk::Window>();
                show_info_dialog(
                    parent.as_ref(),
                    "VPN Proxy nedir?",
                    "ISS'n (internet sağlayıcı) video trafiğini yavaşlatıp kısıtlıyorsa buradan \
yerel bir proxy (sing-box + ProtonVPN WireGuard) çalıştırabilirsin.\n\n\
• Proxy ayakta olduğunda video trafiği otomatik olarak \
127.0.0.1:10808 üzerinden çıkar; root (yönetici) izni gerekmez.\n\
• Başlattıktan sonra çıkan pencerede 'Yeniden Başlat' dersen ana sayfa/arama gibi \
API istekleri de tünel üzerinden gider (ISS engellerini tamamen aşar).\n\
• Proxy kapalıyken hiçbir şey değişmez, uygulama normal bağlantını kullanır.\n\
• İlk kurulum için 'Başlat'a bas, adım adım rehber çıkar.\n\
• Proxy kapatılırsa uygulama otomatik normal bağlantıya döner, hiçbir ayarın bozulmaz.",
                );
            });
        }
        vpn_group.set_header_suffix(Some(&info_btn));

        let vpn_status_row = adw::ActionRow::new();
        vpn_status_row.set_title("Durum");
        vpn_status_row.set_subtitle("Yerel proxy (127.0.0.1:10808) üzerinden ISS kısıtlamalarını aşar");
        let refresh_vpn_status = {
            let row = vpn_status_row.clone();
            move || {
                if crate::vpn::port_alive() {
                    row.set_subtitle("Çalışıyor — video trafiği 127.0.0.1:10808 üzerinden çıkıyor");
                    row.remove_css_class("dim-label");
                } else {
                    row.set_subtitle("Kapalı — uygulama normal bağlantıyı kullanır");
                    row.add_css_class("dim-label");
                }
            }
        };
        refresh_vpn_status();
        vpn_group.add(&vpn_status_row);
        root.append(&vpn_group);

        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let start_btn = gtk::Button::with_label("Başlat");
        start_btn.add_css_class("suggested-action");
        start_btn.add_css_class("pill");
        let stop_btn = gtk::Button::with_label("Durdur");
        stop_btn.add_css_class("pill");
        btn_box.append(&start_btn);
        btn_box.append(&stop_btn);
        let vpn_btn_row = adw::ActionRow::new();
        vpn_btn_row.set_title("sing-box (WireGuard → SOCKS köprüsü)");
        vpn_btn_row.set_subtitle("ProtonVPN ücretsiz hesapla çalışır; kurulum için Başlat'a bas");
        vpn_btn_row.add_suffix(&btn_box);
        vpn_group.add(&vpn_btn_row);

        let refresh_status_rc = std::rc::Rc::new(refresh_vpn_status);

        let live_rs = refresh_status_rc.clone();
        glib::timeout_add_seconds_local(2, move || {
            live_rs();
            glib::ControlFlow::Continue
        });

        let rs = refresh_status_rc.clone();
        start_btn.connect_clicked(move |btn| {
            let parent = btn.root().and_downcast::<gtk::Window>();
            let rs = rs.clone();
            glib::spawn_future_local(async move {
                match crate::vpn::detect() {
                    None => {
                        show_info_dialog(parent.as_ref(), "VPN Proxy Kurulumu", &crate::vpn::setup_instructions());
                        rs();
                    }
                    Some((bin, cfg)) => {
                        let res = {
                            let bin = bin.clone();
                            let cfg = cfg.clone();
                            gio::spawn_blocking(move || crate::vpn::start(&bin, &cfg)).await
                        };
                        match res {
                            Ok(Ok(())) => {
                                let dlg = adw::MessageDialog::builder()
                                    .heading("VPN Proxy başlatıldı")
                                    .body("Video trafiği artık tünel üzerinden çıkıyor.\nAPI isteklerinin (ana sayfa, arama) de tüneleden geçmesi için uygulamayı yeniden başlat.")
                                    .close_response("later")
                                    .build();
                                dlg.add_response("later", "Sonra");
                                dlg.add_response("restart", "Yeniden Başlat 🔄");
                                dlg.set_response_appearance("restart", adw::ResponseAppearance::Suggested);
                                if let Some(w) = parent.as_ref() {
                                    dlg.set_transient_for(Some(w));
                                }
                                dlg.connect_response(None, move |_, resp| {
                                    if resp == "restart" {
                                        crate::restart_app();
                                    }
                                });
                                dlg.present();
                            }
                            Ok(Err(e)) => show_info_dialog(parent.as_ref(), "VPN Proxy Hatası", &format!("{e}\n\nLog dosyası: {}", crate::vpn::log_path().display())),
                            Err(_) => show_info_dialog(parent.as_ref(), "VPN Proxy Hatası", "Proxy başlatılırken beklenmeyen bir hata oluştu (süreç çökmüş olabilir)."),
                        }
                        rs();
                    }
                }
            });
        });

        let rs2 = refresh_status_rc.clone();
        stop_btn.connect_clicked(move |btn| {
            let parent = btn.root().and_downcast::<gtk::Window>();
            let rs2 = rs2.clone();
            glib::spawn_future_local(async move {
                let killed = gio::spawn_blocking(|| crate::vpn::stop()).await;
                match killed {
                    Ok(true) => {}
                    Ok(false) => show_info_dialog(parent.as_ref(), "VPN Proxy", "Çalışan sing-box süreci bulunamadı."),
                    Err(_) => show_info_dialog(parent.as_ref(), "VPN Proxy Hatası", "Proxy durdurulurken beklenmeyen bir hata oluştu."),
                }
                rs2();
            });
        });
        }

        let img_group = adw::PreferencesGroup::new();
        img_group.set_title("Görüntü İyileştirme");
        let upscale_row = adw::ComboRow::new();
        upscale_row.set_title("Görüntü İyileştirme");
        upscale_row.set_subtitle("Düşük çözünürlüklü kaynağı yukarı ölçekler");
        let upscale_model = gtk::StringList::new(&[
            "Kapalı",
            "Keskinleştir",
            "Hafif",
            "Ultra",
            "Hafif + Keskinleştirme",
        ]);
        upscale_row.set_model(Some(&upscale_model));
        upscale_row.set_selected(match settings.upscale.as_str() {
            "hafif_keskin" => 4,
            "ultra" => 3,
            "hafif" => 2,
            "sharp" => 1,
            _ => 0,
        });
        img_group.add(&upscale_row);

        let upscale_desc = gtk::Label::new(Some(
            "Yalnızca kaynak çözünürlüğü ekrandan küçükse etki eder.\nHafif: DTD (iGPU dostu, hafif). Ultra: CNN (en kaliteli). Hafif + Keskinleştirme: DTD + keskinleştirme filtresi.",
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

        let fansub_group = adw::PreferencesGroup::new();
        fansub_group.set_title("Çeviri (Fansub) Seçimi");
        let ask_row = adw::SwitchRow::new();
        ask_row.set_title("Her bölümde sor");
        ask_row.set_subtitle("Kapalıysa otomatik olarak en yüksek puanlı çeviri seçilir");
        ask_row.set_active(settings.fansub_ask_each_time);
        fansub_group.add(&ask_row);
        let fansub_desc = gtk::Label::new(Some(
            "Bir bölüme tıkladığınızda mevcut çeviriler listelenir (örn. Kirigana, Wolwead). Puan yıldızı topluluk oylarına dayanır.",
        ));
        fansub_desc.set_wrap(true);
        fansub_desc.set_xalign(0.0);
        fansub_desc.set_margin_top(2);
        fansub_desc.set_margin_bottom(8);
        fansub_desc.set_margin_start(14);
        fansub_desc.set_selectable(false);
        fansub_desc.add_css_class("dim-label");
        fansub_group.add(&fansub_desc);
        root.append(&fansub_group);

        let aicix_group = adw::PreferencesGroup::new();
        aicix_group.set_title("Aicix (Yapay Zeka Asistan)");
        aicix_group.set_description(Some("Groq API kullanarak doğal dilde anime arama ve öneri alın"));

        let aicix_key_row = adw::PasswordEntryRow::new();
        aicix_key_row.set_title("Groq API Anahtarı");
        aicix_key_row.set_show_apply_button(false);
        aicix_key_row.set_text(settings.aicix_api_key.as_deref().unwrap_or(""));
        aicix_group.add(&aicix_key_row);

        let aicix_model_row = adw::ComboRow::new();
        aicix_model_row.set_title("Model");
        aicix_model_row.set_subtitle("qwen/qwen3.8-27b önerilen");
        let models = &[
            "qwen/qwen3.8-27b",
            "qwen/qwen3-32b",
            "llama-3.3-70b-versatile",
            "llama-3.1-8b-instant",
        ];
        let model_list = gtk::StringList::new(models);
        aicix_model_row.set_model(Some(&model_list));
        let cur = models.iter().position(|&m| m == settings.aicix_model).unwrap_or(0);
        aicix_model_row.set_selected(cur as u32);
        aicix_group.add(&aicix_model_row);

        let aicix_test_btn = gtk::Button::with_label("Bağlantıyı Test Et");
        aicix_test_btn.add_css_class("pill");
        aicix_test_btn.add_css_class("suggested-action");
        aicix_test_btn.set_halign(gtk::Align::End);
        aicix_test_btn.set_margin_top(8);
        aicix_test_btn.set_margin_end(14);
        aicix_test_btn.set_margin_bottom(8);
        aicix_group.add(&aicix_test_btn);

        let aicix_desc = gtk::Label::new(Some(
            "BYOK: API anahtarınız Groq'a gönderilir, başka yere kaydedilmez. Anahtar sadece bu cihazda, ~/.config/animecix/state.json içinde saklanır.\n\nÜcretsiz Groq anahtarı: console.groq.com/keys",
        ));
        aicix_desc.set_wrap(true);
        aicix_desc.set_xalign(0.0);
        aicix_desc.set_margin_top(2);
        aicix_desc.set_margin_bottom(8);
        aicix_desc.set_margin_start(14);
        aicix_desc.set_selectable(true);
        aicix_desc.add_css_class("dim-label");
        aicix_group.add(&aicix_desc);
        root.append(&aicix_group);

        let update_group = adw::PreferencesGroup::new();
        update_group.set_title("Güncelleme");

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
            let patience_spin_c = patience_spin.clone();
            let ask_r = ask_row.clone();
            let aicix_key_r = aicix_key_row.clone();
            let aicix_model_r = aicix_model_row.clone();
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
                    2 => "hafif".into(),
                    3 => "ultra".into(),
                    4 => "hafif_keskin".into(),
                    _ => "off".into(),
                };
                updated.light_mode = light_r.is_active();
                updated.source_patience_secs = patience_spin_c.value() as u64;
                updated.fansub_ask_each_time = ask_r.is_active();
                let key_text = aicix_key_r.text().to_string();
                updated.aicix_api_key = if key_text.trim().is_empty() {
                    None
                } else {
                    Some(key_text)
                };
                updated.aicix_model = match aicix_model_r.selected() {
                    1 => "qwen/qwen3-32b".into(),
                    2 => "llama-3.3-70b-versatile".into(),
                    3 => "llama-3.1-8b-instant".into(),
                    _ => "qwen/qwen3.8-27b".into(),
                };
                on_save(updated);
            })
        };

        let sa_ask = save_all.clone();
        ask_row.connect_active_notify(move |_| sa_ask());
        let sa_aicix_key = save_all.clone();
        aicix_key_row.connect_changed(move |_| {
            let _ = sa_aicix_key;
        });
        let sa_aicix_apply = save_all.clone();
        aicix_key_row.connect_apply(move |_| {
            sa_aicix_apply();
        });
        let sa_aicix_model = save_all.clone();
        aicix_model_row.connect_selected_notify(move |_| sa_aicix_model());

        aicix_test_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            let key = aicix_key_row.text().to_string();
            if key.trim().is_empty() {
                btn.set_label("Anahtar boş");
                btn.set_sensitive(true);
                return;
            }
            let model = match aicix_model_row.selected() {
                1 => "qwen/qwen3-32b",
                2 => "llama-3.3-70b-versatile",
                3 => "llama-3.1-8b-instant",
                _ => "qwen/qwen3.8-27b",
            };
            let state = Arc::new(Mutex::new(aicix::AicixState::new()));
            state.lock().unwrap().api_key = Some(key);
            state.lock().unwrap().model = model.to_string();
            let client = aicix::AicixClient::new(state);
            let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
            std::thread::spawn(move || {
                let result = client.send_test();
                let _ = tx.send(result);
            });
            let btn_c = btn.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Ok(result) = rx.recv() {
                    match result {
                        Ok(msg) => {
                            btn_c.set_label(&format!("✓ {msg}"));
                        }
                        Err(e) => {
                            btn_c.set_label(&format!("✗ {e}"));
                        }
                    }
                }
                btn_c.set_sensitive(true);
            });
        });

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
        let sa10 = save_all.clone();
        patience_spin.connect_value_changed(move |_| sa10());

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
