mod api;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use adw::prelude::*;

use api::{Client, Episode, HistoryEntry, Title};

#[derive(Clone)]
enum Page {
    Home,
    Category(usize),
    Search,
    Saved,
    History,
    Episodes { title: Title, eps: Vec<Episode> },
    Movie { title: Title, eps: Vec<Episode> },
}

enum Msg {
    Cats(Result<Vec<api::Category>, String>),
    Search(Result<Vec<Title>, String>),
    Eps(Title, Result<Vec<Episode>, String>),
    Play(Title, Episode, Result<String, String>),
}

struct App {
    window: adw::ApplicationWindow,
    list: gtk::ListBox,
    back_btn: gtk::Button,
    title_label: gtk::Label,
    search_entry: gtk::Entry,
    loading: gtk::Box,
    loading_spin: gtk::Spinner,
    toast: adw::ToastOverlay,
    client: std::sync::Arc<Client>,
    stack: Rc<RefCell<Vec<Page>>>,
    cats: Rc<RefCell<Vec<api::Category>>>,
    search_results: Rc<RefCell<Vec<Title>>>,
    cover_cache: Rc<RefCell<HashMap<String, Option<gtk::gdk::Texture>>>>,
    cover_bytes: Rc<RefCell<HashMap<String, Option<Vec<u8>>>>>,
    cover_waiters: Rc<RefCell<HashMap<String, Vec<gtk::Picture>>>>,
    cover_queue: Arc<Mutex<VecDeque<String>>>,
    cover_busy: Rc<Cell<bool>>,
    loading_gen: Rc<Cell<u32>>,
    fav_btn: gtk::Button,
    hist_btn: gtk::Button,
    home_acts: Rc<RefCell<Vec<Option<usize>>>>,
}

impl App {
    fn new(app: &adw::Application) -> Rc<Self> {
        let header = adw::HeaderBar::new();
        let title_label = gtk::Label::new(Some("AnimeciX"));
        title_label.add_css_class("title-2");
        header.set_title_widget(Some(&title_label));

        let back_btn = gtk::Button::from_icon_name("go-previous");
        back_btn.set_visible(false);
        back_btn.set_tooltip_text(Some("Geri"));
        header.pack_start(&back_btn);

        let search_entry = gtk::Entry::new();
        search_entry.set_placeholder_text(Some("Anime veya film ara…"));
        search_entry.set_width_chars(24);

        let fav_btn = gtk::Button::new();
        fav_btn.add_css_class("flat");
        let fav_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        fav_box.append(&gtk::Image::from_icon_name("starred-symbolic"));
        fav_box.append(&gtk::Label::new(Some("Favoriler")));
        fav_btn.set_child(Some(&fav_box));

        let hist_btn = gtk::Button::new();
        hist_btn.add_css_class("flat");
        let hist_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        hist_box.append(&gtk::Image::from_icon_name("document-open-recent-symbolic"));
        hist_box.append(&gtk::Label::new(Some("Geçmiş")));
        hist_btn.set_child(Some(&hist_box));

        header.pack_end(&search_entry);

        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.set_halign(gtk::Align::End);
        toolbar.set_margin_top(8);
        toolbar.set_margin_bottom(2);
        toolbar.set_margin_start(14);
        toolbar.set_margin_end(14);
        toolbar.append(&fav_btn);
        toolbar.append(&hist_btn);

        let window = adw::ApplicationWindow::new(app);
        window.set_default_size(980, 660);
        window.set_title(Some("AnimeciX"));
        gtk::Window::set_default_icon_name("tr.com.animecix");

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(true);
        list.set_vexpand(true);
        list.set_hexpand(true);

        let loading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        loading.set_halign(gtk::Align::Center);
        loading.set_valign(gtk::Align::End);
        loading.set_margin_bottom(28);
        loading.set_css_classes(&["osd-pill"]);
        loading.set_visible(false);
        let spin_big = gtk::Spinner::new();
        spin_big.set_size_request(18, 18);
        let loading_label = gtk::Label::new(Some("Yükleniyor…"));
        loading.append(&spin_big);
        loading.append(&loading_label);

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&list));
        overlay.add_overlay(&loading);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scrolled.set_child(Some(&overlay));
        scrolled.set_vexpand(true);
        scrolled.set_hexpand(true);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&header);
        content.append(&toolbar);
        content.append(&scrolled);

        let toast = adw::ToastOverlay::new();
        toast.set_child(Some(&content));
        window.set_content(Some(&toast));

        let app = Rc::new(Self {
            window,
            list,
            back_btn,
            title_label,
            search_entry,
            loading,
            loading_spin: spin_big,
            toast,
            client: std::sync::Arc::new(Client::new()),
            stack: Rc::new(RefCell::new(vec![Page::Home])),
            cats: Rc::new(RefCell::new(Vec::new())),
            search_results: Rc::new(RefCell::new(Vec::new())),
            cover_cache: Rc::new(RefCell::new(HashMap::new())),
            cover_bytes: Rc::new(RefCell::new(HashMap::new())),
            cover_waiters: Rc::new(RefCell::new(HashMap::new())),
            cover_queue: Arc::new(Mutex::new(VecDeque::new())),
            cover_busy: Rc::new(Cell::new(false)),
            loading_gen: Rc::new(Cell::new(0)),
            fav_btn,
            hist_btn,
            home_acts: Rc::new(RefCell::new(Vec::new())),
        });

        app.chain_signals();
        app.fetch_home();
        app
    }

    fn clone_ref(&self) -> Rc<Self> {
        Rc::new(Self {
            window: self.window.clone(),
            list: self.list.clone(),
            back_btn: self.back_btn.clone(),
            title_label: self.title_label.clone(),
            search_entry: self.search_entry.clone(),
            loading: self.loading.clone(),
            loading_spin: self.loading_spin.clone(),
            toast: self.toast.clone(),
            client: self.client.clone(),
            stack: self.stack.clone(),
            cats: self.cats.clone(),
            search_results: self.search_results.clone(),
            cover_cache: self.cover_cache.clone(),
            cover_bytes: self.cover_bytes.clone(),
            cover_waiters: self.cover_waiters.clone(),
            cover_queue: self.cover_queue.clone(),
            cover_busy: self.cover_busy.clone(),
            loading_gen: self.loading_gen.clone(),
            fav_btn: self.fav_btn.clone(),
            hist_btn: self.hist_btn.clone(),
            home_acts: self.home_acts.clone(),
        })
    }

    fn chain_signals(&self) {
        let this = self.clone_ref();
        self.back_btn.connect_clicked(move |_| {
            let mut st = this.stack.borrow_mut();
            if st.len() > 1 {
                st.pop();
            }
            let top = st.last().unwrap().clone();
            drop(st);
            this.show_page(&top);
        });

        let this = self.clone_ref();
        self.search_entry.connect_activate(move |e| {
            let q = e.text().to_string();
            this.do_search(q);
        });

        let this = self.clone_ref();
        self.fav_btn.connect_clicked(move |_| {
            let mut st = this.stack.borrow_mut();
            if !matches!(st.last(), Some(Page::Saved)) {
                st.push(Page::Saved);
                let page = st.last().unwrap().clone();
                drop(st);
                this.show_page(&page);
            }
        });

        let this = self.clone_ref();
        self.hist_btn.connect_clicked(move |_| {
            let mut st = this.stack.borrow_mut();
            if !matches!(st.last(), Some(Page::History)) {
                st.push(Page::History);
                let page = st.last().unwrap().clone();
                drop(st);
                this.show_page(&page);
            }
        });

        let this = self.clone_ref();
        self.list.connect_row_activated(move |_, row| {
            let idx = row.index() as usize;
            let top = this.stack.borrow().last().unwrap().clone();
            this.activate(&top, idx);
        });
    }

    // ---- görünüm yardımcıları ----

    fn clear(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
    }

    fn busy(&self, on: bool) {
        self.loading.set_visible(on);
        let gen = self.loading_gen.get() + 1;
        self.loading_gen.set(gen);
        if on {
            self.loading_spin.start();
            let this = self.clone_ref();
            glib::timeout_add_local(std::time::Duration::from_secs(8), move || {
                if this.loading_gen.get() == gen {
                    this.loading.set_visible(false);
                    this.loading_spin.stop();
                }
                glib::ControlFlow::Break
            });
        } else {
            self.loading_spin.stop();
        }
    }

    /// Kapak yükleyici: aynı URL bir kez indirilir, texture paylaşılır.
    /// İndirmeler kuyruktan sırayla alınır; biri bitince diğeri başlar.
    /// Resim ekran boyutuna ölçeklenir (w×h), böylece doğal boyutu büyük
    /// kalmayıp satırların gerilmesine yol açmaz.
    fn load_cover(&self, url: Option<&str>, pic: &gtk::Picture, w: i32, h: i32) {
        let Some(url) = url else { return };
        let url = url.replace("image.tmdb.org/t/p/original", "image.tmdb.org/t/p/w342");
        let key = format!("{url}@{w}x{h}");
        if let Some(Some(t)) = self.cover_cache.borrow().get(&key) {
            pic.set_paintable(Some(t));
            return;
        }
        if let Some(None) = self.cover_cache.borrow().get(&key) {
            return;
        }
        if let Some(Some(bytes)) = self.cover_bytes.borrow().get(&url) {
            if let Some(t) = Self::scale_texture(bytes, w, h) {
                pic.set_paintable(Some(&t));
                self.cover_cache.borrow_mut().insert(key, Some(t));
            }
            return;
        }
        if let Some(None) = self.cover_bytes.borrow().get(&url) {
            return;
        }
        self.cover_waiters
            .borrow_mut()
            .entry(key)
            .or_default()
            .push(pic.clone());
        let mut q = self.cover_queue.lock().unwrap();
        if !q.iter().any(|u| u == &url) {
            q.push_back(url);
        }
        drop(q);
        if !self.cover_busy.replace(true) {
            self.pump_covers();
        }
    }

    fn scale_texture(bytes: &[u8], w: i32, h: i32) -> Option<gtk::gdk::Texture> {
        let loader = gdk_pixbuf::PixbufLoader::new();
        loader.write(bytes).ok()?;
        loader.close().ok()?;
        let src = loader.pixbuf()?;
        let pb = src.scale_simple(w, h, gdk_pixbuf::InterpType::Bilinear)?;
        Some(gtk::gdk::Texture::for_pixbuf(&pb))
    }

    fn pump_covers(&self) {
        let url = self.cover_queue.lock().unwrap().pop_front();
        let Some(url) = url else {
            self.cover_busy.set(false);
            return;
        };
        let client = self.client.clone();
        let url2 = url.clone();
        let (tx, rx) = std::sync::mpsc::channel::<(String, Option<Vec<u8>>)>();
        std::thread::spawn(move || {
            let _ = tx.send((url2, client.get_bytes(&url)));
        });
        let this = self.clone_ref();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok((u, bytes)) => {
                this.finish_cover(&u, bytes);
                this.pump_covers();
                glib::ControlFlow::Break
            }
            Err(_) => glib::ControlFlow::Continue,
        });
    }

    fn finish_cover(&self, url: &str, bytes: Option<Vec<u8>>) {
        if let Some(b) = &bytes {
            let waiters = self.cover_waiters.borrow();
            for (key, pics) in waiters.iter() {
                let Some(rest) = key.strip_prefix(url) else { continue };
                let Some(rest) = rest.strip_prefix('@') else { continue };
                let Some((ws, hs)) = rest.split_once('x') else { continue };
                let (Ok(w), Ok(h)) = (ws.parse::<i32>(), hs.parse::<i32>()) else { continue };
                if let Some(t) = Self::scale_texture(b, w, h) {
                    for p in pics {
                        p.set_paintable(Some(&t));
                    }
                    self.cover_cache.borrow_mut().insert(key.clone(), Some(t));
                }
            }
            drop(waiters);
            self.cover_bytes.borrow_mut().insert(url.to_string(), bytes.clone());
        } else {
            self.cover_bytes.borrow_mut().insert(url.to_string(), None);
        }
    }

    /// kapak yer tutucusu: yüklenene kadar gri kutu, sonra resim
    fn cover_picture(&self, url: Option<&str>, w: i32, h: i32) -> gtk::Picture {
        let pic = gtk::Picture::new();
        pic.set_width_request(w);
        pic.set_height_request(h);
        pic.set_content_fit(gtk::ContentFit::Cover);
        pic.set_css_classes(&["cover"]);
        self.load_cover(url, &pic, w, h);
        pic
    }

    /// kaydetme düğmesi: durumu state.json'da, ikonu starred/non-starred
    fn bookmark_button(&self, t: &Title) -> gtk::Button {
        let saved = self.client.is_saved(t.id);
        let b = gtk::Button::from_icon_name(if saved {
            "starred-symbolic"
        } else {
            "non-starred-symbolic"
        });
        b.add_css_class("flat");
        b.add_css_class("circular");
        b.add_css_class("lg-icon");
        b.add_css_class("bookmark-btn");
        b.set_tooltip_text(Some("Kaydet"));
        let t2 = t.clone();
        let this = self.clone_ref();
        b.connect_clicked(move |b| {
            let on = this.client.toggle_saved(&t2);
            b.set_icon_name(if on {
                "starred-symbolic"
            } else {
                "non-starred-symbolic"
            });
            let toast = adw::Toast::new(if on {
                "★ Kaydedildi"
            } else {
                "Kayıtlardan çıkarıldı"
            });
            toast.set_timeout(2);
            this.toast.add_toast(toast);
            if matches!(this.stack.borrow().last(), Some(Page::Saved)) {
                this.show_page(&Page::Saved);
            }
        });
        b
    }

    /// Netflix tarzı kart: kapak + kaydet düğmesi + isim (+ alt yazı)
    fn title_card(&self, t: &Title, sub: Option<&str>) -> gtk::Box {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
        card.set_valign(gtk::Align::Start);

        let pic = self.cover_picture(t.poster.as_deref(), 128, 192);
        let btn = gtk::Button::new();
        btn.add_css_class("flat");
        btn.set_child(Some(&pic));
        let t2 = t.clone();
        let this = self.clone_ref();
        btn.connect_clicked(move |_| this.open_episodes(t2.clone()));

        let book = self.bookmark_button(t);
        book.set_halign(gtk::Align::End);
        book.set_valign(gtk::Align::Start);
        book.set_margin_top(4);
        book.set_margin_end(4);

        let over = gtk::Overlay::new();
        over.set_child(Some(&btn));
        over.add_overlay(&book);

        let name = gtk::Label::new(Some(&t.name));
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(16);
        card.append(&over);
        card.append(&name);
        if let Some(s) = sub {
            let l = gtk::Label::new(Some(s));
            l.set_xalign(0.0);
            l.add_css_class("dim-label");
            l.set_max_width_chars(16);
            l.set_ellipsize(gtk::pango::EllipsizeMode::End);
            card.append(&l);
        }
        card
    }

    /// yatay kaydırmalı raf; başlık satırına tıklanırsa kategori sayfası açılır
    fn add_shelf(&self, name: &str, items: Vec<(&Title, Option<String>)>, cat_idx: Option<usize>) {
        let head = gtk::Label::new(Some(name));
        head.set_xalign(0.0);
        head.add_css_class("heading");
        head.add_css_class("dim-label");
        head.set_margin_top(16);
        head.set_margin_start(14);
        head.set_margin_end(14);
        self.list.append(&head);
        self.home_acts.borrow_mut().push(cat_idx);

        let sw = gtk::ScrolledWindow::new();
        sw.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        sw.set_has_frame(false);
        let boxh = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        boxh.set_margin_top(8);
        boxh.set_margin_bottom(6);
        boxh.set_margin_start(14);
        boxh.set_margin_end(14);
        boxh.set_valign(gtk::Align::Start);
        for (t, sub) in &items {
            boxh.append(&self.title_card(t, sub.as_deref()));
        }
        sw.set_child(Some(&boxh));
        self.list.append(&sw);
        self.home_acts.borrow_mut().push(None);
    }

    fn add_title_row(&self, t: &Title) {
        let name = gtk::Label::new(Some(&t.name));
        name.set_xalign(0.0);
        name.set_wrap(true);
        name.add_css_class("title-4");

        let sub = gtk::Label::new(Some(&t.year.map(|y| y.to_string()).unwrap_or_default()));
        sub.set_xalign(0.0);
        sub.add_css_class("dim-label");

        let pic = self.cover_picture(t.poster.as_deref(), 48, 72);
        pic.set_valign(gtk::Align::Center);

        let right = gtk::Box::new(gtk::Orientation::Vertical, 4);
        right.set_vexpand(true);
        right.set_hexpand(true);
        right.set_valign(gtk::Align::Center);
        right.append(&name);
        right.append(&sub);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(8);
        row.set_margin_bottom(8);
        row.set_margin_start(14);
        row.set_margin_end(14);
        row.set_valign(gtk::Align::Center);
        row.append(&pic);
        row.append(&right);
        let book = self.bookmark_button(t);
        book.set_valign(gtk::Align::Center);
        row.append(&book);
        self.list.append(&row);
    }

    fn add_episode_row(&self, e: &Episode, poster: Option<&String>) {
        let name = gtk::Label::new(Some(&format!(
            "S{:02} E{:02}   {}",
            e.season, e.episode, e.name
        )));
        name.set_xalign(0.0);
        name.add_css_class("title-4");

        let pic = self.cover_picture(poster.map(|s| s.as_str()), 48, 72);
        pic.set_valign(gtk::Align::Center);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_margin_start(14);
        row.set_margin_end(14);
        row.set_valign(gtk::Align::Center);
        row.append(&pic);
        row.append(&name);
        self.list.append(&row);
    }

    fn add_history_row(&self, h: &HistoryEntry) {
        let name = gtk::Label::new(Some(&h.title.name));
        name.set_xalign(0.0);
        name.set_wrap(true);
        name.add_css_class("title-4");

        let sub = gtk::Label::new(Some(&format!(
            "S{:02} E{:02} · {}",
            h.episode.season, h.episode.episode, h.episode.name
        )));
        sub.set_xalign(0.0);
        sub.add_css_class("dim-label");

        let pic = self.cover_picture(h.title.poster.as_deref(), 48, 72);
        pic.set_valign(gtk::Align::Center);

        let right = gtk::Box::new(gtk::Orientation::Vertical, 4);
        right.set_vexpand(true);
        right.set_hexpand(true);
        right.set_valign(gtk::Align::Center);
        right.append(&name);
        right.append(&sub);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(8);
        row.set_margin_bottom(8);
        row.set_margin_start(14);
        row.set_margin_end(14);
        row.set_valign(gtk::Align::Center);
        row.append(&pic);
        row.append(&right);
        self.list.append(&row);
    }

    fn add_msg(&self, text: &str) {
        let l = gtk::Label::new(Some(text));
        l.set_xalign(0.0);
        l.add_css_class("dim-label");
        l.set_margin_top(20);
        l.set_margin_start(14);
        self.list.append(&l);
        if let Some(row) = self.list.last_child() {
            if let Ok(row) = row.downcast::<gtk::ListBoxRow>() {
                row.set_activatable(false);
                row.set_selectable(false);
            }
        }
    }

    fn show_page(&self, page: &Page) {
        self.clear();
        match page {
            Page::Home => {
                self.title_label.set_text("AnimeciX");
                self.back_btn.set_visible(self.stack.borrow().len() > 1);
                *self.home_acts.borrow_mut() = Vec::new();
                let cats = self.cats.borrow();
                if cats.is_empty() {
                    self.add_msg("Yükleniyor… ağa bağlı olduğundan emin ol.");
                } else {
                    for (i, c) in cats.iter().enumerate() {
                        let items: Vec<(&Title, Option<String>)> =
                            c.items.iter().map(|t| (t, None)).collect();
                        self.add_shelf(&c.name, items, Some(i));
                    }
                }
            }
            Page::Category(idx) => {
                let cats = self.cats.borrow();
                if let Some(cat) = cats.get(*idx) {
                    self.title_label.set_text(&cat.name);
                    self.back_btn.set_visible(true);
                    for t in cat.items.iter() {
                        self.add_title_row(t);
                    }
                }
            }
            Page::Search => {
                self.title_label.set_text("Arama Sonuçları");
                self.back_btn.set_visible(true);
                let results = self.search_results.borrow();
                if results.is_empty() {
                    self.add_msg("Sonuç yok.");
                } else {
                    for t in results.iter() {
                        self.add_title_row(t);
                    }
                }
            }
            Page::Saved => {
                self.title_label.set_text("Favoriler");
                self.back_btn.set_visible(true);
                let st = self.client.load_state();
                if st.saved.is_empty() {
                    self.add_msg("Henüz kayıt yok. Bir yıldıza tıklayarak dizi veya film ekleyebilirsin.");
                } else {
                    for t in st.saved.iter() {
                        self.add_title_row(t);
                    }
                }
            }
            Page::History => {
                self.title_label.set_text("Geçmiş");
                self.back_btn.set_visible(true);
                let st = self.client.load_state();
                if st.history.is_empty() {
                    self.add_msg("Henüz izlenen yok.");
                } else {
                    for h in st.history.iter() {
                        self.add_history_row(h);
                    }
                    let clear = gtk::Button::with_label("Geçmişi Temizle");
                    clear.add_css_class("destructive-action");
                    clear.set_halign(gtk::Align::Start);
                    clear.set_margin_top(16);
                    clear.set_margin_start(14);
                    clear.set_margin_end(14);
                    let this = self.clone_ref();
                    clear.connect_clicked(move |_| {
                        this.client.clear_history();
                        this.show_page(&Page::History);
                    });
                    self.list.append(&clear);
                    if let Some(row) = self.list.last_child() {
                        if let Ok(row) = row.downcast::<gtk::ListBoxRow>() {
                            row.set_activatable(false);
                            row.set_selectable(false);
                        }
                    }
                }
            }
            Page::Movie { title, eps } => {
                self.title_label.set_text(&title.name);
                self.back_btn.set_visible(true);
                let v = gtk::Box::new(gtk::Orientation::Vertical, 10);
                v.set_halign(gtk::Align::Center);
                v.set_hexpand(true);
                v.set_margin_top(28);
                v.set_margin_bottom(28);
                v.set_margin_start(24);
                v.set_margin_end(24);

                let cover = self.cover_picture(title.poster.as_deref(), 260, 390);
                cover.set_halign(gtk::Align::Center);
                v.append(&cover);

                let name = gtk::Label::new(Some(&title.name));
                name.add_css_class("title-1");
                name.set_halign(gtk::Align::Center);
                name.set_wrap(true);
                name.set_max_width_chars(46);
                v.append(&name);

                if let Some(y) = title.year {
                    let yl = gtk::Label::new(Some(&y.to_string()));
                    yl.add_css_class("dim-label");
                    yl.set_halign(gtk::Align::Center);
                    v.append(&yl);
                }
                if let Some(d) = &title.description {
                    let dl = gtk::Label::new(Some(d));
                    dl.add_css_class("dim-label");
                    dl.set_wrap(true);
                    dl.set_justify(gtk::Justification::Center);
                    dl.set_max_width_chars(72);
                    dl.set_halign(gtk::Align::Center);
                    v.append(&dl);
                }

                let watch = gtk::Button::with_label("▶ İzle");
                watch.add_css_class("suggested-action");
                watch.set_halign(gtk::Align::Center);
                watch.set_margin_top(10);
                let t2 = title.clone();
                let this = self.clone_ref();
                if let Some(e) = eps.first() {
                    let e = e.clone();
                    watch.connect_clicked(move |_| this.play(&t2, &e));
                } else {
                    watch.set_sensitive(false);
                    let no = gtk::Label::new(Some(
                        "Bu yapım için henüz kaynak eklenmemiş.",
                    ));
                    no.add_css_class("dim-label");
                    no.set_halign(gtk::Align::Center);
                    v.append(&no);
                }
                v.append(&watch);
                self.list.append(&v);
            }
            Page::Episodes { title, eps } => {
                self.title_label.set_text(&title.name);
                self.back_btn.set_visible(true);

                let pic = self.cover_picture(title.poster.as_deref(), 48, 72);
                let hname = gtk::Label::new(Some(&title.name));
                hname.set_xalign(0.0);
                hname.set_wrap(true);
                hname.add_css_class("title-4");
                let right = gtk::Box::new(gtk::Orientation::Vertical, 4);
                right.set_vexpand(true);
                right.set_hexpand(true);
                right.append(&hname);
                let hrow = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                hrow.set_margin_top(8);
                hrow.set_margin_bottom(8);
                hrow.set_margin_start(14);
                hrow.set_margin_end(14);
                hrow.set_valign(gtk::Align::Center);
                hrow.append(&pic);
                hrow.append(&right);
                let book = self.bookmark_button(title);
                book.set_valign(gtk::Align::Center);
                hrow.append(&book);
                self.list.append(&hrow);
                if let Some(r) = self.list.last_child() {
                    if let Ok(r) = r.downcast::<gtk::ListBoxRow>() {
                        r.set_activatable(false);
                        r.set_selectable(false);
                    }
                }

                for e in eps.iter() {
                    self.add_episode_row(e, title.poster.as_ref());
                }
            }
        }
    }

    fn activate(&self, page: &Page, idx: usize) {
        match page {
            Page::Home => {
                if let Some(Some(i)) = self.home_acts.borrow().get(idx) {
                    let mut st = self.stack.borrow_mut();
                    st.push(Page::Category(*i));
                    drop(st);
                    self.show_page(&Page::Category(*i));
                }
            }
            Page::Movie { .. } => {}
            Page::Saved => {
                let st = self.client.load_state();
                if let Some(t) = st.saved.get(idx) {
                    self.open_episodes(t.clone());
                }
            }
            Page::History => {
                let st = self.client.load_state();
                if let Some(h) = st.history.get(idx) {
                    self.open_episodes(h.title.clone());
                }
            }
            Page::Category(cat_idx) => {
                let cats = self.cats.borrow();
                if let Some(t) = cats.get(*cat_idx).and_then(|c| c.items.get(idx)) {
                    self.open_episodes(t.clone());
                }
            }
            Page::Search => {
                let t = self.search_results.borrow().get(idx).cloned();
                if let Some(t) = t {
                    self.open_episodes(t);
                }
            }
            Page::Episodes { title, eps } => {
                if let Some(e) = eps.get(idx.saturating_sub(1)) {
                    self.play(title, e);
                }
            }
        }
    }

    // ---- asenkron ----

    fn spawn<F, T>(&self, job: F)
    where
        F: FnOnce(&Client) -> T + Send + 'static,
        T: FnOnce() -> Msg + Send + 'static,
    {
        let client = self.client.clone();
        let (tx, rx) = std::sync::mpsc::channel::<T>();
        std::thread::spawn(move || {
            let f = job(&client);
            let _ = tx.send(f);
        });
        let this = self.clone_ref();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(f) => {
                this.on_msg(f());
                glib::ControlFlow::Break
            }
            Err(_) => glib::ControlFlow::Continue,
        });
    }

    fn on_msg(&self, msg: Msg) {
        self.busy(false);
        match msg {
            Msg::Cats(res) => match res {
                Ok(c) => {
                    *self.cats.borrow_mut() = c;
                    self.show_page(&Page::Home);
                }
                Err(e) => self.show_error(&e),
            },
            Msg::Search(res) => match res {
                Ok(r) => {
                    *self.search_results.borrow_mut() = r;
                    self.stack.borrow_mut().push(Page::Search);
                    self.show_page(&Page::Search);
                }
                Err(e) => self.show_error(&e),
            },
            Msg::Eps(title, res) => match res {
                Ok(eps) => {
                    let page = if eps.is_empty() {
                        Page::Movie { title, eps }
                    } else {
                        match title.title_type.as_deref() {
                            Some("movie") => Page::Movie { title, eps },
                            _ => Page::Episodes { title, eps },
                        }
                    };
                    self.stack.borrow_mut().push(page.clone());
                    self.show_page(&page);
                }
                Err(e) => self.show_error(&e),
            },
            Msg::Play(title, ep, res) => match res {
                Ok(url) => {
                    let w = api::Watched {
                        title_id: title.id,
                        episode: ep.episode,
                        season: ep.season,
                    };
                    self.client.save_watched(&w, &title.name);
                    self.client.add_history(&title, &ep);
                    let media_title = format!("{} | S{:02}E{:02}", title.name, ep.season, ep.episode);
                    let _ = Command::new("mpv")
                        .arg("--user-agent=mozilla")
                        .arg(format!("--force-media-title={media_title}"))
                        .arg("--keep-open=yes")
                        .arg(&url)
                        .spawn();
                    let t = adw::Toast::new(&format!("▶ {media_title} açılıyor…"));
                    t.set_timeout(2);
                    self.toast.add_toast(t);
                }
                Err(e) => self.show_error(&e),
            },
        }
    }

    fn open_episodes(&self, title: Title) {
        self.busy(true);
        self.spawn(move |c| {
            let res = c.episodes(&title);
            move || Msg::Eps(title.clone(), res)
        });
    }

    fn play(&self, title: &Title, ep: &Episode) {
        let tid = title.id;
        let epn = ep.episode;
        let sez = ep.season;
        let is_movie = title.title_type.as_deref() == Some("movie");
        let title = title.clone();
        let ep = ep.clone();
        self.busy(true);
        self.spawn(move |c| {
            let res = if is_movie {
                c.resolve_movie(tid)
            } else {
                c.resolve(tid, epn, sez)
            };
            move || Msg::Play(title.clone(), ep.clone(), res)
        });
    }

    fn do_search(&self, q: String) {
        let q = q.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.busy(true);
        self.spawn(move |c| {
            let res = c.search(&q);
            move || Msg::Search(res)
        });
    }

    fn fetch_home(&self) {
        self.busy(true);
        self.spawn(move |c| {
            let res = c.home_lists();
            move || Msg::Cats(res)
        });
    }

    fn show_error(&self, msg: &str) {
        eprintln!("animecix hatası: {msg}");
        self.add_msg(&format!("Hata: {msg}"));
    }
}

fn main() {
    let app = adw::Application::builder()
        .application_id("tr.com.animecix")
        .build();

    app.connect_activate(|app| {
        if let Some(display) = gtk::gdk::Display::default() {
            let mut base = std::path::PathBuf::new();
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    base = dir.to_path_buf();
                }
            }
            if let Ok(ad) = std::env::var("APPDIR") {
                if !ad.is_empty() {
                    base = std::path::PathBuf::from(ad);
                }
            }
            let theme = gtk::IconTheme::for_display(&display);
            theme.add_search_path(base.join("assets/hicolor"));
            theme.add_search_path(base.join("usr/share/icons/hicolor"));

            let css = gtk::CssProvider::new();
            css.load_from_string(
                ".cover { background-color: alpha(currentColor, 0.08); border-radius: 8px; }
                 .lg-icon { -gtk-icon-size: 26px; }
                 .bookmark-btn { background-color: alpha(black, 0.35); }
                 .osd-pill { background-color: alpha(black, 0.75); border-radius: 18px; padding: 8px 16px; }
                 .osd-pill label { color: white; }",
            );
            gtk::style_context_add_provider_for_display(
                &display,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let window = &App::new(app).window;
        window.present();
    });

    app.run();
}