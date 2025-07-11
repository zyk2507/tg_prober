use crate::commands::isonline::CmdResult;
use crate::config::Config;
use crate::db::Db;
use std::sync::Arc;
use teloxide::prelude::Requester;
use teloxide::types::ChatId;
use teloxide::{Bot, RequestError};

pub async fn start_command(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    cfg: &Config,
    db: Arc<Db>,
) -> CmdResult {
    if cfg.admins.contains(&user_id) {
        db.add_subscription(chat_id.0).await.ok();
        bot.send_message(chat_id, "✅ 已启用订阅").await?;
    }
    Ok(())
}
