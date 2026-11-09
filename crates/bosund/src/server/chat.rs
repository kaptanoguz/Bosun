//! Sohbet: asistana soru, kayıtlı geçmiş, sıfırlama.

use super::param_str;
use crate::agent;
use crate::state::Ctx;
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub async fn handle(ctx: &Ctx, method: &str, p: &Value) -> Result<Value> {
    match method {
        "chat" => {
            let conv = p["conversation_id"].as_str().unwrap_or("default");
            let msg = param_str(p, "message")?;
            if msg.chars().count() > 4000 {
                bail!("message is too long");
            }
            let r = agent::chat(ctx, conv, msg).await?;
            Ok(json!({ "reply": r.reply, "plans": r.plans, "tools_used": r.tools_used }))
        }
        "chat.history" => {
            // Diskten geri yüklenen geçmişi arayüzün gösterebileceği düz mesajlara çevirir.
            let conv = p["conversation_id"].as_str().unwrap_or("default");
            let st = ctx.state.read().await;
            let msgs = st
                .chats
                .get(conv)
                .map(|(_, m)| crate::agent::visible_messages(m))
                .unwrap_or_default();
            Ok(json!({ "messages": msgs }))
        }
        "chat.reset" => {
            let conv = p["conversation_id"].as_str().unwrap_or("default");
            let mut st = ctx.state.write().await;
            st.chats.remove(conv);
            if st.cfg.persist_chat {
                crate::state::save_chats(&st.chats);
            }
            Ok(json!({"reset": true}))
        }
        _ => unreachable!("chat does not handle {method}"),
    }
}
