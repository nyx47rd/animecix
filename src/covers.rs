use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use crate::api::Client;

/// İstenen boyuta göre kesin (max/min/genişlik/yükseklik) kısıtlama içeren
/// bir CSS sağlayıcıyı yalnızca bir kez olmak üzere global olarak kaydeder.
/// Böylece kapak fotoğrafları, global stil dosyası yüklenmese veya parse
/// edilmese bile asla tam doku boyutuna (ör. 185px) şişmez.
fn ensure_size_provider(w: i32, h: i32) {
    static CACHE: Mutex<Option<HashMap<(i32, i32), ()>>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    if map.contains_key(&(w, h)) {
        return;
    }

    let css = gtk::CssProvider::new();
    css.load_from_string(&format!(
        ".cover-fixed-{w}-{h} {{ \
            min-width:{w}px !important; max-width:{w}px !important; width:{w}px !important; \
            min-height:{h}px !important; max-height:{h}px !important; height:{h}px !important; \
        }}"
    ));

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    map.insert((w, h), ());
}

/// Verilen boyutta, taşmayı engelleyen kesin kısıtlamalara sahip bir
/// gtk::Picture widget'ı oluşturur.
pub fn new_sized_picture(w: i32, h: i32) -> gtk::Picture {
    let pic = gtk::Picture::new();
    pic.set_width_request(w);
    pic.set_height_request(h);
    pic.set_hexpand(false);
    pic.set_vexpand(false);
    pic.set_can_shrink(true);
    pic.set_content_fit(gtk::ContentFit::Cover);

    let size_class = match w {
        0..=60 => "cover-thumb",
        61..=130 => "cover-header",
        131..=150 => "cover-shelf",
        _ => "cover-movie-header",
    };

    ensure_size_provider(w, h);
    let fixed_class = format!("cover-fixed-{w}-{h}");
    pic.set_css_classes(&["cover", size_class, &fixed_class]);
    pic
}

pub struct CoverManager {
    client: Arc<Client>,
    cache: Rc<RefCell<HashMap<String, Option<gtk::gdk::Texture>>>>,
    waiters: Rc<RefCell<HashMap<String, Vec<gtk::Picture>>>>,
    queue: Arc<Mutex<VecDeque<String>>>,
    active: Rc<Cell<usize>>,
}

impl CoverManager {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            cache: Rc::new(RefCell::new(HashMap::new())),
            waiters: Rc::new(RefCell::new(HashMap::new())),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            active: Rc::new(Cell::new(0)),
        }
    }

    pub fn clone_ref(&self) -> Self {
        Self {
            client: self.client.clone(),
            cache: self.cache.clone(),
            waiters: self.waiters.clone(),
            queue: self.queue.clone(),
            active: self.active.clone(),
        }
    }

    pub fn cover_picture(&self, url: Option<&str>, w: i32, h: i32) -> gtk::Picture {
        let pic = new_sized_picture(w, h);
        self.load_cover(url, &pic, w, h);
        pic
    }

    pub fn load_cover(&self, url: Option<&str>, pic: &gtk::Picture, w: i32, h: i32) {
        let Some(url) = url else { return };
        let url = url
            .replace("image.tmdb.org/t/p/original", "image.tmdb.org/t/p/w185")
            .replace("image.tmdb.org/t/p/w500", "image.tmdb.org/t/p/w185")
            .replace("image.tmdb.org/t/p/w342", "image.tmdb.org/t/p/w185");
        let key = format!("{url}@{w}x{h}");

        if let Some(Some(t)) = self.cache.borrow().get(&key) {
            pic.set_paintable(Some(t));
            return;
        }
        if let Some(None) = self.cache.borrow().get(&key) {
            return;
        }

        self.waiters.borrow_mut().entry(key).or_default().push(pic.clone());
        let mut q = self.queue.lock().unwrap();
        if !q.iter().any(|u| u == &url) {
            q.push_back(url);
        }
        drop(q);
        self.pump_covers();
    }

    pub fn scale_texture(bytes: &[u8], w: i32, h: i32) -> Option<gtk::gdk::Texture> {
        let loader = gdk_pixbuf::PixbufLoader::new();
        loader.write(bytes).ok()?;
        loader.close().ok()?;
        let src = loader.pixbuf()?;
        let pb = src.scale_simple(w, h, gdk_pixbuf::InterpType::Nearest)?;
        Some(gtk::gdk::Texture::for_pixbuf(&pb))
    }

    fn pump_covers(&self) {
        let max_workers = 12;
        let mut active = self.active.get();

        while active < max_workers {
            let url = self.queue.lock().unwrap().pop_front();
            let Some(url) = url else { break };

            self.active.set(active + 1);
            active += 1;

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
                    let curr = this.active.get();
                    if curr > 0 {
                        this.active.set(curr - 1);
                    }
                    this.pump_covers();
                    glib::ControlFlow::Break
                }
                Err(_) => glib::ControlFlow::Continue,
            });
        }
    }

    fn finish_cover(&self, url: &str, bytes: Option<Vec<u8>>) {
        let mut waiters = self.waiters.borrow_mut();
        let mut keys_to_remove = Vec::new();

        if let Some(b) = &bytes {
            let mut cache = self.cache.borrow_mut();
            if cache.len() > 40 {
                cache.clear();
            }

            for (key, pics) in waiters.iter() {
                let Some(rest) = key.strip_prefix(url) else { continue };
                let Some(rest) = rest.strip_prefix('@') else { continue };
                let Some((ws, hs)) = rest.split_once('x') else { continue };
                let (Ok(w), Ok(h)) = (ws.parse::<i32>(), hs.parse::<i32>()) else { continue };
                if let Some(t) = Self::scale_texture(b, w, h) {
                    for p in pics {
                        p.set_paintable(Some(&t));
                    }
                    cache.insert(key.clone(), Some(t));
                } else {
                    cache.insert(key.clone(), None);
                }
                keys_to_remove.push(key.clone());
            }
        } else {
            let mut cache = self.cache.borrow_mut();
            for (key, _) in waiters.iter() {
                if key.starts_with(url) {
                    cache.insert(key.clone(), None);
                    keys_to_remove.push(key.clone());
                }
            }
        }

        for k in keys_to_remove {
            waiters.remove(&k);
        }
    }
}

