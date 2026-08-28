use gtk::prelude::*;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::aicix;

pub struct AicixPanel {
    pub popover: gtk::Popover,
    pub message_list: gtk::Box,
    pub input_entry: gtk::Entry,
    pub send_btn: gtk::Button,
    pub state: Arc<Mutex<aicix::AicixState>>,
    pub client: aicix::AicixClient,
    pub messages_container: gtk::Box,
    pub status_label: gtk::Label,
    pub empty_state: gtk::Box,
    pub empty_state_visible: Rc<RefCell<bool>>,
}

impl AicixPanel {
    pub fn new(state: Arc<Mutex<aicix::AicixState>>, parent: &impl IsA<gtk::Widget>) -> Self {
        let popover = gtk::Popover::new();
        popover.set_parent(parent);
        popover.set_position(gtk::PositionType::Top);
        popover.set_size_request(420, 560);
        popover.set_autohide(false);
        popover.add_css_class("aicix-popover");

        let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        outer.set_size_request(420, 560);

        let header = gtk::HeaderBar::new();
        header.set_title_widget(Some(&gtk::Label::new(Some("Aicix · Yapay Zeka Asistan"))));
        header.add_css_class("flat");
        outer.append(&header);

        let status_label = gtk::Label::new(Some("Mesaj yazıp Enter'a bas"));
        status_label.set_xalign(0.0);
        status_label.set_margin_start(12);
        status_label.set_margin_end(12);
        status_label.set_margin_top(4);
        status_label.set_margin_bottom(4);
        status_label.add_css_class("dim-label");
        status_label.add_css_class("caption");
        outer.append(&status_label);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        scrolled.set_vscrollbar_policy(gtk::PolicyType::Automatic);
        scrolled.set_margin_start(8);
        scrolled.set_margin_end(8);
        scrolled.set_margin_bottom(4);

        let messages_container = gtk::Box::new(gtk::Orientation::Vertical, 6);
        messages_container.set_margin_top(4);
        messages_container.set_margin_bottom(4);
        messages_container.set_halign(gtk::Align::Fill);
        scrolled.set_child(Some(&messages_container));
        outer.append(&scrolled);

        let input_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        input_box.set_margin_start(8);
        input_box.set_margin_end(8);
        input_box.set_margin_top(4);
        input_box.set_margin_bottom(8);

        let input_entry = gtk::Entry::new();
        input_entry.set_placeholder_text(Some("Mesajınızı yazın…"));
        input_entry.set_hexpand(true);
        input_entry.add_css_class("aicix-input");
        input_box.append(&input_entry);

        let send_btn = gtk::Button::from_icon_name("paper-plane-symbolic");
        send_btn.add_css_class("suggested-action");
        send_btn.add_css_class("circular");
        send_btn.set_tooltip_text(Some("Gönder"));
        input_box.append(&send_btn);

        outer.append(&input_box);
        popover.set_child(Some(&outer));

        let client = aicix::AicixClient::new(state.clone());

        let panel = Self {
            popover,
            message_list: messages_container.clone(),
            input_entry: input_entry.clone(),
            send_btn,
            state: state.clone(),
            client,
            messages_container,
            status_label: status_label.clone(),
            empty_state: gtk::Box::new(gtk::Orientation::Vertical, 0),
            empty_state_visible: Rc::new(RefCell::new(false)),
        };

        panel.refresh_messages();
        panel
    }

    pub fn refresh_messages(&self) {
        while let Some(child) = self.messages_container.first_child() {
            self.messages_container.remove(&child);
        }
        let state = self.state.lock().unwrap();
        if state.history.is_empty() {
            let empty = gtk::Label::new(Some("Merhaba! Aicix'e bir şeyler sor. Örneğin:\n\n• \"bana aksiyon anime öner\"\n• \"one piece kaç bölüm\"\n• \"naruto aç\""));
            empty.set_xalign(0.5);
            empty.set_yalign(0.5);
            empty.set_wrap(true);
            empty.set_vexpand(true);
            empty.set_valign(gtk::Align::Center);
            empty.set_margin_top(80);
            empty.set_margin_bottom(80);
            empty.set_margin_start(16);
            empty.set_margin_end(16);
            empty.add_css_class("dim-label");
            self.messages_container.append(&empty);
            return;
        }
        for msg in state.history.iter() {
            let bubble = Self::make_bubble(msg);
            self.messages_container.append(&bubble);
        }
    }

    fn make_bubble(msg: &aicix::ChatMessage) -> gtk::Widget {
        let wrap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        wrap.set_halign(match msg.role {
            aicix::MessageRole::User => gtk::Align::End,
            aicix::MessageRole::Assistant => gtk::Align::Start,
            aicix::MessageRole::Tool => gtk::Align::Start,
            aicix::MessageRole::System => gtk::Align::Start,
        });
        wrap.set_margin_start(8);
        wrap.set_margin_end(8);
        wrap.set_margin_top(4);
        wrap.set_margin_bottom(4);

        let bubble = gtk::Box::new(gtk::Orientation::Vertical, 4);
        bubble.set_halign(gtk::Align::Start);
        bubble.set_size_request(360, -1);

        let label_text = if msg.content.is_empty() {
            "(...)".to_string()
        } else {
            msg.content.clone()
        };
        let label = gtk::Label::new(Some(&label_text));
        label.set_wrap(true);
        label.set_xalign(0.0);
        label.set_yalign(0.0);
        label.set_selectable(true);
        label.set_margin_start(12);
        label.set_margin_end(12);
        label.set_margin_top(8);
        label.set_margin_bottom(8);

        match msg.role {
            aicix::MessageRole::User => {
                bubble.add_css_class("aicix-bubble-user");
                label.set_xalign(1.0);
            }
            aicix::MessageRole::Assistant => {
                bubble.add_css_class("aicix-bubble-bot");
            }
            aicix::MessageRole::Tool => {
                bubble.add_css_class("aicix-bubble-tool");
            }
            aicix::MessageRole::System => {
                bubble.add_css_class("aicix-bubble-system");
            }
        }
        bubble.append(&label);
        wrap.append(&bubble);
        wrap.upcast::<gtk::Widget>()
    }

    pub fn show(&self) {
        self.refresh_messages();
        self.popover.popup();
    }

    pub fn append_text_message(&self, role: aicix::MessageRole, text: &str) {
        let msg = aicix::ChatMessage {
            role,
            content: text.to_string(),
            tool_call_id: None,
            tool_calls: None,
            name: None,
            is_card: false,
            card: None,
        };
        let bubble = Self::make_bubble(&msg);
        self.messages_container.append(&bubble);
    }

    pub fn update_status(&self, text: &str) {
        self.status_label.set_text(text);
    }
}
