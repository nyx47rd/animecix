use gtk::prelude::*;
use crate::api::{Client, Title};

pub fn bookmark_button(client: &Client, t: &Title) -> gtk::Button {
    let saved = client.is_saved(t.id);
    let b = gtk::Button::from_icon_name(if saved {
        "starred-symbolic"
    } else {
        "non-starred-symbolic"
    });
    b.add_css_class("flat");
    b.add_css_class("circular");
    b.add_css_class("lg-icon");
    b.add_css_class("bookmark-btn");
    b.set_tooltip_text(Some(if saved { "Favorilerden Çıkar" } else { "Favorilere Ekle" }));
    b
}

pub fn marathon_button(client: &Client, t: &Title) -> gtk::Button {
    let in_marathon = client.is_in_marathon(t.id);
    let b = gtk::Button::from_icon_name(if in_marathon {
        "media-playlist-repeat-symbolic"
    } else {
        "flag-symbolic"
    });
    b.add_css_class("flat");
    b.add_css_class("circular");
    b.add_css_class("lg-icon");
    b.set_tooltip_text(Some(if in_marathon { "Maratondan Çıkar" } else { "İzleme Maratonuna Ekle" }));
    b
}

pub fn create_status_page(title: &str, description: &str, icon_name: &str) -> adw::StatusPage {
    let sp = adw::StatusPage::new();
    sp.set_title(title);
    sp.set_description(Some(description));
    sp.set_icon_name(Some(icon_name));
    sp.set_vexpand(true);
    sp
}
