use crate::config::Config;
use crate::db::Db;
use std::sync::Arc;
use teloxide::prelude::Requester;
use teloxide::types::ChatId;
use teloxide::{Bot, RequestError};

/// Handle the `/stop` command (admin only)
pub async fn stop_command(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    cfg: &Config,
    db: Arc<Db>,
) -> Result<(), RequestError> {
    if cfg.admins.contains(&user_id) {
        db.remove_subscription(chat_id.0).await.ok();
        bot.send_message(chat_id, "❌ 已取消订阅").await?;
    }
    Ok(())
}
