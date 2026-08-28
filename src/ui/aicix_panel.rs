use gtk::prelude::*;
use adw::prelude::*;
use lucide_icons::Icon;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::aicix;
use crate::ui::icons;

const CSS: &str = r#"
.aicix-root { background: alpha(@card_bg_color, 0.0); }
.aicix-header {
    background: linear-gradient(135deg, #5e35b1 0%, #3949ab 100%);
    border-radius: 0;
    padding: 12px 16px;
    min-height: 56px;
}
.aicix-header-title {
    color: white;
    font-size: 18px;
    font-weight: 700;
}
.aicix-header-subtitle {
    color: alpha(white, 0.8);
    font-size: 11px;
}
.aicix-new-btn {
    background: alpha(white, 0.15);
    color: white;
    border: 1px solid alpha(white, 0.3);
    border-radius: 18px;
    padding: 6px 14px;
    font-weight: 500;
}
.aicix-new-btn:hover { background: alpha(white, 0.25); }
.aicix-new-btn:active { background: alpha(white, 0.35); }
.aicix-status {
    color: @dim_label;
    font-size: 11px;
    padding: 6px 16px;
    border-bottom: 1px solid alpha(@borders, 0.3);
}
.aicix-empty {
    color: @dim_label;
    font-size: 14px;
}
.aicix-empty-icon {
    color: alpha(@accent_color, 0.6);
    font-family: 'Lucide';
    font-size: 48px;
}
.aicix-bubble-user {
    background: linear-gradient(135deg, #3949ab 0%, #5e35b1 100%);
    color: white;
    border-radius: 18px 18px 4px 18px;
    padding: 10px 14px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.12);
}
.aicix-bubble-bot {
    background: alpha(@card_bg_color, 0.95);
    color: @window_fg_color;
    border-radius: 18px 18px 18px 4px;
    padding: 10px 14px;
    border: 1px solid alpha(@borders, 0.4);
    box-shadow: 0 1px 2px rgba(0,0,0,0.06);
}
.aicix-bubble-tool {
    background: alpha(@warning_bg_color, 0.15);
    color: @window_fg_color;
    border-radius: 12px;
    padding: 8px 12px;
    border-left: 3px solid @warning_color;
    font-family: monospace;
    font-size: 11px;
}
.aicix-bubble-system {
    background: alpha(@error_bg_color, 0.15);
    color: @window_fg_color;
    border-radius: 12px;
    padding: 8px 12px;
    font-style: italic;
}
.aicix-bubble-streaming {
    border: 1px solid @accent_color;
}
.aicix-bubble-content {
    color: inherit;
    font-size: 14px;
    line-height: 1.45;
    font-weight: 400;
}
.aicix-bubble-user .aicix-bubble-content { color: white; }
.aicix-avatar {
    background: linear-gradient(135deg, #5e35b1 0%, #3949ab 100%);
    color: white;
    border-radius: 18px;
    min-width: 32px;
    min-height: 32px;
    font-family: 'Lucide';
    font-size: 16px;
    margin-top: 4px;
}
.aicix-avatar-user {
    background: linear-gradient(135deg, #00838f 0%, #00695c 100%);
    border-radius: 18px;
    min-width: 32px;
    min-height: 32px;
    font-family: 'Lucide';
    font-size: 16px;
    color: white;
    margin-top: 4px;
}
.aicix-input-row {
    background: alpha(@card_bg_color, 0.95);
    border: 1px solid alpha(@borders, 0.5);
    border-radius: 22px;
    padding: 4px 6px 4px 14px;
    margin: 8px 12px 12px 12px;
    box-shadow: 0 1px 4px rgba(0,0,0,0.08);
}
.aicix-input {
    background: transparent;
    border: none;
    box-shadow: none;
    padding: 8px 4px;
    font-size: 14px;
    min-height: 36px;
}
.aicix-input:focus { box-shadow: none; outline: none; }
.aicix-send-btn {
    background: linear-gradient(135deg, #5e35b1 0%, #3949ab 100%);
    color: white;
    border: none;
    border-radius: 18px;
    min-width: 36px;
    min-height: 36px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.15);
}
.aicix-send-btn:hover { background: linear-gradient(135deg, #6d44c6 0%, #4757c0 100%); }
.aicix-send-btn:active { background: linear-gradient(135deg, #4a2890 0%, #2c3990 100%); }
.aicix-fab {
    background: linear-gradient(135deg, #5e35b1 0%, #3949ab 100%);
    color: white;
    border: none;
    box-shadow: 0 4px 12px rgba(57, 73, 171, 0.45);
    transition: all 200ms ease;
}
.aicix-fab:hover {
    background: linear-gradient(135deg, #6d44c6 0%, #4757c0 100%);
    box-shadow: 0 6px 16px rgba(57, 73, 171, 0.55);
    transform: scale(1.05);
}
.aicix-fab:active { transform: scale(0.95); }
.aicix-card {
    background: alpha(@card_bg_color, 0.95);
    border-radius: 12px;
    border: 1px solid alpha(@borders, 0.4);
    padding: 10px 12px;
    margin: 4px 0;
}
.aicix-card-title {
    font-weight: 600;
    font-size: 14px;
    color: @window_fg_color;
}
.aicix-card-meta {
    font-size: 11px;
    color: @dim_label;
}
"#;

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
        install_css();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("aicix-root");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.add_css_class("aicix-header");
        header.set_hexpand(true);
        header.set_valign(gtk::Align::Center);

        let header_icon = icons::lucide_label(Icon::Sparkles, 22);
        header_icon.set_margin_end(4);
        header.append(&header_icon);

        let title_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        title_box.set_hexpand(true);
        let title = gtk::Label::new(Some("Aicix"));
        title.set_xalign(0.0);
        title.add_css_class("aicix-header-title");
        title_box.append(&title);
        let subtitle = gtk::Label::new(Some("Groq · qwen/qwen3.8-27b · BYOK"));
        subtitle.set_xalign(0.0);
        subtitle.add_css_class("aicix-header-subtitle");
        title_box.append(&subtitle);
        header.append(&title_box);

        let new_btn = icons::lucide_button(Icon::Trash2, Some("Yeni"), 14);
        new_btn.set_tooltip_text(Some("Yeni sohbet"));
        new_btn.add_css_class("aicix-new-btn");
        header.append(&new_btn);

        root.append(&header);

        let status_label = gtk::Label::new(Some("Mesaj yazıp Enter'a bas — Aicix Türkçe yanıt verir."));
        status_label.set_xalign(0.0);
        status_label.add_css_class("aicix-status");
        status_label.set_wrap(true);
        root.append(&status_label);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        scrolled.set_vscrollbar_policy(gtk::PolicyType::Automatic);

        let messages_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        messages_container.set_margin_top(12);
        messages_container.set_margin_bottom(8);
        messages_container.set_margin_start(12);
        messages_container.set_margin_end(12);
        scrolled.set_child(Some(&messages_container));
        root.append(&scrolled);

        let input_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        input_row.set_valign(gtk::Align::Center);
        input_row.add_css_class("aicix-input-row");

        let input_entry = gtk::Entry::new();
        input_entry.set_placeholder_text(Some("Aicix'e bir şey sor…  'bana aksiyon anime öner'"));
        input_entry.set_hexpand(true);
        input_entry.add_css_class("aicix-input");
        input_row.append(&input_entry);

        let send_btn = icons::lucide_button(Icon::Send, None, 16);
        send_btn.set_tooltip_text(Some("Gönder (Enter)"));
        send_btn.add_css_class("aicix-send-btn");
        input_row.append(&send_btn);

        root.append(&input_row);

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
        let new_btn_clone = new_btn.clone();
        let panel_for_new = panel.root.clone();
        new_btn.connect_clicked(move |_| {
            state_for_new.lock().unwrap().clear_history();
            let _ = panel_for_new;
            let _ = new_btn_clone;
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
            let empty_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
            empty_box.set_valign(gtk::Align::Center);
            empty_box.set_vexpand(true);
            empty_box.set_margin_top(60);
            empty_box.set_margin_bottom(60);
            empty_box.set_halign(gtk::Align::Center);
            let empty_icon = icons::lucide_label(Icon::MessageCircleMore, 48);
            empty_icon.add_css_class("aicix-empty-icon");
            empty_box.append(&empty_icon);
            let empty_title = gtk::Label::new(Some("Merhaba!"));
            empty_title.set_xalign(0.5);
            empty_title.add_css_class("title-2");
            empty_box.append(&empty_title);
            let empty_body = gtk::Label::new(Some("Aicix'e bir şeyler sor. Örnekler:\n  • \"bana aksiyon anime öner\"\n  • \"one piece kaç bölüm\"\n  • \"naruto aç\"\n  • \"bocchi the rock fansubları\""));
            empty_body.set_xalign(0.5);
            empty_body.set_yalign(0.0);
            empty_body.set_wrap(true);
            empty_body.set_max_width_chars(45);
            empty_body.add_css_class("aicix-empty");
            empty_box.append(&empty_body);
            self.messages_container.append(&empty_box);
            return;
        }
        for msg in state.history.iter() {
            let bubble = Self::make_bubble(msg);
            self.messages_container.append(&bubble);
        }
    }

    pub fn make_bubble(msg: &aicix::ChatMessage) -> gtk::Widget {
        let is_user = msg.role == aicix::MessageRole::User;
        let wrap = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        wrap.set_halign(if is_user { gtk::Align::End } else { gtk::Align::Start });
        wrap.set_margin_top(2);
        wrap.set_margin_bottom(2);

        if !is_user {
            let avatar = icons::lucide_label(Icon::Bot, 18);
            avatar.add_css_class("aicix-avatar");
            avatar.set_valign(gtk::Align::Start);
            wrap.append(&avatar);
        }

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
        label.set_max_width_chars(80);
        label.add_css_class("aicix-bubble-content");
        bubble.append(&label);

        match msg.role {
            aicix::MessageRole::User => bubble.add_css_class("aicix-bubble-user"),
            aicix::MessageRole::Assistant => bubble.add_css_class("aicix-bubble-bot"),
            aicix::MessageRole::Tool => bubble.add_css_class("aicix-bubble-tool"),
            aicix::MessageRole::System => bubble.add_css_class("aicix-bubble-system"),
        }
        wrap.append(&bubble);

        if is_user {
            let avatar = icons::lucide_label(Icon::User, 18);
            avatar.add_css_class("aicix-avatar-user");
            avatar.set_valign(gtk::Align::Start);
            wrap.append(&avatar);
        }
        wrap.upcast::<gtk::Widget>()
    }

    pub fn ensure_streaming_assistant_bubble(&self) {
        let mut current = self.streaming_assistant_label.borrow_mut();
        if current.is_none() {
            let wrap = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            wrap.set_halign(gtk::Align::Start);
            wrap.set_margin_top(2);
            wrap.set_margin_bottom(2);
            let avatar = icons::lucide_label(Icon::Bot, 18);
            avatar.add_css_class("aicix-avatar");
            wrap.append(&avatar);
            let bubble = gtk::Box::new(gtk::Orientation::Vertical, 4);
            bubble.set_halign(gtk::Align::Start);
            bubble.set_size_request(360, -1);
            bubble.add_css_class("aicix-bubble-bot");
            bubble.add_css_class("aicix-bubble-streaming");
            let label = gtk::Label::new(Some(""));
            label.set_wrap(true);
            label.set_xalign(0.0);
            label.set_yalign(0.0);
            label.set_selectable(true);
            label.set_max_width_chars(80);
            label.add_css_class("aicix-bubble-content");
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

fn install_css() {
    if let Some(display) = gtk::gdk::Display::default() {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
