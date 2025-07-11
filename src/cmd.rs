// src/cmd.rs

//! Central command dispatcher
use crate::commands::{graph, isonline, start, stop, uptime};
use crate::config::Config;
use crate::db::Db;
use std::net::SocketAddr;
use std::sync::Arc;
use futures::TryFutureExt;
use teloxide::types::ChatKind;
use teloxide::Bot;
use teloxide::{dptree, macros::BotCommands, prelude::*};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "这些命令可用:")]
enum Command {
    #[command(description = "启用订阅 (仅限管理员)")]
    Start,
    #[command(description = "取消订阅 (仅限管理员)")]
    Stop,
    #[command(description = "检查在线状态")]
    Isonline,
    #[command(description = "获取延迟曲线")]
    Graph,
    #[command(description = "简单获取前2小时在线状态")]
    Uptime,
}

/// Mount this dispatcher in main.rs:
///
/// Dispatcher::builder(bot.clone(), handler)
///     .dependencies(dptree::deps![bot, cfg, db, targets])
///     .enable_ctrlc_handler()
///     .build()
///     .dispatch()
///     .await;
pub async fn cmd_dispatch(bot: Bot, cfg: Config, db: Arc<Db>, targets: Vec<(SocketAddr, String)>) {
    let handler = Update::filter_message()
        .filter_command::<Command>()
        .endpoint(handle_cmd);

    Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![bot, cfg, db, targets])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn handle_cmd(
    bot: Bot,
    msg: Message,
    cmd: Command,
    cfg: Config,
    db: Arc<Db>,
    targets: Vec<(SocketAddr, String)>,
) -> ResponseResult<()> {
    let chat_id = msg.chat.id;
    // only group chats
    if !matches!(msg.chat.kind, ChatKind::Public(_)) {
        return Ok(());
    }
    let user_id = match msg.from() {
        Some(u) => u.id.0 as i64,
        None => return Ok(()),
    };
    match cmd {
        Command::Start => {
            start::start_command(bot.clone(), chat_id, user_id, &cfg, db.clone()).await?;
        }
        Command::Stop => {
            stop::stop_command(bot.clone(), chat_id, user_id, &cfg, db.clone()).await?;
        }
        Command::Isonline => {
            isonline::isonline_command(
                bot.clone(),
                chat_id,
                user_id,
                &cfg,
                Arc::clone(&db),
                targets.clone(),
            )
            .await?;
        }
        Command::Graph => {
            if db.is_subscribed(chat_id.0).await.unwrap_or(false) {
                // Call graph_command and handle its Result directly
                match graph::graph_command(bot.clone(), chat_id, db.clone()).await {
                    Ok(()) => {
                        // success—nothing more to do
                    }
                    Err(e) => {
                        // send an error message back to the chat
                        let _ = bot
                            .send_message(chat_id, format!("❌ 绘制图表失败: {}", e))
                            .await;
                    }
                }
            }
        }
        Command::Uptime => {
            match uptime::draw_uptime(bot.clone(),  chat_id, db.clone()).await {
                Ok(_) => {}
                Err(e) => {
                    let _ = bot
                        .send_message(chat_id, format!("❌ 绘制过去二小时在线状态失败: {}", e))
                        .await;
                }
            }
        }
    }
    Ok(())
}
