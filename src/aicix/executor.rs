use crate::aicix::{self, OpenAIToolCall};
use crate::app::App;
use serde_json::Value;

pub fn execute_tool(app: &App, call: &OpenAIToolCall) -> String {
    let args: Value = match serde_json::from_str(&call.function.arguments) {
        Ok(v) => v,
        Err(e) => return format!("Tool argümanları ayrıştırılamadı: {e}"),
    };
    match call.function.name.as_str() {
        "search_anime" => tool_search_anime(app, &args),
        "get_title_details" => tool_get_title_details(app, &args),
        "get_episodes" => tool_get_episodes(app, &args),
        "get_fansubs" => tool_get_fansubs(app, &args),
        "open_title" => tool_open_title(app, &args),
        other => format!("Bilinmeyen tool: {other}"),
    }
}

fn tool_search_anime(app: &App, args: &Value) -> String {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    if query.is_empty() {
        return "query parametresi gerekli".to_string();
    }
    match app.client.search(&query) {
        Ok(titles) => {
            let capped = titles.into_iter().take(limit).collect::<Vec<_>>();
            let results: Vec<aicix::SearchResultCard> = capped
                .iter()
                .map(|t| aicix::SearchResultCard {
                    id: t.id,
                    name: t.name.clone(),
                    romanji: t.name_romanji.clone(),
                    english: t.name_english.clone(),
                    year: t.year.map(|y| y as i32),
                    poster: t.poster.clone(),
                    rating: t.local_vote_average.as_ref().and_then(|s| s.parse().ok()),
                    episode_count: t.episode_count.map(|e| e as i32),
                })
                .collect();
            let card = aicix::CardPayload::SearchResults { results: results.clone() };
            {
                let mut s = app.aicix_state.lock().unwrap();
                let msg = aicix::ChatMessage {
                    role: aicix::MessageRole::Assistant,
                    content: format!("[{} sonuç bulundu]", results.len()),
                    tool_call_id: None,
                    tool_calls: None,
                    name: None,
                    is_card: true,
                    card: Some(card),
                };
                s.history.push(msg);
            }
            serde_json::to_string(&serde_json::json!({
                "count": results.len(),
                "results": results.iter().map(|r| serde_json::json!({
                    "id": r.id, "name": r.name,
                    "year": r.year, "rating": r.rating,
                    "episode_count": r.episode_count,
                })).collect::<Vec<_>>(),
            }))
            .unwrap_or_default()
        }
        Err(e) => format!("Arama hatası: {e}"),
    }
}

fn tool_get_title_details(app: &App, args: &Value) -> String {
    let title_id = match args.get("title_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return "title_id gerekli".to_string(),
    };
    let raw = match app.client.fetch_title_json(title_id) {
        Ok(v) => v,
        Err(e) => return format!("Detay alınamadı: {e}"),
    };
    let name = raw.get("title").and_then(|t| t.get("name")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let year = raw.get("title").and_then(|t| t.get("year")).and_then(|v| v.as_i64()).map(|y| y as i32);
    let episode_count = raw.get("title").and_then(|t| t.get("episode_count")).and_then(|v| v.as_i64()).map(|e| e as i32);
    let description = raw.get("title").and_then(|t| t.get("description")).and_then(|v| v.as_str()).map(|s| s.to_string());
    let rating: Option<f64> = raw.get("title").and_then(|t| t.get("local_vote_average")).and_then(|v| v.as_str()).and_then(|s| s.parse().ok());
    let poster = raw.get("title").and_then(|t| t.get("poster")).and_then(|v| v.as_str()).map(|s| s.to_string());

    let card = aicix::CardPayload::TitleDetail {
        title_id,
        name: name.clone(),
        year,
        rating,
        episode_count,
        description: description.clone(),
        poster: poster.clone(),
    };
    {
        let mut s = app.aicix_state.lock().unwrap();
        let msg = aicix::ChatMessage {
            role: aicix::MessageRole::Assistant,
            content: format!("[{name} detayları]"),
            tool_call_id: None,
            tool_calls: None,
            name: None,
            is_card: true,
            card: Some(card),
        };
        s.history.push(msg);
    }
    serde_json::to_string(&serde_json::json!({
        "id": title_id,
        "name": name,
        "year": year,
        "episode_count": episode_count,
        "rating": rating,
        "description": description,
    })).unwrap_or_default()
}

fn tool_get_episodes(app: &App, args: &Value) -> String {
    let title_id = match args.get("title_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return "title_id gerekli".to_string(),
    };
    let raw = match app.client.fetch_title_json(title_id) {
        Ok(v) => v,
        Err(e) => return format!("Bölüm listesi alınamadı: {e}"),
    };

    let episodes: Vec<aicix::EpisodeCard> = raw
        .get("episodes")
        .and_then(|e| e.as_array())
        .map(|arr| {
            arr.iter()
                .take(50)
                .map(|e| aicix::EpisodeCard {
                    number: e.get("episode_number").and_then(|n| n.as_u64()).unwrap_or(0),
                    name: e.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
                    duration: None,
                    is_filler: false,
                })
                .collect()
        })
        .unwrap_or_default();

    let title_name = raw.get("title").and_then(|t| t.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
    let card = aicix::CardPayload::EpisodeList {
        title_id,
        title_name: title_name.clone(),
        episodes: episodes.clone(),
    };
    {
        let mut s = app.aicix_state.lock().unwrap();
        let msg = aicix::ChatMessage {
            role: aicix::MessageRole::Assistant,
            content: format!("[{} bölüm]", episodes.len()),
            tool_call_id: None,
            tool_calls: None,
            name: None,
            is_card: true,
            card: Some(card),
        };
        s.history.push(msg);
    }
    serde_json::to_string(&serde_json::json!({
        "title_id": title_id,
        "count": episodes.len(),
        "episodes": episodes,
    })).unwrap_or_default()
}

fn tool_get_fansubs(app: &App, args: &Value) -> String {
    let title_id = match args.get("title_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return "title_id gerekli".to_string(),
    };
    let season = args.get("season").and_then(|v| v.as_u64()).unwrap_or(1);
    let episode = args.get("episode").and_then(|v| v.as_u64()).unwrap_or(1);
    match app.client.list_fansubs(title_id, episode, season) {
        Ok(fansubs) => {
            let fcards: Vec<aicix::FansubCard> = fansubs.iter().map(|f| aicix::FansubCard {
                name: f.name.clone(),
                rating: f.rating,
                total_votes: f.total_votes,
                approved: f.approved_only,
                mirror_count: f.mirror_count,
                hosts: f.hosts.clone(),
            }).collect();
            let card = aicix::CardPayload::FansubList {
                title_id,
                episode,
                season,
                title_name: format!("S{:02}E{:02}", season, episode),
                fansubs: fcards.clone(),
            };
            {
                let mut s = app.aicix_state.lock().unwrap();
                let msg = aicix::ChatMessage {
                    role: aicix::MessageRole::Assistant,
                    content: format!("[{} fansub]", fcards.len()),
                    tool_call_id: None,
                    tool_calls: None,
                    name: None,
                    is_card: true,
                    card: Some(card),
                };
                s.history.push(msg);
            }
            serde_json::to_string(&serde_json::json!({
                "title_id": title_id,
                "season": season,
                "episode": episode,
                "fansubs": fcards,
            })).unwrap_or_default()
        }
        Err(e) => format!("Fansub listesi alınamadı: {e}"),
    }
}

fn tool_open_title(app: &App, args: &Value) -> String {
    let title_id = match args.get("title_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return "title_id gerekli".to_string(),
    };
    let raw = match app.client.fetch_title_json(title_id) {
        Ok(v) => v,
        Err(e) => return format!("Title alınamadı: {e}"),
    };
    let title = match crate::api::Client::parse_title_from_json(&raw) {
        Ok(t) => t,
        Err(e) => return format!("Title parse: {e}"),
    };
    let title_name = title.name.clone();
    let this = app.clone_ref();
    glib::MainContext::default().spawn_local(async move {
        let enriched = this.client.enrich_title(&title);
        this.open_episodes(enriched);
    });
    format!("{} açılıyor (id={})", title_name, title_id)
}
