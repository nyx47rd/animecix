use gtk::prelude::*;

fn main() {
    let app = gtk::Application::builder()
        .application_id("tr.com.scrolltest")
        .build();
    app.connect_activate(|app| {
        if std::env::var("SCROLLTEST_NOCSS").is_err() {
            let css = gtk::CssProvider::new();
            css.load_from_string(".cover-xs { max-width: 48px; max-height: 72px; min-width: 48px; min-height: 72px; }");
            gtk::style_context_add_provider_for_display(
                &gtk::gdk::Display::default().unwrap(),
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let window = gtk::ApplicationWindow::new(app);
        window.set_default_size(600, 400);
        window.set_title(Some("scrolltest"));

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(true);
        if std::env::var("SCROLLTEST_VEXPAND").is_err() {
            list.set_vexpand(false);
        } else {
            list.set_vexpand(true);
        }
        if std::env::var("SCROLLTEST_LIST_START").is_ok() {
            list.set_valign(gtk::Align::Start);
        }
        list.set_hexpand(true);

        let tex = {
            let bytes = std::fs::read("/tmp/opencode/testcover.jpg").unwrap();
            let loader = gdk_pixbuf::PixbufLoader::new();
            loader.write(&bytes).ok().unwrap();
            loader.close().ok().unwrap();
            let src = loader.pixbuf().unwrap();
            let pb = src.scale_simple(48, 72, gdk_pixbuf::InterpType::Bilinear).unwrap();
            gtk::gdk::Texture::for_pixbuf(&pb)
        };

        let n: usize = std::env::var("SCROLLTEST_N").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
        for i in 0..n {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            if std::env::var("SCROLLTEST_ROW_START").is_ok() {
                row.set_valign(gtk::Align::Start);
            }
            let pic = gtk::Picture::new();
            pic.set_width_request(48);
            pic.set_height_request(72);
            pic.set_content_fit(gtk::ContentFit::Cover);
            pic.set_css_classes(&["cover-xs"]);
            pic.set_paintable(Some(&tex));
            row.append(&pic);
            let l = gtk::Label::new(Some(&format!("Kayıt {i} — uzun isim")));
            row.append(&l);
            list.append(&row);
        }

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scrolled.set_vexpand(true);
        scrolled.set_hexpand(true);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        if std::env::var("SCROLLTEST_NO_OVERLAY").is_ok() {
            scrolled.set_child(Some(&list));
        } else {
            let overlay = gtk::Overlay::new();
            overlay.set_child(Some(&list));
            scrolled.set_child(Some(&overlay));
        }
        content.append(&scrolled);
        window.set_child(Some(&content));
        window.present();

        glib::timeout_add_local(std::time::Duration::from_millis(1500), move || {
            let v = scrolled.vadjustment();
            let natural = v.upper();
            let visible = v.page_size();
            let pic = list.first_child()
                .and_then(|r| r.first_child())
                .and_then(|b| b.first_child())
                .unwrap();
            let a = pic.allocation();
            let (mh, nh, _, _) = pic.measure(gtk::Orientation::Vertical, -1);
            let (mw, nw, _, _) = pic.measure(gtk::Orientation::Horizontal, -1);
            println!("SCROLLTEST upper={natural} visible={visible} pic alloc=({},{}) min=({mw},{mh}) natural=({nw},{nh})", a.width(), a.height());
            glib::ControlFlow::Break
        });
    });
    app.run();
}
