use std::rc::Rc;
use gtk::prelude::*;
use crate::api::VideoSource;

pub fn show_source_dialog(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    sources: Vec<VideoSource>,
    on_select: impl Fn(VideoSource) + 'static,
) {
    let dialog = gtk::Window::builder()
        .title(title)
        .modal(true)
        .transient_for(parent)
        .default_width(420)
        .default_height(300)
        .resizable(false)
        .build();

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);

    let header = gtk::Label::new(Some("Kaynak Seç"));
    header.add_css_class("title-2");
    header.set_xalign(0.0);
    vbox.append(&header);

    let count_text = format!("{} kaynak bulundu", sources.len());
    let subtitle = gtk::Label::new(Some(&count_text));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);
    vbox.append(&subtitle);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let on_select = Rc::new(on_select);

    for source in &sources {
        let row = gtk::Button::new();
        row.add_css_class("card");
        row.set_halign(gtk::Align::Fill);

        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);

        let host_lbl = gtk::Label::new(Some(&capitalize_host(&source.host)));
        host_lbl.add_css_class("title-4");
        host_lbl.set_xalign(0.0);
        host_lbl.set_hexpand(true);
        hbox.append(&host_lbl);

        if !source.quality.is_empty() {
            let q_badge = gtk::Label::new(Some(&source.quality));
            q_badge.add_css_class("accent");
            q_badge.set_xalign(0.5);
            hbox.append(&q_badge);
        }

        if source.votes > 0 {
            let votes_text = format!("👍 {}", source.votes);
            let votes_lbl = gtk::Label::new(Some(&votes_text));
            votes_lbl.add_css_class("dim-label");
            votes_lbl.set_xalign(0.5);
            hbox.append(&votes_lbl);
        }

        if source.points > 0.0 {
            let pts_text = format!("⭐ {:.1}", source.points);
            let pts_lbl = gtk::Label::new(Some(&pts_text));
            pts_lbl.add_css_class("dim-label");
            pts_lbl.set_xalign(0.5);
            hbox.append(&pts_lbl);
        }

        row.set_child(Some(&hbox));

        let src = source.clone();
        let on_sel = on_select.clone();
        let dlg = dialog.clone();
        row.connect_clicked(move |_| {
            on_sel(src.clone());
            dlg.close();
        });

        list.append(&row);
    }

    scrolled.set_child(Some(&list));
    vbox.append(&scrolled);

    let close_btn = gtk::Button::with_label("İptal");
    close_btn.set_halign(gtk::Align::End);
    let dlg2 = dialog.clone();
    close_btn.connect_clicked(move |_| dlg2.close());
    vbox.append(&close_btn);

    dialog.set_child(Some(&vbox));
    dialog.present();
}

fn capitalize_host(host: &str) -> String {
    match host {
        "tau-video" | "tau" => "Tau Video".to_string(),
        "sibnet" => "Sibnet".to_string(),
        "streamtape" => "Streamtape".to_string(),
        _ => {
            let mut c = host.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    }
}
