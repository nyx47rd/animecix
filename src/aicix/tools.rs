use serde_json::{json, Value};

pub struct ToolDefinition {
    pub name: &'static str,
    pub definition: &'static str,
}

pub const TOOL_DEFINITIONS: &[ToolDefinition] = &[
    ToolDefinition {
        name: "search_anime",
        definition: r#"{
            "type": "function",
            "function": {
                "name": "search_anime",
                "description": "AnimeciX sitesinde anime, dizi veya film arar. Kullanıcı bir isim veya anahtar kelime söylediğinde çağır. Sonuçlar otomatik olarak kullanıcıya kart olarak gösterilir.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Arama sorgusu (anime adı, romanji adı, İngilizce adı veya anahtar kelime). Örnek: 'naruto', 'one piece', 'bocchi the rock'"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maksimum sonuç sayısı (varsayılan 5, max 10)",
                            "default": 5,
                            "minimum": 1,
                            "maximum": 10
                        }
                    },
                    "required": ["query"]
                }
            }
        }"#,
    },
    ToolDefinition {
        name: "get_title_details",
        definition: r#"{
            "type": "function",
            "function": {
                "name": "get_title_details",
                "description": "Bir anime/dizi/filmin detaylarını getirir: başlık, yıl, açıklama, bölüm sayısı, rating, poster. Genelde search_anime'dan sonra çağrılır. Sonuç otomatik olarak detay kartı olarak gösterilir.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title_id": {
                            "type": "integer",
                            "description": "AnimeciX title ID. search_anime sonucundaki 'id' alanından al."
                        }
                    },
                    "required": ["title_id"]
                }
            }
        }"#,
    },
    ToolDefinition {
        name: "get_episodes",
        definition: r#"{
            "type": "function",
            "function": {
                "name": "get_episodes",
                "description": "Bir animenin bölümlerini listeler. Her bölüm için numara, ad, süre, filler olup olmadığı döner. Sonuç liste kartı olarak gösterilir.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title_id": {
                            "type": "integer",
                            "description": "AnimeciX title ID"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maksimum bölüm sayısı (varsayılan 20)",
                            "default": 20,
                            "minimum": 1,
                            "maximum": 50
                        }
                    },
                    "required": ["title_id"]
                }
            }
        }"#,
    },
    ToolDefinition {
        name: "get_fansubs",
        definition: r#"{
            "type": "function",
            "function": {
                "name": "get_fansubs",
                "description": "Bir bölüm için mevcut çeviri/fansub seçeneklerini listeler. Her fansub için ad, rating, mirror sayısı, host listesi döner.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title_id": {
                            "type": "integer",
                            "description": "AnimeciX title ID"
                        },
                        "season": {
                            "type": "integer",
                            "description": "Sezon numarası (varsayılan 1)",
                            "default": 1
                        },
                        "episode": {
                            "type": "integer",
                            "description": "Bölüm numarası (varsayılan 1)",
                            "default": 1
                        }
                    },
                    "required": ["title_id"]
                }
            }
        }"#,
    },
    ToolDefinition {
        name: "open_title",
        definition: r#"{
            "type": "function",
            "function": {
                "name": "open_title",
                "description": "Bir anime/dizi/filmin detay sayfasını açar. Kullanıcı 'Bunu aç', 'göster', 'git' derse çağır.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title_id": {
                            "type": "integer",
                            "description": "AnimeciX title ID"
                        }
                    },
                    "required": ["title_id"]
                }
            }
        }"#,
    },
];

pub fn all_definitions_json() -> Vec<Value> {
    TOOL_DEFINITIONS
        .iter()
        .filter_map(|t| serde_json::from_str::<Value>(t.definition).ok())
        .collect()
}

pub fn find_tool(name: &str) -> Option<&'static ToolDefinition> {
    TOOL_DEFINITIONS.iter().find(|t| t.name == name)
}
