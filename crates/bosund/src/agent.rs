//! Claude ajanı: Messages API + araç kullanımı döngüsü.
//!
//! Güvenlik sınırı: ajanın elindeki araçların **hiçbiri** sistemi değiştirmez.
//! `propose_cleanup` yalnızca bir plan oluşturur; uygulamak için kullanıcı arayüzde
//! "Onayla"ya basmalıdır (`plan.approve` ajanın erişebileceği bir araç değildir).

use crate::state::{findings_payload, Ctx};
use anyhow::{anyhow, bail, Context, Result};
use bosun_core::categories::CategoryId;
use bosun_core::config::Provider;
use bosun_core::plan::{CleanupPlan, Selection};
use bosun_core::util::human_bytes;
use bosun_core::Lang;
use serde_json::{json, Value};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MAX_TURNS: usize = 6;
const MAX_HISTORY: usize = 40;

const SYSTEM_PROMPT_EN: &str = r#"You are Bosun, the boatswain looking after a Linux desktop. Talk to the user in English: short, concrete and plain.

Rules:
- Take every number about the system from the tools; never guess or invent. If a tool can't tell you something, say you don't know.
- You cannot delete or change anything yourself. Offer cleanups with `propose_cleanup`; they run only if the user approves the card shown in the app. Say "I've put an approval card up", never "I deleted".
- Work that needs administrator rights (system logs, APT cache, unused Flatpak runtimes, leftover packages, journald/swappiness/Snap settings) can be done from the Cleanup tab or the Overview's "Apply…" buttons; the system asks for the user's password. Point the user there, and also give the terminal command.
- Leftovers from old programs (broken links, broken shortcuts, old settings folders) are listed in the Cleanup tab. Settings folders are only a guess; tell the user to check each one.
- Emptying the trash is permanent; always say so when you suggest it.
- Never suggest deleting the user's personal files (Documents, Pictures, etc.). For large files in Downloads, only list them and let the user decide.
- Keep replies short enough to fit on a phone screen. Write sizes readably, like "1.4 GB".
"#;

const SYSTEM_PROMPT_TR: &str = r#"Sen Bosun'sun: bir Linux masaüstünün bakımından sorumlu lostromo. Kullanıcıyla Türkçe, kısa ve net konuşursun.

Kurallar:
- Sistem hakkındaki her sayıyı araçlardan al; tahmin etme, uydurma. Bilmediğin bir şeyi araçla öğrenemiyorsan bilmediğini söyle.
- Hiçbir şeyi kendin silemez ya da değiştiremezsin. Temizlik önerini `propose_cleanup` ile sunarsın; kullanıcı arayüzdeki kartta onaylarsa uygulanır. Kullanıcıya "onay kartı gösterildi" de, "sildim" deme.
- Yönetici yetkisi gereken işler (sistem günlükleri, APT önbelleği, kullanılmayan Flatpak ortamları, artık paketler, journald/swappiness/Snap ayarları) Temizlik sekmesinden ya da Genel bakış'taki "Uygula…" düğmeleriyle yapılabilir; sistem kullanıcının şifresini sorar. Kullanıcıyı oraya yönlendir, terminal komutunu da ver.
- Eski programlardan kalanlar (kırık bağlantılar, kırık kısayollar, eski ayar klasörleri) Temizlik sekmesinde listelenir. Ayar klasörleri yalnızca bir tahmindir; kullanıcıya her birine bakmasını söyle.
- Çöp kutusunu boşaltmak kalıcıdır; bunu önerirken mutlaka belirt.
- Kullanıcının kişisel dosyalarını (Belgeler, Resimler vb.) asla silmeyi önerme. İndirilenlerdeki büyük dosyaları yalnızca listeleyip kullanıcının karar vermesini iste.
- Yanıtların bir telefon ekranına sığacak uzunlukta olsun. Boyutları "1,4 GB" gibi okunur biçimde yaz.
"#;

fn tools(share_paths: bool) -> Value {
    let cats: Vec<&str> = bosun_core::categories::ALL
        .iter()
        .filter(|c| c.bulk_selectable())
        .map(|c| c.as_str())
        .collect();
    let mut t = vec![
        json!({
            "name": "get_system_snapshot",
            "description": "Current CPU, memory, swap, disk usage, temperature, load average and a summary of the last hour.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "get_cleanup_candidates",
            "description": "Cleanup categories with sizes, safety levels and methods. If `refresh` is true, rescans first.",
            "input_schema": {"type": "object", "properties": {"refresh": {"type": "boolean"}}}
        }),
        json!({
            "name": "get_findings",
            "description": "Rule-engine warnings, settings advice (with commands) and failed systemd units.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "propose_cleanup",
            "description": "Creates a cleanup plan for the chosen categories and shows it to the user as an approval card. Deletes nothing.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "categories": {"type": "array", "items": {"type": "string", "enum": cats}},
                    "reason": {"type": "string", "description": "One-sentence reason shown to the user, in the user's language"}
                },
                "required": ["categories", "reason"]
            }
        }),
    ];
    if share_paths {
        t.push(json!({
            "name": "get_large_files",
            "description": "Lists files over 500 MB in Downloads (information only; cannot be proposed for deletion).",
            "input_schema": {"type": "object", "properties": {}}
        }));
    }
    Value::Array(t)
}

pub struct ChatReply {
    pub reply: String,
    pub plans: Vec<CleanupPlan>,
    pub tools_used: Vec<String>,
}

/// Bir sohbet turunda kullanılacak sağlayıcı bilgileri.
struct Backend {
    provider: Provider,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
}

pub async fn chat(ctx: &Ctx, conversation_id: &str, message: &str) -> Result<ChatReply> {
    let (backend, share_paths, lang) = {
        let st = ctx.state.read().await;
        let c = &st.cfg;
        (
            Backend {
                provider: c.provider,
                model: c.effective_model(),
                api_key: c.key_for(c.provider).map(str::to_string),
                base_url: c.effective_base_url(),
            },
            c.share_file_paths,
            c.language,
        )
    };
    if !matches!(backend.provider, Provider::Custom) && backend.api_key.is_none() {
        bail!(
            "{}",
            lang.s(
                format!(
                    "No API key is set for {}. Add it in Settings → Chat.",
                    backend.provider.label()
                ),
                format!(
                    "{} için API anahtarı tanımlı değil. Ayarlar → Sohbet bölümünden ekleyin.",
                    backend.provider.label()
                ),
            )
        );
    }

    // Geçmiş sağlayıcının mesaj biçiminde tutulur; sağlayıcı değiştiyse sohbet yeniden başlar.
    let mut messages = {
        let st = ctx.state.read().await;
        match st.chats.get(conversation_id) {
            Some((p, m)) if *p == backend.provider => m.clone(),
            _ => vec![],
        }
    };
    messages.push(json!({"role": "user", "content": message}));

    let mut reply = ChatReply {
        reply: String::new(),
        plans: vec![],
        tools_used: vec![],
    };
    let system = if lang == Lang::Tr {
        SYSTEM_PROMPT_TR
    } else {
        SYSTEM_PROMPT_EN
    };
    let tool_defs = tools(share_paths);

    for turn in 0..MAX_TURNS {
        let (events, conv, turn_no) = (ctx.events.clone(), conversation_id.to_string(), turn);
        let on_delta = move |t: &str| {
            let _ = events.send(bosun_core::ipc::Event {
                event: "chat_delta".into(),
                data: json!({ "conversation_id": conv, "turn": turn_no, "text": t }),
            });
        };
        let calls = match backend.provider {
            Provider::Anthropic => {
                anthropic_turn(
                    ctx,
                    &backend,
                    system,
                    &tool_defs,
                    &mut messages,
                    &mut reply,
                    &on_delta,
                )
                .await?
            }
            _ => {
                openai_turn(
                    ctx,
                    &backend,
                    system,
                    &tool_defs,
                    &mut messages,
                    &mut reply,
                    &on_delta,
                )
                .await?
            }
        };
        if calls.is_empty() {
            break;
        }
        let mut results = Vec::new();
        for call in calls {
            ctx.emit(
                "chat_tool",
                json!({ "conversation_id": conversation_id, "name": call.name }),
            );
            reply.tools_used.push(call.name.clone());
            let (out, is_error) =
                match run_tool(ctx, &call.name, &call.input, &mut reply.plans, lang).await {
                    Ok(v) => (v, false),
                    Err(e) => (json!({"error": e.to_string()}), true),
                };
            results.push((call.id, truncate_tool_output(out.to_string()), is_error));
        }
        match backend.provider {
            Provider::Anthropic => messages.push(json!({
                "role": "user",
                "content": results.into_iter().map(|(id, content, is_error)| json!({
                    "type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error,
                })).collect::<Vec<_>>(),
            })),
            _ => {
                for (id, content, _) in results {
                    messages.push(json!({"role": "tool", "tool_call_id": id, "content": content}));
                }
            }
        }
    }

    if reply.reply.is_empty() {
        reply.reply = lang.s(
            "I couldn't finish that request; please try again.",
            "İsteğini tamamlayamadım; lütfen tekrar dener misin?",
        );
    }

    trim_history(&mut messages);
    let mut st = ctx.state.write().await;
    st.chats
        .insert(conversation_id.to_string(), (backend.provider, messages));
    if st.cfg.persist_chat {
        crate::state::save_chats(&st.chats);
    }
    Ok(reply)
}

/// Saklanan geçmişten (her iki API biçimi) yalnızca kullanıcının ve asistanın metin mesajları.
pub fn visible_messages(history: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for m in history {
        let role = m["role"].as_str().unwrap_or_default();
        let text = match &m["content"] {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        if (role == "user" || role == "assistant") && !text.trim().is_empty() {
            out.push(json!({ "role": role, "text": text }));
        }
    }
    out
}

/// Araç çıktısı modele gitmeden önce sınırlanır (bağlam ve maliyet için).
const MAX_TOOL_OUTPUT: usize = 12_000;

fn truncate_tool_output(s: String) -> String {
    if s.len() <= MAX_TOOL_OUTPUT {
        return s;
    }
    let mut cut = MAX_TOOL_OUTPUT;
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    format!(
        "{}… [truncated: {} more bytes not shown]",
        &s[..cut],
        s.len() - cut
    )
}

struct ToolCall {
    id: String,
    name: String,
    input: Value,
}

fn api_error(provider: Provider, status: reqwest::StatusCode, v: &Value) -> anyhow::Error {
    // Gemini bazen hatayı bir dizi içinde döndürür.
    let e = if v.is_array() {
        &v[0]["error"]
    } else {
        &v["error"]
    };
    let msg = e["message"]
        .as_str()
        .or_else(|| e.as_str())
        .unwrap_or("unknown error");
    anyhow!("{} API error ({status}): {msg}", provider.label())
}

/// Akıştan gelen her metin parçası için çağrılır.
type OnDelta<'a> = &'a (dyn Fn(&str) + Send + Sync);

/// Yanıt başarısızsa gövdeyi JSON hata olarak okur.
async fn check(provider: Provider, resp: reqwest::Response) -> Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    Err(api_error(provider, status, &v))
}

/// Anthropic Messages API ile akışlı bir tur. Araç çağrılarını döndürür (yoksa tur bitmiştir).
async fn anthropic_turn(
    ctx: &Ctx,
    b: &Backend,
    system: &str,
    tools: &Value,
    messages: &mut Vec<Value>,
    reply: &mut ChatReply,
    on_delta: OnDelta<'_>,
) -> Result<Vec<ToolCall>> {
    let body = json!({
        "model": b.model, "max_tokens": 1500, "system": system, "tools": tools,
        "messages": messages, "stream": true,
    });
    let url = std::env::var("BOSUN_API_URL").unwrap_or_else(|_| API_URL.to_string());
    let resp = ctx
        .http
        .post(&url)
        .header("x-api-key", b.api_key.as_deref().unwrap_or_default())
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .context("could not reach the Claude API")?;
    let mut resp = check(b.provider, resp).await?;
    let (mut sse, mut acc) = (
        crate::stream::Sse::default(),
        crate::stream::AnthropicAcc::default(),
    );
    while let Some(chunk) = resp
        .chunk()
        .await
        .context("the response stream broke off")?
    {
        for ev in sse.feed(&chunk) {
            if let Some(t) = acc.feed(&ev) {
                on_delta(&t);
            }
        }
    }
    if let Some(e) = acc.error.take() {
        bail!("{} API error: {e}", b.provider.label());
    }
    let (content, stop) = acc.finish();
    messages.push(json!({"role": "assistant", "content": content}));
    let blocks = content.as_array().cloned().unwrap_or_default();
    let text: Vec<&str> = blocks
        .iter()
        .filter(|x| x["type"] == "text")
        .filter_map(|x| x["text"].as_str())
        .collect();
    if !text.is_empty() {
        reply.reply = text.join("\n");
    }
    if stop.as_deref() != Some("tool_use") {
        return Ok(vec![]);
    }
    Ok(blocks
        .iter()
        .filter(|x| x["type"] == "tool_use")
        .map(|x| ToolCall {
            id: x["id"].as_str().unwrap_or_default().into(),
            name: x["name"].as_str().unwrap_or_default().into(),
            input: x["input"].clone(),
        })
        .collect())
}

/// Anthropic araç tanımlarını OpenAI "function" biçimine çevirir.
fn openai_tools(tools: &Value) -> Value {
    Value::Array(
        tools
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|t| {
                        json!({"type": "function", "function": {
                            "name": t["name"], "description": t["description"], "parameters": t["input_schema"],
                        }})
                    })
                    .collect()
            })
            .unwrap_or_default(),
    )
}

/// OpenAI uyumlu Chat Completions (xAI Grok, Google Gemini, OpenAI, Ollama…) ile akışlı bir tur.
async fn openai_turn(
    ctx: &Ctx,
    b: &Backend,
    system: &str,
    tools: &Value,
    messages: &mut Vec<Value>,
    reply: &mut ChatReply,
    on_delta: OnDelta<'_>,
) -> Result<Vec<ToolCall>> {
    let base = b
        .base_url
        .clone()
        .ok_or_else(|| anyhow!("no server address (base_url) is set for this provider"))?;
    let mut all = vec![json!({"role": "system", "content": system})];
    all.extend(messages.iter().cloned());
    let body = json!({
        "model": b.model, "messages": all, "tools": openai_tools(tools), "tool_choice": "auto", "stream": true,
    });
    let mut req = ctx
        .http
        .post(format!("{base}/chat/completions"))
        .json(&body);
    if let Some(k) = &b.api_key {
        req = req.bearer_auth(k);
    }
    let resp = req
        .send()
        .await
        .with_context(|| format!("could not reach {}", b.provider.label()))?;
    let mut resp = check(b.provider, resp).await?;
    let (mut sse, mut acc) = (
        crate::stream::Sse::default(),
        crate::stream::OpenAiAcc::default(),
    );
    while let Some(chunk) = resp
        .chunk()
        .await
        .context("the response stream broke off")?
    {
        for ev in sse.feed(&chunk) {
            if let Some(t) = acc.feed(&ev) {
                on_delta(&t);
            }
        }
    }
    if let Some(e) = acc.error.take() {
        bail!("{} API error: {e}", b.provider.label());
    }
    if !acc.text().trim().is_empty() {
        reply.reply = acc.text().to_string();
    }
    let (msg, calls) = acc.finish();
    messages.push(msg);
    Ok(calls
        .into_iter()
        .map(|(id, name, input)| ToolCall { id, name, input })
        .collect())
}

/// Geçmişi sınırla; kesimi düz metin bir kullanıcı mesajında yap ki araç sonuçları yetim kalmasın.
fn trim_history(messages: &mut Vec<Value>) {
    while messages.len() > MAX_HISTORY {
        messages.remove(0);
        while let Some(first) = messages.first() {
            if first["role"] == "user" && first["content"].is_string() {
                break;
            }
            messages.remove(0);
        }
    }
}

async fn run_tool(
    ctx: &Ctx,
    name: &str,
    input: &Value,
    plans: &mut Vec<CleanupPlan>,
    lang: Lang,
) -> Result<Value> {
    match name {
        "get_system_snapshot" => {
            let st = ctx.state.read().await;
            let s = st
                .history
                .back()
                .ok_or_else(|| anyhow!("no measurements yet"))?;
            let n = st.history.len().max(1) as f32;
            let avg_cpu = st.history.iter().map(|h| h.cpu_percent).sum::<f32>() / n;
            let peak_mem = st
                .history
                .iter()
                .map(|h| h.mem_percent())
                .fold(0.0f32, f32::max);
            Ok(json!({
                "host": st.host,
                "cpu_percent": s.cpu_percent,
                "cpu_avg_last_hour": avg_cpu,
                "cpu_count": s.cpu_count,
                "memory": {"used": human_bytes(s.mem_used), "total": human_bytes(s.mem_total), "percent": s.mem_percent(), "peak_percent_last_hour": peak_mem},
                "swap": {"used": human_bytes(s.swap_used), "total": human_bytes(s.swap_total), "percent": s.swap_percent()},
                "load_avg": s.load_avg,
                "uptime_hours": s.uptime_secs / 3600,
                "disks": s.disks.iter().map(|d| json!({
                    "mount": d.mount, "fs": d.fs, "used_percent": d.used_percent(),
                    "free": human_bytes(d.available), "total": human_bytes(d.total)
                })).collect::<Vec<_>>(),
                "max_temp_c": s.max_temp(),
                "network": {"download_per_sec": human_bytes(s.net_rx_bps), "upload_per_sec": human_bytes(s.net_tx_bps)},
                "battery": s.battery,
                "gpu": s.gpu.as_ref().map(|g| json!({
                    "name": g.name, "util_percent": g.util_percent,
                    "memory": format!("{} / {}", human_bytes(g.mem_used), human_bytes(g.mem_total)), "temp_c": g.temp_c,
                })),
            }))
        }
        "get_cleanup_candidates" => {
            let scan = if input["refresh"].as_bool().unwrap_or(false) {
                ctx.scan_now().await
            } else {
                ctx.ensure_scan().await
            };
            Ok(json!({
                "scanned_at": scan.ts,
                "reclaimable_safe": human_bytes(scan.reclaimable_bytes()),
                "categories": scan.categories.iter().map(|c| json!({
                    "id": c.id, "title": c.title, "size": human_bytes(c.bytes), "bytes": c.bytes,
                    "items": c.item_count, "safety": c.safety, "method": c.method,
                    "can_propose": c.bulk_selectable, "manual_command": c.manual_command,
                    "description": c.description,
                })).collect::<Vec<_>>(),
            }))
        }
        "get_findings" => {
            let st = ctx.state.read().await;
            Ok(findings_payload(&st))
        }
        "get_large_files" => {
            if !ctx.state.read().await.cfg.share_file_paths {
                bail!("file path sharing is disabled");
            }
            let scan = ctx.ensure_scan().await;
            let files = scan
                .get(CategoryId::LargeFiles)
                .map(|c| c.items.iter().take(20).map(|i| json!({"path": i.path, "size": human_bytes(i.bytes), "modified": i.modified})).collect::<Vec<_>>())
                .unwrap_or_default();
            Ok(json!({ "files": files }))
        }
        "propose_cleanup" => {
            let mut cats = Vec::new();
            for c in input["categories"].as_array().cloned().unwrap_or_default() {
                let s = c.as_str().unwrap_or_default();
                let id = CategoryId::parse(s).ok_or_else(|| anyhow!("unknown category: {s}"))?;
                cats.push(id);
            }
            if cats.is_empty() {
                bail!("choose at least one category");
            }
            let scan = ctx.ensure_scan().await;
            let sel = Selection {
                categories: cats,
                picked: vec![],
                origin: "assistant".into(),
                reason: input["reason"].as_str().map(str::to_string),
            };
            let plan = CleanupPlan::create(&scan, &sel, lang);
            ctx.state.write().await.plans.insert(plan.id, plan.clone());
            ctx.audit.append("plan_proposed", json!({"plan_id": plan.id, "origin": "assistant", "total_bytes": plan.total_bytes, "summary": plan.summary}));
            let out = json!({
                "plan_id": plan.id,
                "total": human_bytes(plan.total_bytes),
                "items": plan.items.len(),
                "permanent": plan.has_permanent,
                "blocked": plan.blocked,
                "note": "The plan is shown to the user as an approval card. You cannot execute it; it runs only if the user approves.",
            });
            plans.push(plan.preview(15));
            Ok(out)
        }
        other => bail!("unknown tool: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_never_include_an_execute_capability() {
        let t = tools(true).to_string();
        assert!(!t.contains("approve"));
        assert!(!t.contains("execute"));
        assert!(
            !t.contains("\"journal\""),
            "yetki gerektiren kategoriler önerilemez"
        );
    }

    #[test]
    fn long_tool_output_is_truncated_on_char_boundary() {
        let s = "ğ".repeat(10_000); // 2 bayt/karakter
        let t = truncate_tool_output(s);
        assert!(t.len() < 12_100);
        assert!(t.contains("[truncated:"));
        assert_eq!(truncate_tool_output("kısa".into()), "kısa");
    }

    #[test]
    fn converts_tools_to_openai_format() {
        let t = openai_tools(&tools(false));
        let first = &t[0];
        assert_eq!(first["type"], "function");
        assert_eq!(first["function"]["name"], "get_system_snapshot");
        assert!(first["function"]["parameters"].is_object());
    }

    #[test]
    fn trim_keeps_history_starting_with_user_text() {
        let mut m: Vec<Value> = (0..50)
            .flat_map(|i| {
                vec![
                    json!({"role": "user", "content": format!("q{i}")}),
                    json!({"role": "assistant", "content": [{"type": "tool_use"}]}),
                    json!({"role": "user", "content": [{"type": "tool_result"}]}),
                ]
            })
            .collect();
        trim_history(&mut m);
        assert!(m.len() <= MAX_HISTORY);
        assert!(m[0]["content"].is_string());
    }
}
