use gtk::prelude::*;
use adw::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use gtk::gio;

use crate::api::{self, Client, Episode, State, Title};
use crate::covers::CoverManager;

use crate::ui::components;
use crate::ui::episodes_view;
use crate::ui::views;
use crate::{check_all_dependencies, check_desktop_entry_installed, install_desktop_entry};

#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Welcome,
    Home,
    Favs,
    Marathon,
    History,
    Settings,
    Search,
    Episodes { title: Title, eps: Vec<Episode> },
    Movie { title: Title, eps: Vec<Episode> },
}

pub enum Msg {
    Cats(Result<Vec<api::Category>, String>),
    Search(Result<Vec<Title>, String>),
    Eps(Title, Result<Vec<Episode>, String>),
    Play(Title, Episode, Result<Vec<String>, String>),
}

pub struct App {
    pub window: adw::ApplicationWindow,
    pub stack: gtk::Stack,
    pub back_btn: gtk::Button,
    pub search_toggle_btn: gtk::Button,
    pub fav_btn: gtk::Button,
    pub marathon_btn: gtk::Button,
    pub hist_btn: gtk::Button,
    pub settings_btn: gtk::Button,
    pub title_label: gtk::Label,
    pub search_bar: gtk::SearchBar,
    pub search_entry: gtk::SearchEntry,
    pub loading: gtk::Box,
    pub toast: adw::ToastOverlay,
    pub client: Arc<Client>,
    pub covers: CoverManager,
    pub page_history: Rc<RefCell<Vec<Page>>>,
    pub cats: Rc<RefCell<Vec<api::Category>>>,
    pub search_results: Rc<RefCell<Vec<Title>>>,
    pub settings: Rc<RefCell<api::Settings>>,
    pub progress: Rc<RefCell<HashMap<String, (f64, f64)>>>,
    pub progress_bars: Rc<RefCell<HashMap<String, (gtk::ProgressBar, gtk::Label)>>>,
    pub loading_toast: Rc<RefCell<Option<adw::Toast>>>,
    pub loading_gen: Rc<Cell<u32>>,
    pub home_acts: Rc<RefCell<Vec<Option<usize>>>>,
}

/// Çalışma anında Anime4K shader dosyasının yolunu bulur (AppImage'da APPDIR,
/// geliştirme ortamında exe'nin yakınları). Bulunamazsa None -> özellik pas geçilir.
fn resolve_upscale_shader(name: &str) -> Option<String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(ad) = std::env::var("APPDIR") {
        if !ad.is_empty() {
            candidates.push(std::path::Path::new(&ad).join("usr/share/animecix/assets/upscale").join(name));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("usr/share/animecix/assets/upscale").join(name));
            candidates.push(parent.join("assets/upscale").join(name));
            candidates.push(parent.join("../../assets/upscale").join(name));
        }
    }
    candidates.into_iter().find(|p| p.exists()).map(|p| p.to_string_lossy().into_owned())
}

impl App {
    pub fn new(app: &adw::Application) -> Rc<Self> {
        let client = Arc::new(Client::new());

        // Açılışta ağı GTK pencere kurulumuyla çakıştır: bağlantıyı ısıt (warmup) ve ana
        // sayfa JSON'unu önceden indir (prefetch). Böylece show_page(Home)+fetch_home() çağrıldığında
        // veri bellek önbelleğinde hazır olur; soğuk açılış ~2.3-3s'ye iner. Pencereyi bloklamaz.
        {
            let cl = client.clone();
            std::thread::spawn(move || {
                cl.warmup();
                let _ = cl.home_lists();
            });
        }
        let welcome_seen = client.is_welcome_seen();

        let header = adw::HeaderBar::new();
        let title_label = gtk::Label::new(Some("AnimeciX"));
        title_label.add_css_class("title-2");
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(28);
        header.set_title_widget(Some(&title_label));

        let back_btn = gtk::Button::from_icon_name("go-previous-symbolic");
        back_btn.add_css_class("flat");
        back_btn.add_css_class("circular");
        back_btn.set_tooltip_text(Some("Geri"));
        header.pack_start(&back_btn);

        let search_toggle_btn = gtk::Button::from_icon_name("system-search-symbolic");
        search_toggle_btn.add_css_class("flat");
        search_toggle_btn.add_css_class("circular");
        search_toggle_btn.set_tooltip_text(Some("Arama Yap"));
        header.pack_end(&search_toggle_btn);

        let nav_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let fav_btn = gtk::Button::with_label("Favoriler");
        fav_btn.add_css_class("flat");
        fav_btn.add_css_class("header-nav-btn");

        let marathon_btn = gtk::Button::with_label("Maraton");
        marathon_btn.add_css_class("flat");
        marathon_btn.add_css_class("header-nav-btn");

        let hist_btn = gtk::Button::with_label("Geçmiş");
        hist_btn.add_css_class("flat");
        hist_btn.add_css_class("header-nav-btn");

        let settings_btn = gtk::Button::with_label("Ayarlar");
        settings_btn.add_css_class("flat");
        settings_btn.add_css_class("header-nav-btn");

        nav_box.append(&fav_btn);
        nav_box.append(&marathon_btn);
        nav_box.append(&hist_btn);
        nav_box.append(&settings_btn);
        header.pack_end(&nav_box);

        let search_entry = gtk::SearchEntry::new();
        search_entry.set_placeholder_text(Some("Anime veya dizi ara…"));
        search_entry.set_hexpand(true);

        let search_bar = gtk::SearchBar::new();
        search_bar.set_child(Some(&search_entry));
        search_bar.connect_entry(&search_entry);
        search_bar.set_key_capture_widget(Some(&app.active_window().unwrap_or_default()));



        // Sayfa geçişleri için GTK Stack
        let main_stack = gtk::Stack::new();
        main_stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
        main_stack.set_transition_duration(220);
        main_stack.set_vexpand(true);
        main_stack.set_hexpand(true);

        let loading = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        loading.add_css_class("card");
        loading.set_halign(gtk::Align::Center);
        loading.set_valign(gtk::Align::Start);
        loading.set_margin_top(12);
        loading.set_margin_bottom(12);
        loading.set_margin_start(16);
        loading.set_margin_end(16);

        let spin = gtk::Spinner::new();
        spin.start();
        let l_lbl = gtk::Label::new(Some("Yükleniyor…"));
        l_lbl.add_css_class("title-4");
        loading.append(&spin);
        loading.append(&l_lbl);
        loading.set_visible(false);

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&main_stack));
        overlay.add_overlay(&loading);
        overlay.set_vexpand(true);
        overlay.set_hexpand(true);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&header);
        content.append(&search_bar);
        content.append(&overlay);

        let toast = adw::ToastOverlay::new();
        toast.set_child(Some(&content));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("AnimeciX")
            .default_width(980)
            .default_height(720)
            .resizable(false)
            .content(&toast)
            .build();

        let initial_page = if welcome_seen { Page::Home } else { Page::Welcome };

        let covers = CoverManager::new(client.clone());

        let app_inst = Rc::new(Self {
            window,
            stack: main_stack,
            back_btn,
            search_toggle_btn,
            title_label,
            search_bar,
            search_entry,
            loading,
            toast,
            client: client.clone(),
            covers,
            page_history: Rc::new(RefCell::new(vec![initial_page.clone()])),
            cats: Rc::new(RefCell::new(Vec::new())),
            search_results: Rc::new(RefCell::new(Vec::new())),
            settings: Rc::new(RefCell::new(client.load_settings())),
            progress: Rc::new(RefCell::new(client.load_state().progress)),
            progress_bars: Rc::new(RefCell::new(HashMap::new())),
            loading_toast: Rc::new(RefCell::new(None)),
            loading_gen: Rc::new(Cell::new(0)),
            fav_btn,
            marathon_btn,
            hist_btn,
            settings_btn,
            home_acts: Rc::new(RefCell::new(Vec::new())),
        });

        app_inst.chain_signals();
        app_inst.show_page(&initial_page);
        if welcome_seen {
            app_inst.fetch_home();
        }
        app_inst.apply_goto_arg();
        app_inst
    }

    pub fn clone_ref(&self) -> Rc<Self> {
        Rc::new(Self {
            window: self.window.clone(),
            stack: self.stack.clone(),
            back_btn: self.back_btn.clone(),
            search_toggle_btn: self.search_toggle_btn.clone(),
            title_label: self.title_label.clone(),
            search_bar: self.search_bar.clone(),
            search_entry: self.search_entry.clone(),
            loading: self.loading.clone(),
            toast: self.toast.clone(),
            client: self.client.clone(),
            covers: self.covers.clone_ref(),
            page_history: self.page_history.clone(),
            cats: self.cats.clone(),
            search_results: self.search_results.clone(),
            settings: self.settings.clone(),
            progress: self.progress.clone(),
            progress_bars: self.progress_bars.clone(),
            loading_toast: self.loading_toast.clone(),
            loading_gen: self.loading_gen.clone(),
            fav_btn: self.fav_btn.clone(),
            marathon_btn: self.marathon_btn.clone(),
            hist_btn: self.hist_btn.clone(),
            settings_btn: self.settings_btn.clone(),
            home_acts: self.home_acts.clone(),
        })
    }

    fn chain_signals(&self) {
        let this = self.clone_ref();
        self.search_toggle_btn.connect_clicked(move |_| {
            let active = !this.search_bar.is_search_mode();
            this.search_bar.set_search_mode(active);
            if active {
                this.search_entry.grab_focus();
                if !this.client.is_search_tip_seen() {
                    this.client.set_search_tip_seen(true);
                    let sc = this.settings.borrow().search_shortcut.clone();
                    let toast = adw::Toast::new(&format!(
                        "💡 '{}' kısayolu ile arama çubuğunu hızlıca açabilirsiniz!", sc
                    ));
                    toast.set_timeout(4);
                    this.toast.add_toast(toast);
                }
            }
        });

        let this = self.clone_ref();
        self.back_btn.connect_clicked(move |_| {
            let mut st = this.page_history.borrow_mut();
            if st.len() > 1 {
                st.pop();
                while st.len() > 1 && st.last() == st.get(st.len() - 2) {
                    st.pop();
                }
            }
            let top = st.last().cloned().unwrap_or(Page::Home);
            drop(st);
            this.show_page(&top);
        });

        let this = self.clone_ref();
        self.search_entry.connect_activate(move |e| {
            let q = e.text().to_string();
            if !q.trim().is_empty() {
                this.do_search(q);
            }
        });

        let this = self.clone_ref();
        self.fav_btn.connect_clicked(move |_| {
            let mut st = this.page_history.borrow_mut();
            if st.last() != Some(&Page::Favs) {
                st.push(Page::Favs);
            }
            drop(st);
            this.show_page(&Page::Favs);
        });

        let this = self.clone_ref();
        self.marathon_btn.connect_clicked(move |_| {
            let mut st = this.page_history.borrow_mut();
            if st.last() != Some(&Page::Marathon) {
                st.push(Page::Marathon);
            }
            drop(st);
            this.show_page(&Page::Marathon);
        });

        let this = self.clone_ref();
        self.hist_btn.connect_clicked(move |_| {
            let mut st = this.page_history.borrow_mut();
            if st.last() != Some(&Page::History) {
                st.push(Page::History);
            }
            drop(st);
            this.show_page(&Page::History);
        });

        let this = self.clone_ref();
        self.settings_btn.connect_clicked(move |_| {
            let mut st = this.page_history.borrow_mut();
            if st.last() != Some(&Page::Settings) {
                st.push(Page::Settings);
            }
            drop(st);
            this.show_page(&Page::Settings);
        });

        // === Global arama kısayolu (Ayardan okunur) ===
        {
            let this = self.clone_ref();
            let search_bar = self.search_bar.clone();
            let search_entry = self.search_entry.clone();
            let settings = self.settings.clone();
            let key_ctrl = gtk::EventControllerKey::new();
            key_ctrl.connect_key_pressed(move |_, keyval, _, state| {
                let sc = settings.borrow().search_shortcut.clone();
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();
                let is_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
                let triggered = match sc.as_str() {
                    "Ctrl+K" => is_ctrl && (key_name == "k" || key_name == "K"),
                    "F2" => key_name == "F2",
                    "/" => key_name == "slash" || key_name == "kp_divide",
                    _ => is_ctrl && (key_name == "s" || key_name == "S"), // Ctrl+S
                };
                if triggered {
                    search_bar.set_search_mode(true);
                    search_entry.grab_focus();

                    // Klavyeden kısayol basıldığında ipucu gösterilmez (zaten kısayol kullanılıyor)
                    if !this.client.is_search_tip_seen() {
                        this.client.set_search_tip_seen(true);
                    }
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            self.window.add_controller(key_ctrl);
        }
    }

    pub fn busy(&self, on: bool) {
        let gen = self.loading_gen.get() + 1;
        self.loading_gen.set(gen);
        self.loading.set_visible(false);

        if on {
            if let Some(t) = self.loading_toast.borrow_mut().take() {
                t.dismiss();
            }
            let t = adw::Toast::new("Yükleniyor…");
            t.set_timeout(0);
            self.toast.add_toast(t.clone());
            *self.loading_toast.borrow_mut() = Some(t);
        } else if let Some(t) = self.loading_toast.borrow_mut().take() {
            t.dismiss();
        }
    }

    pub fn show_page(&self, page: &Page) {
        use gtk::prelude::IsA;
        self.progress_bars.borrow_mut().clear();
        self.back_btn.set_sensitive(self.page_history.borrow().len() > 1);

        // Animasyonlu sayfa geçişi:
        // 1) Aynı isimli eski child varsa kaldır (duplicate engellemek için)
        // 2) Yeni child ekle ve görüntr — GTK Stack eski sayfa üzerinden animasyon yapar
        // 3) 300ms sonra (animasyon bittikten sonra) eski sayfaları temizle
        fn switch<T: IsA<gtk::Widget>>(
            stack: &gtk::Stack,
            name: &str,
            transition: gtk::StackTransitionType,
            widget: T,
        ) {
            // Aynı isimli child varsa önce kaldır (duplicate engellemek için)
            if let Some(old) = stack.child_by_name(name) {
                stack.remove(&old);
            }
            stack.set_transition_type(transition);
            stack.add_named(&widget, Some(name));
            stack.set_visible_child_name(name);

            // Animasyon bittikten sonra (>220ms) gizli kalan eski sayfaları temizle.
            // O AN görünen child'ı koru (hardcoded isim değil!) — hızlı geri tuşunda
            // başka bir cleanup'ın yeni sayfayı silmesini önler.
            let stack_c = stack.clone();
            glib::timeout_add_local_once(
                std::time::Duration::from_millis(400),
                move || {
                    // O an gerçekte görünen child
                    let Some(visible) = stack_c.visible_child() else { return; };
                    let mut to_rm = vec![];
                    let mut cur = stack_c.first_child();
                    while let Some(child) = cur {
                        let next = child.next_sibling();
                        if child != visible {
                            to_rm.push(child);
                        }
                        cur = next;
                    }
                    for c in to_rm {
                        stack_c.remove(&c);
                    }
                },
            );
        }

        match page {
            Page::Welcome => {
                self.title_label.set_text("Hoş Geldiniz");
                switch(&self.stack, "welcome", gtk::StackTransitionType::Crossfade, self.build_welcome_view());
            }
            Page::Home => {
                self.title_label.set_text("AnimeciX");
                switch(&self.stack, "home", gtk::StackTransitionType::Crossfade, self.build_home_view());
            }
            Page::Favs => {
                self.title_label.set_text("Favorilerim");
                switch(&self.stack, "favs", gtk::StackTransitionType::Crossfade, self.build_favs_view());
            }
            Page::Marathon => {
                self.title_label.set_text("İzleme Maratonum 🏃‍♂️");
                switch(&self.stack, "marathon", gtk::StackTransitionType::Crossfade, self.build_marathon_view());
            }
            Page::History => {
                self.title_label.set_text("İzleme Geçmişi");
                switch(&self.stack, "history", gtk::StackTransitionType::Crossfade, self.build_history_view());
            }
            Page::Settings => {
                self.title_label.set_text("Ayarlar");
                switch(&self.stack, "settings", gtk::StackTransitionType::Crossfade, self.build_settings_view());
            }
            Page::Search => {
                self.title_label.set_text("Arama Sonuçları");
                switch(&self.stack, "search", gtk::StackTransitionType::SlideLeft, self.build_search_view());
            }
            Page::Episodes { title, eps } | Page::Movie { title, eps } => {
                self.title_label.set_text(&title.name);
                let page_name = format!("eps_{}", title.id);
                switch(&self.stack, &page_name, gtk::StackTransitionType::SlideLeft, self.build_episodes_view(title, eps));
            }
        }
    }

    /// CLI: `--goto <sayfa>` ile başlangıçta belirli bir sayfayı açar.
    /// Ekran görüntüsü otomasyonu için kullanılır; AT-SPI/a11y'ye ihtiyaç duymaz.
    fn apply_goto_arg(&self) {
        let args: Vec<String> = std::env::args().collect();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if a == "--goto" {
                if let Some(val) = it.next() {
                    self.goto_page(val);
                }
            }
        }
    }

    fn goto_page(&self, val: &str) {
        self.page_history.borrow_mut().clear();
        match val {
            "welcome" => {
                self.page_history.borrow_mut().push(Page::Welcome);
                self.show_page(&Page::Welcome);
            }
            "home" => {
                self.page_history.borrow_mut().push(Page::Home);
                self.show_page(&Page::Home);
                self.fetch_home();
            }
            "favorites" => {
                self.page_history.borrow_mut().push(Page::Favs);
                self.show_page(&Page::Favs);
            }
            "marathon" => {
                self.page_history.borrow_mut().push(Page::Marathon);
                self.show_page(&Page::Marathon);
            }
            "history" => {
                self.page_history.borrow_mut().push(Page::History);
                self.show_page(&Page::History);
            }
            "settings" => {
                self.page_history.borrow_mut().push(Page::Settings);
                self.show_page(&Page::Settings);
            }
            "search" => {
                self.page_history.borrow_mut().push(Page::Home);
                self.show_page(&Page::Home);
                self.fetch_home();
                self.search_bar.set_search_mode(true);
                self.search_entry.set_text("Tokyo");
                let q = self.search_entry.text().to_string();
                self.do_search(q);
            }
            "episodes" => {
                self.page_history.borrow_mut().push(Page::Home);
                self.show_page(&Page::Home);
                self.fetch_home();
                let this = self.clone_ref();
                glib::timeout_add_local_once(std::time::Duration::from_millis(1800), move || {
                    let cats = this.cats.borrow();
                    let first = cats.iter().flat_map(|c| c.items.iter()).next().cloned();
                    drop(cats);
                    if let Some(t) = first {
                        this.open_episodes(t);
                    }
                });
            }
            _ => {}
        }
    }

    fn build_welcome_view(&self) -> gtk::ScrolledWindow {
        let scroll = gtk::ScrolledWindow::new();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
        root.set_margin_top(20);
        root.set_margin_bottom(24);
        root.set_margin_start(24);
        root.set_margin_end(24);

        // --- Başlık Bölümü ---
        let header_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        header_box.set_halign(gtk::Align::Center);

        let icon = gtk::Image::from_icon_name("tr.com.animecix");
        icon.set_pixel_size(84);
        header_box.append(&icon);

        let title = gtk::Label::new(Some("AnimeciX Masaüstü Kurulum & Kontrol Sihirbazı"));
        title.add_css_class("title-1");
        header_box.append(&title);

        let desc = gtk::Label::new(Some(
            "Uygulamayı kullanmaya başlamadan önce sistem bağımlılıklarını ve masaüstü entegrasyonunu kontrol edin."
        ));
        desc.add_css_class("dim-label");
        desc.set_wrap(true);
        desc.set_justify(gtk::Justification::Center);
        header_box.append(&desc);

        root.append(&header_box);

        // --- 1. Bağımlılık Kontrol Grubu ---
        let dep_group = adw::PreferencesGroup::new();
        dep_group.set_title("1. Sistem Bağımlılık Kontrolü");
        dep_group.set_description(Some("Uygulamanın sorunsuz çalışabilmesi için gerekli sistem araçları:"));

        let deps = check_all_dependencies();
        for dep in &deps {
            let row = adw::ActionRow::new();
            row.set_title(dep.name);
            row.set_subtitle(dep.desc);

            let status_badge = gtk::Label::new(None);
            status_badge.set_valign(gtk::Align::Center);
            if dep.installed {
                status_badge.set_markup("<span foreground='#2ec27e' weight='bold'>🟢 Yüklü</span>");
            } else {
                status_badge.set_markup("<span foreground='#e01b24' weight='bold'>🔴 Eksik</span>");
                if let Some(cmd) = &dep.install_cmd {
                    row.set_subtitle(&format!("{} • Kurulum: {}", dep.desc, cmd));
                }
            }
            row.add_suffix(&status_badge);
            dep_group.add(&row);
        }

        root.append(&dep_group);

        // --- 2. Masaüstü Entegrasyon Grubu ---
        let desktop_group = adw::PreferencesGroup::new();
        desktop_group.set_title("2. Masaüstü Uygulama Menüsü Entegrasyonu");
        desktop_group.set_description(Some("AnimeciX'i işletim sisteminizin uygulama başlatıcı menüsüne ekleyin:"));

        let desktop_row = adw::ActionRow::new();
        desktop_row.set_title("Masaüstü Menü Başlatıcısı (tr.com.animecix.desktop)");

        let is_installed = check_desktop_entry_installed();
        let desktop_status_lbl = gtk::Label::new(None);
        desktop_status_lbl.set_valign(gtk::Align::Center);

        let desktop_btn = gtk::Button::new();
        desktop_btn.set_valign(gtk::Align::Center);
        desktop_btn.add_css_class("pill");

        if is_installed {
            desktop_status_lbl.set_markup("<span foreground='#2ec27e' weight='bold'>🟢 Menüde Ekli</span>");
            desktop_row.set_subtitle("AnimeciX uygulama menünüzde hazır.");
            desktop_btn.set_label("Yeniden Entegre Et 📌");
            desktop_btn.add_css_class("flat");
        } else {
            desktop_status_lbl.set_markup("<span foreground='#f5c211' weight='bold'>🟡 Menüde Yok</span>");
            desktop_row.set_subtitle("Uygulama menüsüne eklemek için butona tıklayın.");
            desktop_btn.set_label("Uygulamalar Listesine Ekle 📌");
            desktop_btn.add_css_class("suggested-action");
        }

        let this_desk = self.clone_ref();
        let lbl_clone = desktop_status_lbl.clone();
        let row_clone = desktop_row.clone();
        let btn_clone = desktop_btn.clone();

        desktop_btn.connect_clicked(move |_| {
            match install_desktop_entry() {
                Ok(_) => {
                    lbl_clone.set_markup("<span foreground='#2ec27e' weight='bold'>🟢 Başarıyla Eklendi</span>");
                    row_clone.set_subtitle("AnimeciX masaüstü uygulama menüsüne eklendi!");
                    btn_clone.set_label("Yeniden Entegre Et 📌");
                    btn_clone.remove_css_class("suggested-action");
                    btn_clone.add_css_class("flat");

                    let toast = adw::Toast::new("📌 AnimeciX masaüstü uygulama menüsüne eklendi!");
                    toast.set_timeout(4);
                    this_desk.toast.add_toast(toast);
                }
                Err(e) => {
                    let toast = adw::Toast::new(&format!("⚠️ Masaüstü menüsüne eklenemedi: {e}"));
                    this_desk.toast.add_toast(toast);
                }
            }
        });

        desktop_row.add_suffix(&desktop_status_lbl);
        desktop_row.add_suffix(&desktop_btn);
        desktop_group.add(&desktop_row);
        root.append(&desktop_group);

        // --- 3. Hızlı Başlangıç Ayarları Grubu ---
        let player_group = adw::PreferencesGroup::new();
        player_group.set_title("3. Hızlı Başlangıç Tercihleri");

        let fs_row = adw::SwitchRow::new();
        fs_row.set_title("MPV Otomatik Tam Ekran");
        fs_row.set_subtitle("Video başladığında MPV'yi otomatik tam ekran modunda açar");
        fs_row.set_active(self.settings.borrow().auto_fullscreen);

        let aniskip_row = adw::SwitchRow::new();
        aniskip_row.set_title("AniSkip Otomatik İntro Atlama Entegrasyonu");
        aniskip_row.set_subtitle("AniSkip API üzerinden 's' kısayol tuşu ile intro bitişine otomatik atlar");
        aniskip_row.set_active(self.settings.borrow().aniskip_enabled);

        player_group.add(&fs_row);
        player_group.add(&aniskip_row);
        root.append(&player_group);

        // --- 4. Kurulumu Tamamla Butonu ---
        let start_btn = gtk::Button::with_label("Kurulumu Tamamla ve Başlat 🚀");
        start_btn.add_css_class("suggested-action");
        start_btn.add_css_class("pill");
        start_btn.add_css_class("title-3");
        start_btn.set_halign(gtk::Align::Center);
        start_btn.set_margin_top(12);

        let this = self.clone_ref();
        let fs_r = fs_row.clone();
        let ani_r = aniskip_row.clone();

        start_btn.connect_clicked(move |_| {
            let mut s = this.settings.borrow().clone();
            s.auto_fullscreen = fs_r.is_active();
            s.aniskip_enabled = ani_r.is_active();
            this.client.save_settings(&s);
            *this.settings.borrow_mut() = s;

            this.client.set_welcome_seen(true);
            this.page_history.borrow_mut().clear();
            this.page_history.borrow_mut().push(Page::Home);
            this.show_page(&Page::Home);
            this.fetch_home();
        });
        root.append(&start_btn);

        scroll.set_child(Some(&root));
        scroll
    }

    fn create_title_card(&self, t: &Title) -> gtk::Box {
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 6);
        box_.add_css_class("title-btn");
        box_.set_size_request(140, -1);
        box_.set_hexpand(false);
        box_.set_vexpand(false);
        box_.set_halign(gtk::Align::Start);
        box_.set_valign(gtk::Align::Start);

        let pic = self.covers.cover_picture(t.poster.as_deref(), 140, 210);
        pic.set_size_request(140, 210);
        pic.set_can_shrink(false);
        pic.set_hexpand(false);
        pic.set_vexpand(false);
        pic.set_halign(gtk::Align::Start);

        let lbl = gtk::Label::new(Some(&t.name));
        lbl.add_css_class("card-title");
        lbl.set_wrap(true);
        lbl.set_justify(gtk::Justification::Center);
        lbl.set_xalign(0.5);
        lbl.set_max_width_chars(16);
        lbl.set_lines(2);
        lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);

        box_.append(&pic);
        box_.append(&lbl);

        let gesture = gtk::GestureClick::new();
        let this = self.clone_ref();
        let title_clone = t.clone();
        gesture.connect_pressed(move |_, _, _, _| {
            this.open_episodes(title_clone.clone());
        });
        box_.add_controller(gesture);

        box_
    }

    fn build_home_view(&self) -> gtk::ScrolledWindow {
        let scroll = gtk::ScrolledWindow::new();
        let cats = self.cats.borrow();

        // Kategoriler henüz yüklenmediyse yükleme spinner'ı göster
        if cats.is_empty() {
            let spinner_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
            spinner_box.set_valign(gtk::Align::Center);
            spinner_box.set_halign(gtk::Align::Center);
            spinner_box.set_vexpand(true);
            let spinner = gtk::Spinner::new();
            spinner.set_size_request(48, 48);
            spinner.start();
            let lbl = gtk::Label::new(Some("İçerikler yükleniyor…"));
            lbl.add_css_class("dim-label");
            spinner_box.append(&spinner);
            spinner_box.append(&lbl);
            scroll.set_child(Some(&spinner_box));
            return scroll;
        }

        let main_box = gtk::Box::new(gtk::Orientation::Vertical, 18);
        main_box.set_margin_top(12);
        main_box.set_margin_bottom(18);
        main_box.set_margin_start(12);
        main_box.set_margin_end(12);

        for cat in cats.iter() {
            let shelf_title = gtk::Label::new(Some(&cat.name));
            shelf_title.add_css_class("shelf-title");
            shelf_title.set_xalign(0.0);
            shelf_title.set_margin_start(4);
            shelf_title.set_margin_bottom(4);
            main_box.append(&shelf_title);

            let flow = gtk::FlowBox::new();
            flow.set_halign(gtk::Align::Center);
            flow.set_valign(gtk::Align::Start);
            flow.set_selection_mode(gtk::SelectionMode::None);
            flow.set_activate_on_single_click(false);
            flow.set_column_spacing(16);
            flow.set_row_spacing(20);

            for t in &cat.items {
                let btn = self.create_title_card(t);
                flow.append(&btn);
            }
            main_box.append(&flow);
        }

        scroll.set_child(Some(&main_box));
        scroll
    }

    fn build_marathon_view(&self) -> gtk::ScrolledWindow {
        let scroll = gtk::ScrolledWindow::new();
        let this_click = self.clone_ref();
        let this_toggle = self.clone_ref();
        let this_remove = self.clone_ref();
        let this_clear = self.clone_ref();
        let this_cover = self.clone_ref();
        let this_reorder = self.clone_ref();

        let view = views::MarathonView::build(
            self.client.clone(),
            move |title| {
                this_click.open_episodes(title);
            },
            move |id| {
                let is_done = this_toggle.client.toggle_marathon_completed(id);
                let toast_msg = if is_done {
                    "🏁 Maraton hedefi tamamlandı!"
                } else {
                    "⏳ Maraton hedefi devam ediyor"
                };
                let toast = adw::Toast::new(toast_msg);
                toast.set_timeout(2);
                this_toggle.toast.add_toast(toast);
                this_toggle.show_page(&Page::Marathon);
            },
            move |id| {
                this_remove.client.remove_from_marathon(id);
                let toast = adw::Toast::new("Maratondan kaldırıldı");
                toast.set_timeout(2);
                this_remove.toast.add_toast(toast);
                this_remove.show_page(&Page::Marathon);
            },
            move || {
                this_clear.client.clear_marathon();
                let toast = adw::Toast::new("İzleme maratonu temizlendi");
                toast.set_timeout(2);
                this_clear.toast.add_toast(toast);
                this_clear.show_page(&Page::Marathon);
            },
            move |id, new_index| {
                this_reorder.client.reorder_marathon(id, new_index);
                this_reorder.show_page(&Page::Marathon);
            },
            move |poster, pic, w, h| {
                this_cover.covers.load_cover(poster, &pic, w, h);
            },
        );
        scroll.set_child(Some(&view));

        // Sürükleme sırasında listenin kenarlarına yaklaşınca otomatik kaydır (auto-scroll)
        let motion = gtk::DropControllerMotion::new();
        let drag_pos: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));
        let motion_state = drag_pos.clone();
        let scroll_m = scroll.clone();
        motion.connect_motion(move |_, _x, y| {
            let h = scroll_m.height() as f64;
            *motion_state.borrow_mut() = Some((y, h));
        });
        let leave_state = drag_pos.clone();
        motion.connect_leave(move |_| {
            *leave_state.borrow_mut() = None;
        });
        scroll.add_controller(motion);

        // İmleç kenarda beklese bile kaydırmaya devam etsin diye zamanlayıcı
        let scroll_t = scroll.clone();
        let timer_state = drag_pos.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            if let Some((y, h)) = *timer_state.borrow() {
                let margin = 50.0;
                let adj = scroll_t.vadjustment();
                let max = (adj.upper() - adj.page_size()).max(0.0);
                let cur = adj.value();
                let new = if y < margin {
                    (cur - ((margin - y) * 0.6 + 6.0)).clamp(0.0, max)
                } else if y > h - margin {
                    (cur + ((y - (h - margin)) * 0.6 + 6.0)).clamp(0.0, max)
                } else {
                    cur
                };
                adj.set_value(new);
            }
            gtk::glib::ControlFlow::Continue
        });

        scroll
    }

    fn build_favs_view(&self) -> gtk::ScrolledWindow {
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        let saved = self.client.load_state().saved;
        if saved.is_empty() {
            let sp = components::create_status_page(
                "Henüz Favori Eklenmedi",
                "Beğendiğiniz anime veya dizileri yıldız ikonuna tıklayarak favorilerinize ekleyin.",
                "starred-symbolic",
            );
            scroll.set_child(Some(&sp));
            return scroll;
        }

        let list_box = gtk::Box::new(gtk::Orientation::Vertical, 5);
        list_box.set_margin_top(6);
        list_box.set_margin_bottom(6);
        list_box.set_margin_start(10);
        list_box.set_margin_end(10);
        list_box.set_vexpand(false);

        for t in saved {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.add_css_class("fav-item-card");

            // Poster (maraton kartıyla birebir aynı)
            let pic = gtk::Picture::new();
            pic.set_width_request(48);
            pic.set_height_request(72);
            pic.set_can_shrink(true);
            pic.set_content_fit(gtk::ContentFit::Cover);
            pic.set_css_classes(&["cover", "cover-thumb"]);
            pic.set_valign(gtk::Align::Center);
            self.covers.load_cover(t.poster.as_deref(), &pic, 48, 72);

            // Metin bloğu (maraton kartıyla aynı yapı)
            let info_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
            info_box.set_valign(gtk::Align::Center);
            info_box.set_hexpand(true);

            let name = gtk::Label::new(Some(&t.name));
            name.add_css_class("title-3");
            name.set_xalign(0.0);
            name.set_wrap(false);
            name.set_single_line_mode(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);

            info_box.append(&name);
            episodes_view::append_title_submeta(&info_box, &t);

            // Butonlar (maraton kartıyla aynı: İzle + Çıkar)
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            actions.set_valign(gtk::Align::Center);

            let play_btn = gtk::Button::with_label("▶ İzle");
            play_btn.add_css_class("suggested-action");
            play_btn.add_css_class("pill");
            let this_play = self.clone_ref();
            let t_play = t.clone();
            play_btn.connect_clicked(move |_| this_play.open_episodes(t_play.clone()));

            let del_btn = gtk::Button::from_icon_name("user-trash-symbolic");
            del_btn.add_css_class("flat");
            del_btn.add_css_class("circular");
            del_btn.add_css_class("destructive-action");
            del_btn.set_tooltip_text(Some("Favorilerden Çıkar"));
            let this_del = self.clone_ref();
            let t_del = t.clone();
            del_btn.connect_clicked(move |_| {
                this_del.client.toggle_saved(&t_del);
                this_del.show_page(&Page::Favs);
            });

            actions.append(&play_btn);
            actions.append(&del_btn);

            row.append(&pic);
            row.append(&info_box);
            row.append(&actions);

            list_box.append(&row);
        }

        scroll.set_child(Some(&list_box));
        scroll
    }

    fn build_history_view(&self) -> gtk::ScrolledWindow {
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        let history = self.client.load_state().history;
        if history.is_empty() {
            let sp = components::create_status_page(
                "İzleme Geçmişi Boş",
                "İzlediğiniz bölümler burada görünecek.",
                "avatar-default-symbolic",
            );
            scroll.set_child(Some(&sp));
            return scroll;
        }

        let this_del = self.clone_ref();
        let this_clr = self.clone_ref();
        let this_open = self.clone_ref();
        let this_cov = self.clone_ref();
        let view = views::HistoryView::build(
            &self.client,
            &history,
            move |ids| {
                this_del.client.remove_history_items(&ids);
                this_del.show_page(&Page::History);
            },
            move || {
                this_clr.client.clear_history();
                this_clr.show_page(&Page::History);
            },
            move |h| {
                this_open.open_episodes(h.title.clone());
            },
            move |url, pic, w, h| {
                this_cov.covers.load_cover(url, pic, w, h);
            },
        );

        scroll.set_child(Some(&view));
        scroll
    }

    fn build_settings_view(&self) -> gtk::ScrolledWindow {
        let scroll = gtk::ScrolledWindow::new();
        let settings = self.settings.borrow();
        let this_save = self.clone_ref();
        let this_wipe = self.clone_ref();

        let view = views::SettingsView::build(
            &settings,
            move |new_s| {
                *this_save.settings.borrow_mut() = new_s.clone();
                this_save.client.save_settings(&new_s);
                let toast = adw::Toast::new("Ayarlar kaydedildi");
                toast.set_timeout(3);
                this_save.toast.add_toast(toast);
            },
            move |remove_app| {
                this_wipe.client.wipe_all_data();
                if remove_app {
                    crate::uninstall_application();
                    std::process::exit(0);
                } else {
                    let toast = adw::Toast::new("Tüm veriler temizlendi ve sıfırlandı!");
                    toast.set_timeout(3);
                    this_wipe.toast.add_toast(toast);
                    this_wipe.page_history.borrow_mut().clear();
                    this_wipe.page_history.borrow_mut().push(Page::Welcome);
                    this_wipe.show_page(&Page::Welcome);
                }
            },
        );

        scroll.set_child(Some(&view));
        scroll
    }

    fn build_search_view(&self) -> gtk::ScrolledWindow {
        let scroll = gtk::ScrolledWindow::new();
        let results = self.search_results.borrow();

        if results.is_empty() {
            let sp = components::create_status_page(
                "Sonuç Bulunamadı",
                "Arama sorgunuza uygun anime veya dizi bulunamadı.",
                "system-search-symbolic",
            );
            scroll.set_child(Some(&sp));
            return scroll;
        }

        let flow = gtk::FlowBox::new();
        flow.set_margin_top(12);
        flow.set_margin_bottom(18);
        flow.set_margin_start(12);
        flow.set_margin_end(12);
        flow.set_halign(gtk::Align::Center);
        flow.set_valign(gtk::Align::Start);
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_activate_on_single_click(false);
        flow.set_column_spacing(16);
        flow.set_row_spacing(20);

        for t in results.iter() {
            let btn = self.create_title_card(t);
            flow.append(&btn);
        }

        scroll.set_child(Some(&flow));
        scroll
    }

    fn build_episodes_view(&self, title: &Title, eps: &[Episode]) -> gtk::ScrolledWindow {
        let scroll = gtk::ScrolledWindow::new();

        // Eğer içerik Film ise özel Film Detay Görünümü sunulur
        let is_movie = title.title_type.as_deref() == Some("movie")
            || (eps.len() <= 1 && eps.first().map(|e| e.name.contains("Filmi")).unwrap_or(false));

        if is_movie {
            let header_poster = self.covers.cover_picture(title.poster.as_deref(), 160, 240);
            let bookmark_btn = components::bookmark_button(&self.client, title);
            let this_bm = self.clone_ref();
            let t_clone = title.clone();
            bookmark_btn.connect_clicked(move |b| {
                let saved = this_bm.client.toggle_saved(&t_clone);
                b.set_icon_name(if saved { "starred-symbolic" } else { "non-starred-symbolic" });
                b.set_tooltip_text(Some(if saved { "Favorilerden Çıkar" } else { "Favorilere Ekle" }));
            });

            let marathon_btn = components::marathon_button(&self.client, title);
            let this_mar = self.clone_ref();
            let t_clone_mar = title.clone();
            marathon_btn.connect_clicked(move |b| {
                let added = this_mar.client.toggle_marathon(&t_clone_mar);
                b.set_icon_name(if added { "media-playlist-repeat-symbolic" } else { "flag-symbolic" });
                b.set_tooltip_text(Some(if added { "Maratondan Çıkar" } else { "İzleme Maratonuna Ekle" }));
                let msg = if added { "🏆 İzleme Maratonuna eklendi!" } else { "İzleme Maratonundan çıkarıldı" };
                let toast = adw::Toast::new(msg);
                toast.set_timeout(2);
                this_mar.toast.add_toast(toast);
            });

            let this_play = self.clone_ref();
            let title_c = title.clone();
            let ep_c = eps.first().cloned().unwrap_or(Episode {
                episode: 1,
                season: 1,
                name: title.name.clone(),
            });
            let movie_progress = self.client.get_progress(title.id, 1, 1);
            let (movie_view, movie_pb, movie_lbl) = episodes_view::create_movie_detail_view(
                title,
                &header_poster,
                &bookmark_btn,
                &marathon_btn,
                movie_progress,
                move || {
                    this_play.play(&title_c, &ep_c);
                },
            );
            // Film progress bar'ını live güncelleme için kaydet
            let prog_key = format!("{}:1:1", title.id);
            self.progress_bars.borrow_mut().insert(prog_key, (movie_pb, movie_lbl));
            scroll.set_child(Some(&movie_view));
            return scroll;
        }

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

        // 1) Dizi Detay Başlık Kartı (Kapak resmi, Yıl, Açıklama vb.)
        let header_poster = self.covers.cover_picture(title.poster.as_deref(), 120, 180);
        let bookmark_btn = components::bookmark_button(&self.client, title);
        let this_bm = self.clone_ref();
        let t_clone = title.clone();
        bookmark_btn.connect_clicked(move |b| {
            let saved = this_bm.client.toggle_saved(&t_clone);
            b.set_icon_name(if saved { "starred-symbolic" } else { "non-starred-symbolic" });
            b.set_tooltip_text(Some(if saved { "Favorilerden Çıkar" } else { "Favorilere Ekle" }));
        });

        let marathon_btn = components::marathon_button(&self.client, title);
        let this_mar = self.clone_ref();
        let t_clone_mar = title.clone();
        marathon_btn.connect_clicked(move |b| {
            let added = this_mar.client.toggle_marathon(&t_clone_mar);
            b.set_icon_name(if added { "media-playlist-repeat-symbolic" } else { "flag-symbolic" });
            b.set_tooltip_text(Some(if added { "Maratondan Çıkar" } else { "İzleme Maratonuna Ekle" }));
            let msg = if added { "🏆 İzleme Maratonuna eklendi!" } else { "İzleme Maratonundan çıkarıldı" };
            let toast = adw::Toast::new(msg);
            toast.set_timeout(2);
            this_mar.toast.add_toast(toast);
        });

        let detail_header = episodes_view::create_title_detail_header(title, &header_poster, &bookmark_btn, &marathon_btn);
        root.append(&detail_header);

        let settings = self.settings.borrow();

        // 2) Tek Seferlik İpucu Banner'ı (Hızlı Arama)
        if settings.quick_search_enabled && !self.client.is_quick_search_tip_seen() {
            let this_tip = self.clone_ref();
            let tip_banner = episodes_view::create_quick_search_tip_banner(
                &settings.quick_search_shortcut,
                move || {
                    this_tip.client.set_quick_search_tip_seen(true);
                },
            );
            root.append(&tip_banner);
        }

        // 3) Tek Seferlik İpucu Banner'ı (Sağ Tık → İzlendi/İzlenmedi)
        if !self.client.is_right_click_tip_seen() {
            let this_tip2 = self.clone_ref();
            let right_click_tip = episodes_view::create_right_click_tip_banner(move || {
                this_tip2.client.set_right_click_tip_seen(true);
            });
            root.append(&right_click_tip);
        }

        // 3) Hızlı Bölüm Arama Giriş Çubuğu (Ayarlara bağlı)
        let ep_search_entry = gtk::SearchEntry::new();
        if settings.quick_search_enabled {
            let search_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            search_box.set_margin_start(12);
            search_box.set_margin_end(12);
            search_box.set_margin_bottom(8);

            ep_search_entry.set_placeholder_text(Some(&format!(
                "Bölüm numarası veya adı ara… ({})",
                settings.quick_search_shortcut
            )));
            ep_search_entry.set_hexpand(true);
            search_box.append(&ep_search_entry);
            root.append(&search_box);

            // Klavye Kısayolu Dinleyicisi (Uygulama odaklı)
            let shortcut_key = settings.quick_search_shortcut.clone();
            let ep_entry_clone = ep_search_entry.clone();
            let key_controller = gtk::EventControllerKey::new();
            key_controller.connect_key_pressed(move |_, keyval, _, state| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();
                let is_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);

                let triggered = match shortcut_key.as_str() {
                    "Ctrl+F" => is_ctrl && (key_name == "f" || key_name == "F"),
                    "Ctrl+K" => is_ctrl && (key_name == "k" || key_name == "K"),
                    "F3" => key_name == "F3",
                    _ => key_name == "slash" || key_name == "kp_divide",
                };

                if triggered {
                    ep_entry_clone.grab_focus();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            self.window.add_controller(key_controller);
        }
        drop(settings);

        // 4) Bölüm Listesi
        let list_box = gtk::ListBox::new();
        list_box.add_css_class("content-list");
        list_box.set_margin_start(12);
        list_box.set_margin_end(12);
        list_box.set_margin_bottom(16);

        if eps.is_empty() {
            let sp = components::create_status_page(
                "Bölüm Bulunamadı",
                "Bu yapım için henüz bölüm listesi bulunmuyor.",
                "media-tape-symbolic",
            );
            root.append(&sp);
        } else {
            let rows: Vec<(Episode, gtk::Box)> = eps.iter().map(|e| {
                let key = format!("{}:{}:{}", title.id, e.season, e.episode);

                let name = gtk::Label::new(Some(&format!(
                    "S{:02} E{:02}   {}",
                    e.season, e.episode, e.name
                )));
                name.set_xalign(0.0);
                name.add_css_class("title-4");
                name.set_hexpand(true);

                let time_lbl = gtk::Label::new(None);
                time_lbl.set_xalign(1.0);
                time_lbl.add_css_class("dim-label");

                let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                header_box.append(&name);
                header_box.append(&time_lbl);

                let pic = self.covers.cover_picture(title.poster.as_deref(), 48, 72);
                pic.set_valign(gtk::Align::Center);

                let right_col = gtk::Box::new(gtk::Orientation::Vertical, 4);
                right_col.set_valign(gtk::Align::Center);
                right_col.append(&header_box);

                let (saved_pos, saved_dur) = self.progress.borrow()
                    .get(&key).copied()
                    .unwrap_or((0.0, 0.0));

                let progress_bar = gtk::ProgressBar::new();
                progress_bar.add_css_class("episode-progress");

                let fmt_time = |s: f64| -> String {
                    let s = s as u64;
                    if s >= 3600 { format!("{}:{:02}:{:02}", s/3600, (s%3600)/60, s%60) }
                    else { format!("{}:{:02}", s/60, s%60) }
                };

                if saved_dur > 0.0 && saved_pos > 1.0 {
                    progress_bar.set_fraction((saved_pos / saved_dur).clamp(0.0, 1.0));
                    progress_bar.set_visible(true);
                    time_lbl.set_text(&format!("{} / {}", fmt_time(saved_pos), fmt_time(saved_dur)));
                    time_lbl.set_visible(true);
                } else {
                    progress_bar.set_visible(false);
                    time_lbl.set_visible(false);
                }
                right_col.append(&progress_bar);

                self.progress_bars.borrow_mut().insert(key.clone(), (progress_bar, time_lbl));

                let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                row.set_margin_top(6);
                row.set_margin_bottom(6);
                row.set_margin_start(14);
                row.set_margin_end(14);
                row.set_valign(gtk::Align::Center);
                row.append(&pic);
                row.append(&right_col);

                let is_watched = Rc::new(RefCell::new(
                    self.client.is_watched(title.id, e.season, e.episode)
                    || (saved_dur > 0.0 && saved_pos / saved_dur > 0.9)
                ));

                // İzlendi simgesi (dinamik - sağ tık ile toggle)
                let done_icon = gtk::Image::from_icon_name("object-select-symbolic");
                done_icon.add_css_class("dim-label");
                done_icon.set_tooltip_text(Some("İzlendi"));
                done_icon.set_valign(gtk::Align::Center);
                done_icon.set_visible(*is_watched.borrow());
                row.append(&done_icon);

                // Sol tık → izle
                let this_play = self.clone_ref();
                let title_play = title.clone();
                let ep_play = e.clone();
                let click = gtk::GestureClick::new();
                click.set_button(1); // sadece sol tık
                click.connect_pressed(move |_, _, _, _| {
                    this_play.play(&title_play, &ep_play);
                });
                row.add_controller(click);

                // Sağ tık → context menü (İzlendi Toggle)
                let this_ctx = self.clone_ref();
                let title_ctx = title.clone();
                let ep_ctx = e.clone();
                let is_watched_ctx = is_watched.clone();
                let done_icon_ctx = done_icon.clone();
                let row_ctx = row.clone();

                let right_click = gtk::GestureClick::new();
                right_click.set_button(3);
                right_click.connect_pressed(move |gesture, _, x, y| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);

                    let currently_watched = *is_watched_ctx.borrow();
                    let label = if currently_watched {
                        "✖ İzlenmedi Olarak İşaretle"
                    } else {
                        "✅ İzlendi Olarak İşaretle"
                    };

                    let menu_model = gio::Menu::new();
                    menu_model.append(Some(label), Some("row.toggle-watched"));

                    // Action group → row'a bağla
                    let client_c = this_ctx.client.clone();
                    let title_c = title_ctx.clone();
                    let ep_c = ep_ctx.clone();
                    let is_watched_c = is_watched_ctx.clone();
                    let done_icon_c = done_icon_ctx.clone();
                    let this_refresh = this_ctx.clone_ref();

                    let action_group = gio::SimpleActionGroup::new();
                    let action = gio::SimpleAction::new("toggle-watched", None);
                    action.connect_activate(move |_, _| {
                        let was_watched = *is_watched_c.borrow();
                        if was_watched {
                            client_c.remove_watched(title_c.id, ep_c.season, ep_c.episode);
                            *is_watched_c.borrow_mut() = false;
                            done_icon_c.set_visible(false);
                        } else {
                            let w = api::Watched {
                                title_id: title_c.id,
                                episode: ep_c.episode,
                                season: ep_c.season,
                            };
                            client_c.save_watched(&w, &title_c.name);
                            client_c.add_history(&title_c, &ep_c);
                            *is_watched_c.borrow_mut() = true;
                            done_icon_c.set_visible(true);
                        }
                        let msg = if was_watched {
                            "✖ İzlenmedi olarak işaretlendi"
                        } else {
                            "✅ İzlendi olarak işaretlendi"
                        };
                        let toast = adw::Toast::new(msg);
                        toast.set_timeout(2);
                        this_refresh.toast.add_toast(toast);
                    });
                    action_group.add_action(&action);
                    // action group'u row'a ekle (popover parent ile aynı widget)
                    row_ctx.insert_action_group("row", Some(&action_group));

                    // Popover'ı row'a bağla, koordinatlar row'a göre
                    let popover = gtk::PopoverMenu::from_model(Some(&menu_model));
                    popover.set_parent(&row_ctx);
                    let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                    popover.set_pointing_to(Some(&rect));
                    popover.set_has_arrow(true);
                    popover.popup();
                });
                row.add_controller(right_click);

                (e.clone(), row)
            }).collect();

            for (_, row_widget) in &rows {
                list_box.append(row_widget);
            }

            // Arama çubuğu filtreleme mantığı
            let rows_rc = Rc::new(rows);
            ep_search_entry.connect_search_changed(move |e| {
                let query = e.text().trim().to_lowercase();
                for (ep_data, row_widget) in rows_rc.iter() {
                    if query.is_empty() {
                        row_widget.set_visible(true);
                    } else {
                        let name_match = ep_data.name.to_lowercase().contains(&query);
                        let ep_num_match = ep_data.episode.to_string() == query
                            || format!("e{}", ep_data.episode) == query
                            || format!("s{:02}e{:02}", ep_data.season, ep_data.episode) == query;
                        row_widget.set_visible(name_match || ep_num_match);
                    }
                }
            });

            root.append(&list_box);
        }

        scroll.set_child(Some(&root));
        scroll
    }

    fn spawn<F, R>(&self, f: F)
    where
        F: FnOnce(Arc<Client>) -> R + Send + 'static,
        R: FnOnce() -> Msg + Send + 'static,
    {
        let c = self.client.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Msg>();
        std::thread::spawn(move || {
            let res_fn = f(c);
            let _ = tx.send(res_fn());
        });
        let this = self.clone_ref();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(msg) => {
                this.busy(false);
                this.handle_msg(msg);
                glib::ControlFlow::Break
            }
            Err(_) => glib::ControlFlow::Continue,
        });
    }

    fn handle_msg(&self, msg: Msg) {
        match msg {
            Msg::Cats(res) => match res {
                Ok(cats) => {
                    *self.cats.borrow_mut() = cats;
                    if self.page_history.borrow().last() == Some(&Page::Home) {
                        self.show_page(&Page::Home);
                    }
                }
                Err(e) => self.show_error(&e),
            },
            Msg::Search(res) => match res {
                Ok(results) => {
                    *self.search_results.borrow_mut() = results;
                    let mut st = self.page_history.borrow_mut();
                    if st.last() != Some(&Page::Search) {
                        st.push(Page::Search);
                    }
                    drop(st);
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
                    self.page_history.borrow_mut().push(page.clone());
                    self.show_page(&page);
                }
                Err(e) => self.show_error(&e),
            },
            Msg::Play(title, ep, res) => match res {
                Ok(candidates) => self.play_candidates(&title, &ep, &candidates),
                Err(e) => self.show_error(&e),
            },
        }
    }

    fn open_episodes(&self, title: Title) {
        self.busy(true);
        self.spawn(move |c| {
            // Favoriler/geçmiş/maratondan gelen başlıklar eski şemalı olabilir;
            // detay başlığında tür/süre/yayın görünsün diye taze veriyle zenginleştir.
            let enriched = c.enrich_title(&title);
            let res = c.episodes(&enriched);
            move || Msg::Eps(enriched.clone(), res)
        });
    }

    fn play(&self, title: &Title, ep: &Episode) {
        let title = title.clone();
        let ep = ep.clone();
        eprintln!("[PLAY] çağrıldı: {} S{:02}E{:02}", title.name, ep.season, ep.episode);
        let is_movie = title.title_type.as_deref() == Some("movie");
        self.busy(true);
        self.spawn(move |c| {
            let res = if is_movie {
                c.resolve_movie(title.id).map(|u| vec![u])
            } else {
                c.resolve_all(title.id, ep.episode, ep.season)
            };
            move || Msg::Play(title, ep, res)
        });
    }

    /// Bölümü oynatır; en iyi kaliteli kaynaktan başlar, açılamazsa sıradaki kaynağa geçer.
    fn play_candidates(&self, title: &Title, ep: &Episode, candidates: &[String]) {
        let w = api::Watched {
            title_id: title.id,
            episode: ep.episode,
            season: ep.season,
        };
        self.client.set_current(&w);
        self.client.add_history(&title, &ep);
        eprintln!(
            "[PLAY-CAND] yeni mpv başlatılıyor: {} S{:02}E{:02} (kaynak sayısı={})",
            title.name, ep.season, ep.episode, candidates.len()
        );

        let media_title = format!("{} | S{:02}E{:02}", title.name, ep.season, ep.episode);
        let tid = title.id;
        let season = ep.season;
        let episode = ep.episode;
        let prog_key = format!("{tid}:{season}:{episode}");

        let saved_pos = self.client.get_progress(tid, season, episode)
            .filter(|(pos, dur)| *pos > 5.0 && *dur > 0.0 && *pos / *dur < 0.95)
            .map(|(pos, _)| pos);

        let sock_path = format!("/tmp/animecix-mpv-{tid}-{season}-{episode}.sock");
        let _ = std::fs::remove_file(&sock_path);

        let auto_fullscreen = self.settings.borrow().auto_fullscreen;
        let upscale = self.settings.borrow().upscale.clone();
        let aniskip = if self.settings.borrow().aniskip_enabled {
            self.client.fetch_aniskip_timestamps(&title.name, episode)
        } else {
            api::AniSkipTimes::default()
        };

        let fmt_sec = |sec: f64| -> String {
            let s = sec as u64;
            format!("{:02}:{:02}", s / 60, s % 60)
        };

        let skip_cmd = if let (Some(st), Some(et)) = (aniskip.op_start, aniskip.op_end) {
            format!("s seek {et:.1} absolute; show-text \"⏩ İntro Atlandı (AniSkip: {} → {})\" 3000\n", fmt_sec(st), fmt_sec(et))
        } else {
            "s show-text \"⚠️ İntro zamanı bulunamadı (AniSkip)\" 2500\n".to_string()
        };

        let outro_cmd = if let (Some(st), Some(et)) = (aniskip.ed_start, aniskip.ed_end) {
            format!("e seek {et:.1} absolute; show-text \"⏩ Outro Atlandı (AniSkip: {} → {})\" 3000\n", fmt_sec(st), fmt_sec(et))
        } else {
            "e show-text \"⚠️ Outro zamanı bulunamadı (AniSkip)\" 2500\n".to_string()
        };

        let cmd_file = format!("/tmp/animecix-cmd-{tid}.cmd");
        let _ = std::fs::remove_file(&cmd_file);

        let input_conf_path = format!("/tmp/animecix-input-{tid}.conf");
        let input_conf_content = format!(
            "{skip_cmd}\
             {outro_cmd}\
             S seek -30; show-text \"⏪ 30s Geri\" 2000\n\
             n run \"sh\" \"-c\" \"echo next > {cmd_file}\"; show-text \"⏳ Sonraki Bölüm Yükleniyor...\" 4000\n\
             N run \"sh\" \"-c\" \"echo next > {cmd_file}\"; show-text \"⏳ Sonraki Bölüm Yükleniyor...\" 4000\n\
             p run \"sh\" \"-c\" \"echo prev > {cmd_file}\"; show-text \"⏳ Önceki Bölüm Yükleniyor...\" 4000\n\
             P run \"sh\" \"-c\" \"echo prev > {cmd_file}\"; show-text \"⏳ Önceki Bölüm Yükleniyor...\" 4000\n"
        );
        let _ = std::fs::write(&input_conf_path, input_conf_content);

        let progress = self.progress.clone();
        let progress_bars = self.progress_bars.clone();
        let client = self.client.clone();
        let toast = self.toast.clone();

        let t = adw::Toast::new(&format!(
            "▶ {media_title} açılıyor…{}",
            saved_pos.map(|p| {
                let s = p as u64;
                if s >= 3600 { format!(" ({}:{:02}:{:02}'den)", s/3600, (s%3600)/60, s%60) }
                else { format!(" ({}:{:02}'den)", s/60, s%60) }
            }).unwrap_or_default()
        ));
        t.set_timeout(3);
        self.toast.add_toast(t);

        let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        // Şu an izlenen bölüm (sonraki bölüme geçişte güncellenir) — ilerleme ve "izlendi"
        // işaretlemesi bunun üzerinden yapılır, böylece sonraki bölüm izlenmeden tamamlanmaz.
        let current_shared = std::sync::Arc::new(std::sync::Mutex::new((episode, season)));

        // Sonraki/önceki bölüm geçişi: worker mevcut mpv'yi kapatır, uygulama yeni
        // bölümü kendi API'siyle çözüp yepyeni bir mpv ile açar (kırılgan loadfile
        // yaklaşımı yerine sağlam yöntem — ilk bölümün zaten çalışan akışı yeniden kullanılır).
        let mpv_child = std::sync::Arc::new(std::sync::Mutex::new(None::<std::process::Child>));
        let (next_tx, next_rx) = std::sync::mpsc::channel::<(Title, Episode)>();
        {
            let next_rx = std::sync::Arc::new(std::sync::Mutex::new(next_rx));
            let this = self.clone_ref();
            let alive_next = alive.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                let msg = {
                    let rx = next_rx.lock().unwrap();
                    match rx.try_recv() {
                        Ok(m) => Some(m),
                        Err(_) => None,
                    }
                };
                if let Some((t, e)) = msg {
                    eprintln!("[NEXT] GTK callback play çağırıyor (title={}, ep={}/{})", t.name, e.season, e.episode);
                    this.play(&t, &e);
                }
                if alive_next.load(std::sync::atomic::Ordering::Relaxed) {
                    glib::ControlFlow::Continue
                } else {
                    glib::ControlFlow::Break
                }
            });
        }

        // Worker/supervisor iş parçacıklarından ana iş parçacığına toast bildirimi (GObject'lar Send değil)
        let (toast_tx, toast_rx) = std::sync::mpsc::channel::<String>();
        {
            let toast_rx = std::sync::Arc::new(std::sync::Mutex::new(toast_rx));
            let alive_toast = alive.clone();
            let toast_h = toast.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
                let rx = toast_rx.lock().unwrap();
                let mut msg: Option<String> = None;
                loop {
                    match rx.try_recv() {
                        Ok(m) => msg = Some(m),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    }
                }
                drop(rx);
                if let Some(m) = msg {
                    let tt = adw::Toast::new(&m);
                    tt.set_timeout(3);
                    toast_h.add_toast(tt);
                }
                if !alive_toast.load(std::sync::atomic::Ordering::Relaxed) {
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        }

        // İzleme (progress / n-p / aniskip) işçi iş parçacığı — tüm denemeler boyunca yaşar
        {
            let alive = alive.clone();
            let sock_poll = sock_path.clone();
            let sock_c = sock_path.clone();
            let cmd_c = cmd_file.clone();
            let aniskip_c = aniskip.clone();
            let client_c = client.clone();
            let title_c = title.clone();
            let ep_c = ep.clone();
            let current_shared_c = current_shared.clone();
            let toast_tx_w = toast_tx.clone();
            let mpv_child_c = mpv_child.clone();
            let next_tx_c = next_tx.clone();
            let (sender, receiver) = std::sync::mpsc::channel::<(f64, f64)>();
            std::thread::spawn(move || {
                let mut current_ep = ep_c.episode;
                let current_season = ep_c.season;
                let mut op_prompted = false;
                let mut ed_prompted = false;

                // Soket belirene kadar bekle
                while alive.load(std::sync::atomic::Ordering::Relaxed)
                    && !std::path::Path::new(&sock_poll).exists()
                {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
                if !alive.load(std::sync::atomic::Ordering::Relaxed) { return; }

                loop {
                    if std::path::Path::new(&sock_c).exists() {
                        // MPV içi 'n' / 'p' kısayolu — loadfile YERİNE uygulama kendisi
                        // sonraki/önceki bölümü çözüp yepyeni bir mpv ile açsın.
                        if std::path::Path::new(&cmd_c).exists() {
                            if let Ok(cmd_str) = std::fs::read_to_string(&cmd_c) {
                                let _ = std::fs::remove_file(&cmd_c);
                                let cmd = cmd_str.trim();
                                let is_next = cmd == "next";
                                let is_prev = cmd == "prev";
                                eprintln!("[NEXT] cmd={:?} is_next={} is_prev={} current_ep={}", cmd, is_next, is_prev, current_ep);
                                if (is_next || is_prev) && (is_next || current_ep > 1) {
                                    let target_ep = if is_next { current_ep + 1 } else { current_ep - 1 };
                                    current_ep = target_ep;
                                    op_prompted = false;
                                    ed_prompted = false;
                                    // Yükleniyor göstergesi (mpv üstünde OSD)
                                    let _ = crate::player::send_mpv_cmd(
                                        &sock_c,
                                        "{\"command\":[\"show-text\",\"⏳ Sonraki Bölüm Yükleniyor...\",3000]}\n",
                                    );
                                    std::thread::sleep(std::time::Duration::from_millis(300));
                                    // Mevcut mpv'yi kapat
                                    if let Some(c) = mpv_child_c.lock().unwrap().as_mut() {
                                        let _ = c.kill();
                                        eprintln!("[NEXT] mpv kill gönderildi (ep->{})", target_ep);
                                    } else {
                                        eprintln!("[NEXT] UYARI: mpv_child kilit içi boş, kill edilemedi");
                                    }
                                    // GTK ana iş parçacığına sonraki bölümü açtır (uygulama URL'yi çözer)
                                    match next_tx_c.send((title_c.clone(), Episode {
                                        episode: target_ep,
                                        season: current_season,
                                        name: format!("Bölüm {target_ep}"),
                                    })) {
                                        Ok(_) => eprintln!("[NEXT] GTK'ya play mesajı yollandı (ep={})", target_ep),
                                        Err(e) => eprintln!("[NEXT] HATA: play mesajı yollanamadı: {}", e),
                                    }
                                }
                            }
                        }

                        let (pos, dur) = crate::player::query_mpv_position(&sock_c).unwrap_or((0.0, 0.0));
                        if let Some(st) = aniskip_c.op_start {
                            if !op_prompted && pos >= (st - 1.5) && pos <= (st + 10.0) {
                                op_prompted = true;
                                crate::player::send_mpv_cmd(&sock_c, "{\"command\":[\"show-text\", \"⏩ İntro Başladı ('s' ile atlayabilirsiniz)\", 7000]}\n");
                            }
                        }
                        if let Some(st) = aniskip_c.ed_start {
                            if !ed_prompted && pos >= (st - 1.5) && pos <= (st + 10.0) {
                                ed_prompted = true;
                                crate::player::send_mpv_cmd(&sock_c, "{\"command\":[\"show-text\", \"🏁 Outro Başladı ('n' ile Sonraki Bölüm)\", 7000]}\n");
                            }
                        }
                        if sender.send((pos, dur)).is_err() { break; }
                    } else {
                        if !alive.load(std::sync::atomic::Ordering::Relaxed) { break; }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                        continue;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            });

            let progress2 = progress.clone();
            let progress_bars2 = progress_bars.clone();
            let client_prog = client.clone();
            let pk = prog_key.clone();
            let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));
            let current_shared_t = current_shared.clone();
            let mut marked_ep: Option<(u64, u64)> = None;
            glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
                let rx = receiver.lock().unwrap();
                let mut latest: Option<(f64, f64)> = None;
                let mut disconnected = false;
                loop {
                    match rx.try_recv() {
                        Ok(v) => { latest = Some(v); }
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => { disconnected = true; break; }
                    }
                }
                drop(rx);
                let cur = *current_shared_t.lock().unwrap();
                let (cur_ep, cur_season) = cur;
                // İlerleme anahtarı her zaman GÜNCEL bölüme göre olmalı (başlangıç bölümüne
                // sabitlenirse 'n' ile geçilen bölümün ilerlemesi öncekine kaydedilir).
                let pk_cur = format!("{tid}:{cur_season}:{cur_ep}");

                if disconnected {
                    let last = progress2.borrow().get(&pk_cur).copied();
                    if let Some((pos, dur)) = last {
                        client_prog.save_progress(tid, cur.1, cur.0, pos, dur);
                    }
                    return glib::ControlFlow::Break;
                }

                if let Some((pos, dur)) = latest {
                    if pos < 1.0 { return glib::ControlFlow::Continue; }
                    progress2.borrow_mut().insert(pk_cur.clone(), (pos, dur));
                    if let Some((pb, lbl)) = progress_bars2.borrow().get(&pk_cur) {
                        if dur > 0.0 {
                            pb.set_fraction((pos / dur).clamp(0.0, 1.0));
                            let fmt = |s: f64| -> String {
                                let s = s as u64;
                                if s >= 3600 { format!("{}:{:02}:{:02}", s/3600, (s%3600)/60, s%60) }
                                else { format!("{}:{:02}", s/60, s%60) }
                            };
                            lbl.set_text(&format!("{} / {}", fmt(pos), fmt(dur)));
                            lbl.set_visible(true);
                            pb.set_visible(true);
                        }
                    }
                    client_prog.save_progress(tid, cur.1, cur.0, pos, dur);
                    // İzlenme eşiği aşıldıysa bölümü "izlendi" işaretle. Sonraki bölüme geçerken
                    // izlenmeden tamamlanmasın diye işaretleme yalnızca burada yapılır.
                    if api::Client::played_enough(pos, dur) && marked_ep != Some(cur) {
                        client_prog.save_watched(&api::Watched { title_id: tid, episode: cur.0, season: cur.1 }, "");
                        marked_ep = Some(cur);
                    }
                }
                glib::ControlFlow::Continue
            });
        }

        // Kaynak deneme (supervisor) iş parçacığı — en iyi kaliteli önce, başarısız olursa sıradakine geç
        {
            let alive = alive.clone();
            let sock_path_c = sock_path.clone();
            let cmd_file_c = cmd_file.clone();
            let input_conf_path_c = input_conf_path.clone();
            let media_title_c = media_title.clone();
            let toast_tx_c = toast_tx.clone();
            let saved_pos_c = saved_pos;
            let auto_fullscreen_c = auto_fullscreen;
            let upscale_c = upscale;
            let mpv_child_c = mpv_child.clone();
            let candidates: Vec<String> = candidates.to_vec();
            std::thread::spawn(move || {
                'supervisor: for (i, url) in candidates.iter().enumerate() {
                    let _ = std::fs::remove_file(&cmd_file_c);
                    let _ = std::fs::remove_file(&sock_path_c);
                    let mut cmd = std::process::Command::new("mpv");
                    cmd.arg("--user-agent=mozilla")
                        .arg(format!("--force-media-title={media_title_c}"))
                        .arg("--keep-open=yes")
                        .arg(format!("--input-ipc-server={sock_path_c}"));
                    if auto_fullscreen_c { cmd.arg("--fullscreen"); }
                    cmd.arg(format!("--input-conf={input_conf_path_c}"));
                    cmd.args(saved_pos_c.map(|p| format!("--start={p:.1}")).as_slice())
                        .arg("--cache=yes")
                        .arg("--demuxer-max-bytes=128MiB")
                        .arg("--demuxer-max-back-bytes=32MiB")
                        .arg("--demuxer-readahead-secs=120")
                        .arg("--cache-pause=yes")
                        .arg("--cache-pause-wait=3")
                        .arg("--cache-secs=120")
                        .arg("--stream-lavf-o=reconnect=1,reconnect_streamed=1,reconnect_delay_max=5")
                        .arg("--network-timeout=10")
                        .arg("--hwdec=auto-safe")
                        .arg("--ytdl-format=bestvideo[height<=1080]+bestaudio/best")
                        .args(crate::api::upscale_mpv_args(&upscale_c, match upscale_c.as_str() {
                            "anime4k_light" => resolve_upscale_shader("Anime4K_Upscale_DTD_x2.glsl"),
                            "anime4k_normal" => resolve_upscale_shader("Anime4K_Upscale_Original_x2.glsl"),
                            "anime4k_ultra" => resolve_upscale_shader("Anime4K_Upscale_CNN_x2_UL.glsl"),
                            _ => None,
                        }.as_deref()))
                        .arg(url);
                    eprintln!("[SUP] mpv spawn deneniyor (ep={}, kaynak={}, url={:.80})", episode, i, url);
                    let child = match cmd.spawn() {
                        Ok(c) => c,
                        Err(e) => { eprintln!("[SUP] HATA mpv başlatılamadı (ep={}, kaynak={}): {}", episode, i, e); continue; }
                    };
                    eprintln!("[SUP] mpv spawn edildi (ep={}, kaynak={})", episode, i);
                    *mpv_child_c.lock().unwrap() = Some(child);

                    // Bölüm başarıyla açıldıktan sonra supervisor, mpv kapatanacağını
                    // KENDİSİ poller (try_wait); böylece worker aynı mutex'i kilitleyip
                    // mpv'yi öldürebilir. ÖNEMLİ: kilidi blocking `wait()` ile TUTMUYORUZ,
                    // aksi halde worker kill için kilidi alamaz ve deadlock olurdu (mpv
                    // kapanmaz, sonraki bölüme geçilmezdi).
                    let start = std::time::Instant::now();
                    let mut playing = false;
                    loop {
                        // Süreç çıktıysa döngüden çık (kullanıcı kapattı, bölüm bitti ya da
                        // worker sonraki bölüme geçmek için öldürdü). Kilit yalnızca try_wait
                        // süresince KISA tutulur.
                        let exited = {
                            let mut g = mpv_child_c.lock().unwrap();
                            match g.as_mut().unwrap().try_wait() {
                                Ok(Some(_)) => true,
                                Ok(None) => false,
                                Err(_) => true,
                            }
                        };
                        if exited { break; }
                        // IPC soketi oluştuysa MPV dosyayı açtı demektir.
                        if std::path::Path::new(&sock_path_c).exists() {
                            if !playing {
                                eprintln!("[SUP] socket belirdi, oynatma başladı (ep={}, kaynak={})", episode, i);
                            }
                            playing = true;
                        }
                        // Yalnızca hiç açılmadıysa zaman aşımıyla başarısız say.
                        if start.elapsed() > std::time::Duration::from_secs(25) && !playing {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }

                    eprintln!("[SUP] döngü bitti (ep={}, kaynak={}, playing={})", episode, i, playing);

                    if playing {
                        // Bölüm başarıyla açıldı; mpv kapandığında başka kaynağı denemiyoruz.
                        // Worker bu mpv'yi artık kilitlemeyeceğinden kilitli wait güvenli.
                        if let Some(c) = mpv_child_c.lock().unwrap().as_mut() { let _ = c.wait(); }
                        break 'supervisor;
                    } else {
                        if let Some(c) = mpv_child_c.lock().unwrap().as_mut() {
                            let _ = c.kill();
                            let _ = c.wait();
                        }
                        if i + 1 < candidates.len() {
                            let _ = toast_tx_c.send("Kaynak açılamadı, diğer kaynağa geçiliyor…".to_string());
                            continue;
                        } else {
                            let _ = toast_tx_c.send("Bölüm hiçbir kaynakta açılamadı.".to_string());
                            break;
                        }
                    }
                }
                alive.store(false, std::sync::atomic::Ordering::SeqCst);
                let _ = std::fs::remove_file(&sock_path_c);
            });
        }
    }

    fn do_search(&self, q: String) {
        let q = q.trim().to_string();
        if q.is_empty() { return; }
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
        let t = adw::Toast::new(&format!("Hata: {msg}"));
        t.set_timeout(4);
        self.toast.add_toast(t);
    }
}
