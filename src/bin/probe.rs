#[path = "../api.rs"]
mod api;

fn main() {
    let c = api::Client::new();
    match c.home_lists() {
        Ok(cats) => {
            println!("KATEGORİ SAYISI: {}", cats.len());
            for cat in &cats {
                println!(
                    "  CAT: {} ({} items) | ilk: {:?}",
                    cat.name,
                    cat.items.len(),
                    cat.items.first().map(|t| (&t.name, t.id, &t.poster))
                );
            }
        }
        Err(e) => println!("home_lists HATASI: {e}"),
    }
    let t = api::Title {
        id: 13288,
        name: "Oh Boy, Was I Wrong About Her".to_string(),
        year: None,
        title_type: Some("anime".to_string()),
        poster: None,
        description: None,
        season_count: Some(1),
    };
    match c.episodes(&t) {
        Ok(eps) => {
            println!("BÖLÜM SAYISI (13288): {}", eps.len());
            for e in &eps {
                println!("  S{:02}E{:02} {}", e.season, e.episode, e.name);
            }
        }
        Err(e) => println!("episodes HATASI: {e}"),
    }
    let jj = api::Title {
        id: 7352,
        name: "Jujutsu Kaisen".to_string(),
        year: None,
        title_type: Some("anime".to_string()),
        poster: None,
        description: None,
        season_count: Some(3),
    };
    match c.episodes(&jj) {
        Ok(eps) => {
            println!("BÖLÜM SAYISI (7352, 3 sezon): {}", eps.len());
            for e in &eps {
                println!("  S{:02}E{:02} {}", e.season, e.episode, e.name);
            }
        }
        Err(e) => println!("episodes HATASI: {e}"),
    }
    let m = api::Title {
        id: 7457,
        name: "Mugen Train".to_string(),
        year: None,
        title_type: Some("movie".to_string()),
        poster: None,
        description: None,
        season_count: None,
    };
    match c.episodes(&m) {
        Ok(eps) => {
            println!("FİLM BÖLÜM SAYISI (7457): {}", eps.len());
            for e in &eps {
                println!("  S{:02}E{:02} {}", e.season, e.episode, e.name);
            }
            if let Some(e) = eps.first() {
                match c.resolve_movie(7457) {
                    Ok(u) => println!("FİLM OYNATMA URL: {}", u),
                    Err(e) => println!("resolve_movie HATASI: {e}"),
                }
            }
        }
        Err(e) => println!("film episodes HATASI: {e}"),
    }
    match c.search("frieren") {
        Ok(r) => println!("ARAMA: {} sonuç | ilk: {:?}", r.len(), r.first().map(|t| &t.name)),
        Err(e) => println!("search HATASI: {e}"),
    }
    match c.resolve(7352, 1, 1) {
        Ok(u) => println!("DİZİ ÇEVİRİ URL (7352 s1e1): {}", u),
        Err(e) => println!("resolve HATASI: {e}"),
    }
    match c.resolve(13288, 1, 1) {
        Ok(u) => println!("DİZİ ÇEVİRİ URL (13288 s1e1): {}", u),
        Err(e) => println!("resolve HATASI: {e}"),
    }
    match c.resolve_movie(7457) {
        Ok(u) => println!("FİLM OYNATMA URL (7457): {}", u),
        Err(e) => println!("resolve_movie HATASI: {e}"),
    }
    match c.resolve_movie(7542) {
        Ok(u) => println!("FİLM OYNATMA URL (7542): {}", u),
        Err(e) => println!("resolve_movie HATASI: {e}"),
    }
    match c.resolve_movie(8718) {
        Ok(u) => println!("FİLM OYNATMA URL (8718): {}", u),
        Err(e) => println!("resolve_movie HATASI: {e}"),
    }
}