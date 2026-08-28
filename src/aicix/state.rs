use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub is_card: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<CardPayload>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenAIToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: OpenAIFunctionCall,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenAIFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CardPayload {
    SearchResults {
        results: Vec<SearchResultCard>,
    },
    TitleDetail {
        title_id: u64,
        name: String,
        year: Option<i32>,
        rating: Option<f64>,
        episode_count: Option<i32>,
        description: Option<String>,
        poster: Option<String>,
    },
    EpisodeList {
        title_id: u64,
        title_name: String,
        episodes: Vec<EpisodeCard>,
    },
    FansubList {
        title_id: u64,
        episode: u64,
        season: u64,
        title_name: String,
        fansubs: Vec<FansubCard>,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResultCard {
    pub id: u64,
    pub name: String,
    pub romanji: Option<String>,
    pub english: Option<String>,
    pub year: Option<i32>,
    pub poster: Option<String>,
    pub rating: Option<f64>,
    pub episode_count: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpisodeCard {
    pub number: u64,
    pub name: String,
    pub duration: Option<String>,
    pub is_filler: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FansubCard {
    pub name: String,
    pub rating: f64,
    pub total_votes: i64,
    pub approved: bool,
    pub mirror_count: usize,
    pub hosts: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct AicixState {
    pub api_key: Option<String>,
    pub model: String,
    pub history: Vec<ChatMessage>,
    pub is_streaming: bool,
    pub pending_tool_calls: HashMap<String, OpenAIToolCall>,
    pub last_error: Option<String>,
}

impl AicixState {
    pub fn new() -> Self {
        Self {
            api_key: None,
            model: "qwen/qwen3.8-27b".to_string(),
            history: Vec::new(),
            is_streaming: false,
            pending_tool_calls: HashMap::new(),
            last_error: None,
        }
    }

    pub fn has_key(&self) -> bool {
        self.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false)
    }

    pub fn push_user(&mut self, text: &str) {
        self.history.push(ChatMessage {
            role: MessageRole::User,
            content: text.to_string(),
            tool_call_id: None,
            tool_calls: None,
            name: None,
            is_card: false,
            card: None,
        });
    }

    pub fn push_assistant_text(&mut self, text: &str) {
        self.history.push(ChatMessage {
            role: MessageRole::Assistant,
            content: text.to_string(),
            tool_call_id: None,
            tool_calls: None,
            name: None,
            is_card: false,
            card: None,
        });
    }

    pub fn push_tool_result(&mut self, call_id: &str, name: &str, result: &str) {
        self.history.push(ChatMessage {
            role: MessageRole::Tool,
            content: result.to_string(),
            tool_call_id: Some(call_id.to_string()),
            tool_calls: None,
            name: Some(name.to_string()),
            is_card: false,
            card: None,
        });
    }

    pub fn push_assistant_with_tool_calls(
        &mut self,
        content: &str,
        calls: Vec<OpenAIToolCall>,
    ) {
        self.history.push(ChatMessage {
            role: MessageRole::Assistant,
            content: content.to_string(),
            tool_call_id: None,
            tool_calls: Some(calls),
            name: None,
            is_card: false,
            card: None,
        });
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.pending_tool_calls.clear();
    }
}
