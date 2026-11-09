//! Sağlayıcı akışlarının (Server-Sent Events) ayrıştırılması. Ağdan bağımsız, saf mantık:
//! baytlar → JSON olaylar → metin parçaları ve tamamlanmış mesaj.

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// SSE satır ayrıştırıcı. Yalnızca `data:` satırlarını JSON olarak döndürür; `[DONE]` atlanır.
#[derive(Default)]
pub struct Sse {
    buf: Vec<u8>,
}

impl Sse {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Value> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                if let Ok(v) = serde_json::from_str(data) {
                    out.push(v);
                }
            }
        }
        out
    }
}

/// Anthropic Messages akışından içerik bloklarını yeniden kurar.
#[derive(Default)]
pub struct AnthropicAcc {
    blocks: BTreeMap<u64, Value>,
    partial_json: BTreeMap<u64, String>,
    pub stop_reason: Option<String>,
    pub error: Option<String>,
}

impl AnthropicAcc {
    /// Bir olayı işler; varsa yeni metin parçasını döndürür.
    pub fn feed(&mut self, ev: &Value) -> Option<String> {
        let idx = ev["index"].as_u64().unwrap_or(0);
        match ev["type"].as_str()? {
            "content_block_start" => {
                let mut b = ev["content_block"].clone();
                if b["type"] == "text" && b["text"].is_null() {
                    b["text"] = json!("");
                }
                self.blocks.insert(idx, b);
                None
            }
            "content_block_delta" => {
                let d = &ev["delta"];
                match d["type"].as_str()? {
                    "text_delta" => {
                        let t = d["text"].as_str()?.to_string();
                        if let Some(b) = self.blocks.get_mut(&idx) {
                            let cur = b["text"].as_str().unwrap_or("").to_string();
                            b["text"] = json!(cur + &t);
                        }
                        Some(t)
                    }
                    "input_json_delta" => {
                        self.partial_json
                            .entry(idx)
                            .or_default()
                            .push_str(d["partial_json"].as_str().unwrap_or(""));
                        None
                    }
                    _ => None,
                }
            }
            "message_delta" => {
                if let Some(r) = ev["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(r.into());
                }
                None
            }
            "error" => {
                self.error = ev["error"]["message"].as_str().map(str::to_string);
                None
            }
            _ => None,
        }
    }

    /// Tamamlanan içerik blokları (araç girdileri JSON'a çevrilmiş olarak).
    pub fn finish(mut self) -> (Value, Option<String>) {
        for (idx, raw) in std::mem::take(&mut self.partial_json) {
            if let Some(b) = self.blocks.get_mut(&idx) {
                b["input"] = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
            }
        }
        for b in self.blocks.values_mut() {
            if b["type"] == "tool_use" && !b["input"].is_object() {
                b["input"] = json!({});
            }
        }
        (
            Value::Array(self.blocks.into_values().collect()),
            self.stop_reason,
        )
    }
}

/// OpenAI uyumlu Chat Completions akışından mesajı ve araç çağrılarını yeniden kurar.
#[derive(Default)]
pub struct OpenAiAcc {
    content: String,
    calls: BTreeMap<u64, (String, String, String)>,
    pub finish_reason: Option<String>,
    pub error: Option<String>,
}

impl OpenAiAcc {
    pub fn feed(&mut self, ev: &Value) -> Option<String> {
        if let Some(e) = ev["error"]["message"].as_str() {
            self.error = Some(e.into());
            return None;
        }
        let ch = &ev["choices"][0];
        if let Some(r) = ch["finish_reason"].as_str() {
            self.finish_reason = Some(r.into());
        }
        let d = &ch["delta"];
        if let Some(calls) = d["tool_calls"].as_array() {
            for (n, c) in calls.iter().enumerate() {
                let idx = c["index"].as_u64().unwrap_or(n as u64);
                let e = self.calls.entry(idx).or_default();
                if let Some(id) = c["id"].as_str() {
                    e.0 = id.into();
                }
                if let Some(name) = c["function"]["name"].as_str() {
                    e.1.push_str(name);
                }
                if let Some(a) = c["function"]["arguments"].as_str() {
                    e.2.push_str(a);
                }
            }
        }
        let t = d["content"].as_str().filter(|t| !t.is_empty())?;
        self.content.push_str(t);
        Some(t.to_string())
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    /// Geçmişe eklenecek asistan mesajı ve araç çağrıları: (id, ad, girdi).
    pub fn finish(self) -> (Value, Vec<(String, String, Value)>) {
        let calls: Vec<(String, String, Value)> = self
            .calls
            .into_values()
            .enumerate()
            .map(|(n, (id, name, args))| {
                let id = if id.is_empty() {
                    format!("call_{n}")
                } else {
                    id
                };
                let input = serde_json::from_str(&args).unwrap_or_else(|_| json!({}));
                (id, name, input)
            })
            .collect();
        let mut msg = json!({"role": "assistant", "content": self.content});
        if !calls.is_empty() {
            msg["tool_calls"] = Value::Array(
                calls
                    .iter()
                    .map(|(id, name, input)| {
                        json!({"id": id, "type": "function", "function": {"name": name, "arguments": input.to_string()}})
                    })
                    .collect(),
            );
        }
        (msg, calls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_handles_split_chunks_and_done() {
        let mut s = Sse::default();
        assert!(s.feed(b"event: x\ndata: {\"a\":").is_empty());
        let v = s.feed(b"1}\n\ndata: [DONE]\n");
        assert_eq!(v, vec![json!({"a": 1})]);
    }

    #[test]
    fn anthropic_stream_rebuilds_text_and_tool_use() {
        let mut a = AnthropicAcc::default();
        let evs = [
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Bakı"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"yorum."}}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t1","name":"get_findings","input":{}}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"refr"}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"esh\":true}"}}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"}}),
        ];
        let text: String = evs.iter().filter_map(|e| a.feed(e)).collect();
        assert_eq!(text, "Bakıyorum.");
        let (content, stop) = a.finish();
        assert_eq!(stop.as_deref(), Some("tool_use"));
        assert_eq!(content[0]["text"], "Bakıyorum.");
        assert_eq!(content[1]["input"], json!({"refresh": true}));
    }

    #[test]
    fn openai_stream_rebuilds_tool_calls() {
        let mut o = OpenAiAcc::default();
        let evs = [
            json!({"choices":[{"delta":{"content":"Hi "}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"get_","arguments":""}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"findings","arguments":"{}"}}]}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
        ];
        let text: String = evs.iter().filter_map(|e| o.feed(e)).collect();
        assert_eq!(text, "Hi ");
        assert_eq!(o.finish_reason.as_deref(), Some("tool_calls"));
        let (msg, calls) = o.finish();
        assert_eq!(calls[0].1, "get_findings");
        assert_eq!(msg["tool_calls"][0]["id"], "c1");
    }
}
