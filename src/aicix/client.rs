use crate::aicix::state::{AicixState, MessageRole, OpenAIToolCall};
use crate::aicix::{prompts, tools};
use serde_json::{json, Value};
use std::sync::Arc;
use std::sync::mpsc;

const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";

pub struct AicixClient {
    state: Arc<std::sync::Mutex<AicixState>>,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Content(String),
    ToolCalls(Vec<OpenAIToolCall>),
    Done,
    Error(String),
}

impl AicixClient {
    pub fn new(state: Arc<std::sync::Mutex<AicixState>>) -> Self {
        Self { state }
    }

    pub fn state(&self) -> Arc<std::sync::Mutex<AicixState>> {
        self.state.clone()
    }

    pub fn build_request_body(&self) -> Result<Value, String> {
        let state = self.state.lock().unwrap();
        if !state.has_key() {
            return Err("API anahtarı girilmemiş. Ayar ekranından Groq API key ekleyin.".to_string());
        }

        let mut messages: Vec<Value> = Vec::new();
        messages.push(json!({
            "role": "system",
            "content": prompts::SYSTEM_PROMPT,
        }));
        for msg in state.history.iter() {
            let mut m = json!({
                "role": match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "tool",
                },
                "content": msg.content,
            });
            if let Some(ref id) = msg.tool_call_id {
                m["tool_call_id"] = json!(id);
            }
            if let Some(ref name) = msg.name {
                m["name"] = json!(name);
            }
            if let Some(ref calls) = msg.tool_calls {
                m["tool_calls"] = json!(calls);
            }
            messages.push(m);
        }

        let body = json!({
            "model": state.model,
            "messages": messages,
            "tools": tools::all_definitions_json(),
            "tool_choice": "auto",
            "temperature": 0.7,
            "max_tokens": 1024,
            "stream": true,
        });
        Ok(body)
    }

    pub fn send_streaming(
        &self,
        tx: mpsc::Sender<StreamEvent>,
    ) -> Result<(), String> {
        let body = self.build_request_body()?;
        let api_key = {
            let s = self.state.lock().unwrap();
            s.api_key.clone().unwrap_or_default()
        };
        let state = self.state.clone();

        std::thread::spawn(move || {
            let client = match reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(format!("client: {e}")));
                    return;
                }
            };

            let mut resp = match client
                .post(GROQ_ENDPOINT)
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .body(serde_json::to_string(&body).unwrap_or_default())
                .send()
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(format!("request: {e}")));
                    return;
                }
            };

            let status = resp.status().as_u16();
            if status >= 400 {
                let body = resp.text().unwrap_or_default();
                let _ = tx.send(StreamEvent::Error(format!(
                    "HTTP {status}: {}",
                    body.chars().take(300).collect::<String>()
                )));
                return;
            }

            let mut buffer = String::new();
            let mut accumulated_content = String::new();
            let mut tool_calls: Vec<OpenAIToolCall> = Vec::new();
            let mut current_calls: std::collections::HashMap<usize, (String, String, String)> =
                std::collections::HashMap::new();

            use std::io::Read;
            let mut chunk = [0u8; 4096];
            loop {
                let n = match resp.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) => {
                        let _ = tx.send(StreamEvent::Error(format!("stream: {e}")));
                        return;
                    }
                };
                buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));

                while let Some(pos) = buffer.find("\n\n") {
                    let event_str: String = buffer.drain(..pos + 2).collect();
                    for line in event_str.lines() {
                        if let Some(rest) = line.strip_prefix("data:") {
                            let data = rest.trim();
                            if data == "[DONE]" {
                                for (_, (id, name, args)) in current_calls.drain() {
                                    if !id.is_empty() {
                                        tool_calls.push(OpenAIToolCall {
                                            id,
                                            kind: "function".to_string(),
                                            function: crate::aicix::state::OpenAIFunctionCall {
                                                name,
                                                arguments: args,
                                            },
                                        });
                                    }
                                }
                                {
                                    let mut s = state.lock().unwrap();
                                    if !accumulated_content.is_empty() {
                                        s.push_assistant_text(&accumulated_content);
                                    }
                                    if !tool_calls.is_empty() {
                                        let calls = tool_calls.clone();
                                        s.push_assistant_with_tool_calls("", calls);
                                    }
                                }
                                let _ = tx.send(StreamEvent::ToolCalls(tool_calls));
                                let _ = tx.send(StreamEvent::Done);
                                return;
                            }
                            if let Ok(v) = serde_json::from_str::<Value>(data) {
                                let choices = v.get("choices").and_then(|c| c.as_array());
                                if let Some(arr) = choices {
                                    for choice in arr {
                                        let delta = choice.get("delta");
                                        if let Some(content) = delta
                                            .and_then(|d| d.get("content"))
                                            .and_then(|c| c.as_str())
                                        {
                                            if !content.is_empty() {
                                                accumulated_content.push_str(content);
                                                let _ = tx.send(StreamEvent::Content(content.to_string()));
                                            }
                                        }
                                        if let Some(tcs) = delta
                                            .and_then(|d| d.get("tool_calls"))
                                            .and_then(|t| t.as_array())
                                        {
                                            for tc in tcs {
                                                let idx = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                                                let entry = current_calls.entry(idx).or_insert_with(|| {
                                                    (String::new(), String::new(), String::new())
                                                });
                                                if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                                                    if !id.is_empty() {
                                                        entry.0 = id.to_string();
                                                    }
                                                }
                                                if let Some(func) = tc.get("function") {
                                                    if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                                        if !name.is_empty() {
                                                            entry.1.push_str(name);
                                                        }
                                                    }
                                                    if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                                                        entry.2.push_str(args);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            for (_, (id, name, args)) in current_calls.drain() {
                if !id.is_empty() {
                    tool_calls.push(OpenAIToolCall {
                        id,
                        kind: "function".to_string(),
                        function: crate::aicix::state::OpenAIFunctionCall {
                            name,
                            arguments: args,
                        },
                    });
                }
            }
            {
                let mut s = state.lock().unwrap();
                if !accumulated_content.is_empty() {
                    s.push_assistant_text(&accumulated_content);
                }
                if !tool_calls.is_empty() {
                    let calls = tool_calls.clone();
                    s.push_assistant_with_tool_calls("", calls);
                }
            }
            let _ = tx.send(StreamEvent::ToolCalls(tool_calls));
            let _ = tx.send(StreamEvent::Done);
        });
        Ok(())
    }

    pub fn send_test(&self) -> Result<String, String> {
        let state = self.state.lock().unwrap();
        if !state.has_key() {
            return Err("API anahtarı girilmemiş".to_string());
        }
        let api_key = state.api_key.clone().unwrap();
        let model = state.model.clone();
        drop(state);

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;
        let body = json!({
            "model": model,
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 5,
        });
        let resp = client
            .post(GROQ_ENDPOINT)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        if status == 200 {
            Ok(format!("Bağlantı başarılı (HTTP {status})"))
        } else {
            let body = resp.text().unwrap_or_default();
            Err(format!("HTTP {status}: {}", body.chars().take(200).collect::<String>()))
        }
    }
}
