use gtk::prelude::*;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::aicix;

pub struct AicixPanel {
    pub root: gtk::Box,
    pub message_list: gtk::Box,
    pub input_entry: gtk::Entry,
    pub send_btn: gtk::Button,
    pub state: Arc<Mutex<aicix::AicixState>>,
    pub client: aicix::AicixClient,
    pub messages_container: gtk::Box,
    pub status_label: gtk::Label,
    pub streaming_assistant_label: Rc<RefCell<Option<gtk::Label>>>,
    pub streaming_acc: Rc<RefCell<String>>,
}

impl AicixPanel {
    pub fn new(state: Arc<Mutex<aicix::AicixState>>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.set_margin_start(12);
        toolbar.set_margin_end(12);
        toolbar.set_margin_top(8);
        toolbar.set_margin_bottom(4);

        let new_btn = gtk::Button::new();
        new_btn.set_icon_name("user-trash-symbolic");
        new_btn.set_label("Yeni Sohbet");
        new_btn.set_tooltip_text(Some("Konuşma geçmişini temizle"));
        new_btn.add_css_class("aicix-new-btn");
        toolbar.append(&new_btn);

        let model_label = gtk::Label::new(Some("Model: qwen/qwen3.8-27b"));
        model_label.add_css_class("dim-label");
        model_label.add_css_class("caption");
        model_label.set_hexpand(true);
        model_label.set_xalign(1.0);
        toolbar.append(&model_label);
        root.append(&toolbar);

        let status_label = gtk::Label::new(Some("Mesaj yazıp Enter'a bas. BYOK — API anahtarın sadece senin cihazında."));
        status_label.set_xalign(0.0);
        status_label.set_margin_start(12);
        status_label.set_margin_end(12);
        status_label.set_margin_top(2);
        status_label.set_margin_bottom(6);
        status_label.add_css_class("dim-label");
        status_label.add_css_class("caption");
        status_label.set_wrap(true);
        root.append(&status_label);

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
        root.append(&scrolled);

        let input_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        input_box.set_margin_start(8);
        input_box.set_margin_end(8);
        input_box.set_margin_top(4);
        input_box.set_margin_bottom(12);

        let input_entry = gtk::Entry::new();
        input_entry.set_placeholder_text(Some("Aicix'e bir şey sor…  Örnek: 'bana aksiyon anime öner'"));
        input_entry.set_hexpand(true);
        input_entry.add_css_class("aicix-input");
        input_box.append(&input_entry);

        let send_btn = gtk::Button::from_icon_name("mail-send-symbolic");
        send_btn.add_css_class("suggested-action");
        send_btn.add_css_class("circular");
        send_btn.add_css_class("aicix-send-btn");
        send_btn.set_tooltip_text(Some("Gönder"));
        send_btn.set_size_request(40, 40);
        input_box.append(&send_btn);

        root.append(&input_box);

        let client = aicix::AicixClient::new(state.clone());

        let panel = Self {
            root,
            message_list: messages_container.clone(),
            input_entry: input_entry.clone(),
            send_btn,
            state: state.clone(),
            client,
            messages_container,
            status_label: status_label.clone(),
            streaming_assistant_label: Rc::new(RefCell::new(None)),
            streaming_acc: Rc::new(RefCell::new(String::new())),
        };

        let state_for_new = state.clone();
        new_btn.connect_clicked(move |_| {
            state_for_new.lock().unwrap().clear_history();
        });

        panel.refresh_messages();
        panel
    }

    pub fn refresh_messages(&self) {
        while let Some(child) = self.messages_container.first_child() {
            self.messages_container.remove(&child);
        }
        *self.streaming_assistant_label.borrow_mut() = None;
        *self.streaming_acc.borrow_mut() = String::new();
        let state = self.state.lock().unwrap();
        if state.history.is_empty() {
            let empty = gtk::Label::new(Some("Merhaba! Aicix'e bir şeyler sor. Örnekler:\n\n• \"bana aksiyon anime öner\"\n• \"one piece kaç bölüm\"\n• \"naruto aç\"\n• \"bocchi the rock fansubları\""));
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

    pub fn make_bubble(msg: &aicix::ChatMessage) -> gtk::Widget {
        let wrap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        wrap.set_halign(match msg.role {
            aicix::MessageRole::User => gtk::Align::End,
            _ => gtk::Align::Start,
        });
        wrap.set_margin_start(8);
        wrap.set_margin_end(8);
        wrap.set_margin_top(4);
        wrap.set_margin_bottom(4);

        let bubble = gtk::Box::new(gtk::Orientation::Vertical, 4);
        bubble.set_halign(gtk::Align::Start);
        bubble.set_size_request(560, -1);

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

    pub fn append_user_message(&self, text: &str) {
        let msg = aicix::ChatMessage {
            role: aicix::MessageRole::User,
            content: text.to_string(),
            tool_call_id: None,
            tool_calls: None,
            name: None,
            is_card: false,
            card: None,
        };
        let bubble = Self::make_bubble(&msg);
        self.messages_container.append(&bubble);
        let scrolled_ancestor = self.messages_container.ancestor(gtk::ScrolledWindow::static_type());
        if let Some(anc) = scrolled_ancestor {
            if let Ok(sc) = anc.downcast::<gtk::ScrolledWindow>() {
                let adj = sc.vadjustment();
                adj.set_value(adj.upper() - adj.page_size());
            }
        }
    }

    pub fn ensure_streaming_assistant_bubble(&self) {
        let mut current = self.streaming_assistant_label.borrow_mut();
        if current.is_none() {
            let wrap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            wrap.set_halign(gtk::Align::Start);
            wrap.set_margin_start(8);
            wrap.set_margin_end(8);
            wrap.set_margin_top(4);
            wrap.set_margin_bottom(4);
            let bubble = gtk::Box::new(gtk::Orientation::Vertical, 4);
            bubble.set_halign(gtk::Align::Start);
            bubble.set_size_request(560, -1);
            bubble.add_css_class("aicix-bubble-bot");
            bubble.add_css_class("aicix-bubble-streaming");
            let label = gtk::Label::new(Some(""));
            label.set_wrap(true);
            label.set_xalign(0.0);
            label.set_yalign(0.0);
            label.set_selectable(true);
            label.set_margin_start(12);
            label.set_margin_end(12);
            label.set_margin_top(8);
            label.set_margin_bottom(8);
            bubble.append(&label);
            wrap.append(&bubble);
            self.messages_container.append(&wrap);
            *current = Some(label);
        }
    }

    pub fn append_streaming_chunk(&self, chunk: &str) {
        self.ensure_streaming_assistant_bubble();
        let mut acc = self.streaming_acc.borrow_mut();
        acc.push_str(chunk);
        if let Some(label) = self.streaming_assistant_label.borrow().as_ref() {
            label.set_text(&acc);
        }
    }

    pub fn finalize_streaming(&self, final_text: &str) {
        if let Some(label) = self.streaming_assistant_label.borrow_mut().take() {
            label.set_text(final_text);
        } else {
            let msg = aicix::ChatMessage {
                role: aicix::MessageRole::Assistant,
                content: final_text.to_string(),
                tool_call_id: None,
                tool_calls: None,
                name: None,
                is_card: false,
                card: None,
            };
            let bubble = Self::make_bubble(&msg);
            self.messages_container.append(&bubble);
        }
        self.streaming_acc.borrow_mut().clear();
    }

    pub fn clear_streaming(&self) {
        *self.streaming_assistant_label.borrow_mut() = None;
        self.streaming_acc.borrow_mut().clear();
    }

    pub fn update_status(&self, text: &str) {
        self.status_label.set_text(text);
    }
}
