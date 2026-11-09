//! Claude ajanı: Messages API + araç kullanımı döngüsü.
//!
//! Güvenlik sınırı: ajanın elindeki araçların **hiçbiri** sistemi değiştirmez.
//! `propose_cleanup` yalnızca bir plan oluşturur; uygulamak için kullanıcı arayüzde
//! "Onayla"ya basmalıdır (`plan.approve` ajanın erişebileceği bir araç değildir).

use crate::state::{findings_payload, Ctx};
use anyhow::{anyhow, bail, Context, Result};
use bosun_core::categories::CategoryId;
use bosun_core::plan::{CleanupPlan, Selection};
use bosun_core::util::human_bytes;
use serde_json::{json, Value};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MAX_TURNS: usize = 6;
const MAX_HISTORY: usize = 40;

const SYSTEM_PROMPT: &str = r#"Sen Bosun'sun: bir Linux masaüstünün bakımından sorumlu lostromo. Kullanıcıyla Türkçe, kısa ve net konuşursun.

Kurallar:
- Sistem hakkındaki her sayıyı araçlardan al; tahmin etme, uydurma. Bilmediğin bir şeyi araçla öğrenemiyorsan bilmediğini söyle.
- Hiçbir şeyi kendin silemez ya da değiştiremezsin. Temizlik önerini `propose_cleanup` ile sunarsın; kullanıcı arayüzdeki kartta onaylarsa uygulanır. Kullanıcıya "onay kartı gösterildi" de, "sildim" deme.
- Yönetici yetkisi gerektiren işler (journald, APT önbelleği, sysctl) için kullanıcıya çalıştırabileceği komutu ver ve ne yaptığını tek cümleyle açıkla.
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
            "description": "Anlık CPU, bellek, swap, disk doluluğu, sıcaklık, yük ortalaması ve son 1 saatin özetini döndürür.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "get_cleanup_candidates",
            "description": "Temizlik kategorilerini boyutları, güvenlik seviyeleri ve yöntemleriyle döndürür. `refresh` true ise önce yeniden tarar.",
            "input_schema": {"type": "object", "properties": {"refresh": {"type": "boolean"}}}
        }),
        json!({
            "name": "get_findings",
            "description": "Kural motorunun uyarılarını, ayar önerilerini (komutlarıyla) ve başarısız systemd servislerini döndürür.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "propose_cleanup",
            "description": "Seçilen kategoriler için bir temizlik planı oluşturur ve kullanıcıya onay kartı olarak gösterir. Hiçbir şeyi silmez.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "categories": {"type": "array", "items": {"type": "string", "enum": cats}},
                    "reason": {"type": "string", "description": "Kullanıcıya gösterilecek tek cümlelik gerekçe"}
                },
                "required": ["categories", "reason"]
            }
        }),
    ];
    if share_paths {
        t.push(json!({
            "name": "get_large_files",
            "description": "İndirilenler klasöründeki 500 MB'tan büyük dosyaları listeler (yalnızca bilgi; silme önerilemez).",
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

pub async fn chat(ctx: &Ctx, conversation_id: &str, message: &str) -> Result<ChatReply> {
    let (api_key, model, share_paths) = {
        let st = ctx.state.read().await;
        (st.cfg.api_key.clone(), st.cfg.model.clone(), st.cfg.share_file_paths)
    };
    let Some(api_key) = api_key.filter(|k| !k.trim().is_empty()) else {
        bail!("API anahtarı tanımlı değil. ~/.config/bosun/config.toml dosyasına api_key ekleyin ya da ANTHROPIC_API_KEY ortam değişkenini ayarlayın.");
    };

    let mut messages = {
        let st = ctx.state.read().await;
        st.chats.get(conversation_id).cloned().unwrap_or_default()
    };
    messages.push(json!({"role": "user", "content": message}));

    let mut reply = ChatReply { reply: String::new(), plans: vec![], tools_used: vec![] };
    let tool_defs = tools(share_paths);

    for _turn in 0..MAX_TURNS {
        let body = json!({
            "model": model,
            "max_tokens": 1500,
            "system": SYSTEM_PROMPT,
            "tools": tool_defs,
            "messages": messages,
        });
        let url = std::env::var("BOSUN_API_URL").unwrap_or_else(|_| API_URL.to_string());
        let resp = ctx
            .http
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .context("Claude API'ye bağlanılamadı")?;
        let status = resp.status();
        let v: Value = resp.json().await.context("API yanıtı okunamadı")?;
        if !status.is_success() {
            let msg = v["error"]["message"].as_str().unwrap_or("bilinmeyen hata");
            bail!("Claude API hatası ({status}): {msg}");
        }

        let content = v["content"].clone();
        messages.push(json!({"role": "assistant", "content": content}));

        let blocks = content.as_array().cloned().unwrap_or_default();
        let text: Vec<&str> = blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect();
        if !text.is_empty() {
            reply.reply = text.join("\n");
        }

        if v["stop_reason"] != "tool_use" {
            break;
        }

        let mut results = Vec::new();
        for b in blocks.iter().filter(|b| b["type"] == "tool_use") {
            let name = b["name"].as_str().unwrap_or_default();
            reply.tools_used.push(name.to_string());
            let (out, is_error) = match run_tool(ctx, name, &b["input"], &mut reply.plans).await {
                Ok(v) => (v, false),
                Err(e) => (json!({"error": e.to_string()}), true),
            };
            results.push(json!({
                "type": "tool_result",
                "tool_use_id": b["id"],
                "content": out.to_string(),
                "is_error": is_error,
            }));
        }
        messages.push(json!({"role": "user", "content": results}));
    }

    if reply.reply.is_empty() {
        reply.reply = "İsteğini tamamlayamadım; lütfen tekrar dener misin?".into();
    }

    trim_history(&mut messages);
    ctx.state.write().await.chats.insert(conversation_id.to_string(), messages);
    Ok(reply)
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

async fn run_tool(ctx: &Ctx, name: &str, input: &Value, plans: &mut Vec<CleanupPlan>) -> Result<Value> {
    match name {
        "get_system_snapshot" => {
            let st = ctx.state.read().await;
            let s = st.history.back().ok_or_else(|| anyhow!("henüz ölçüm yok"))?;
            let n = st.history.len().max(1) as f32;
            let avg_cpu = st.history.iter().map(|h| h.cpu_percent).sum::<f32>() / n;
            let peak_mem = st.history.iter().map(|h| h.mem_percent()).fold(0.0f32, f32::max);
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
            }))
        }
        "get_cleanup_candidates" => {
            let scan = if input["refresh"].as_bool().unwrap_or(false) { ctx.scan_now().await } else { ctx.ensure_scan().await };
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
                bail!("dosya yolu paylaşımı kapalı");
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
                let id = CategoryId::parse(s).ok_or_else(|| anyhow!("bilinmeyen kategori: {s}"))?;
                cats.push(id);
            }
            if cats.is_empty() {
                bail!("en az bir kategori seçilmeli");
            }
            let scan = ctx.ensure_scan().await;
            let sel = Selection {
                categories: cats,
                large_files: vec![],
                origin: "assistant".into(),
                reason: input["reason"].as_str().map(str::to_string),
            };
            let plan = CleanupPlan::create(&scan, &sel);
            ctx.state.write().await.plans.insert(plan.id, plan.clone());
            ctx.audit.append("plan_proposed", json!({"plan_id": plan.id, "origin": "assistant", "total_bytes": plan.total_bytes, "summary": plan.summary}));
            let out = json!({
                "plan_id": plan.id,
                "total": human_bytes(plan.total_bytes),
                "items": plan.items.len(),
                "permanent": plan.has_permanent,
                "blocked": plan.blocked,
                "note": "Plan kullanıcıya onay kartı olarak gösterildi. Sen uygulayamazsın; kullanıcı onaylarsa uygulanır.",
            });
            plans.push(plan.preview(15));
            Ok(out)
        }
        other => bail!("bilinmeyen araç: {other}"),
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
        assert!(!t.contains("\"journal\""), "yetki gerektiren kategoriler önerilemez");
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
