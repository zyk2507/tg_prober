use crate::config::Config;
use crate::db::Db;
use chrono::{Duration, Local, Utc};
use futures::future::join_all;
use std::{net::SocketAddr, sync::Arc};
use teloxide::prelude::Requester;
use teloxide::types::ChatId;
use teloxide::{Bot, RequestError};
use tokio::{
    task,
    time::{timeout, Duration as TokioDuration},
};

/// Result type for command handlers
pub type CmdResult = Result<(), RequestError>;

/// Handle the `/start` command (admin only)

/// Handle the `/stop` command (admin only)
pub async fn stop_command(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    cfg: &Config,
    db: Arc<Db>,
) -> CmdResult {
    if cfg.admins.contains(&user_id) {
        db.remove_subscription(chat_id.0).await.ok();
        bot.send_message(chat_id, "❌ 已取消订阅").await?;
    }
    Ok(())
}

/// Handle the `/isonline` command: live TCP probes
pub async fn isonline_command(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    cfg: &Config,
    db: Arc<Db>,
    targets: Vec<(SocketAddr, String)>,
) -> CmdResult {
    if !db.is_subscribed(chat_id.0).await.unwrap_or(false) {
        return Ok(());
    }

    // Send placeholder
    let placeholder = bot.send_message(chat_id, "🕒 正在测试中…").await?;
    let msg_id = placeholder.id;
    let bot_clone = bot.clone();
    let cfg_clone = cfg.clone();
    let targets_clone = targets.clone();
    let db_clone = db.clone();

    task::spawn(async move {
        let probe_count = cfg_clone.probe_count;
        let mut probes = Vec::new();

        for (addr, alias) in targets_clone {
            let alias = alias.clone();
            probes.push(task::spawn(async move {
                let mut latencies = Vec::new();
                let mut fails = 0;
                for _ in 0..probe_count {
                    let start = std::time::Instant::now();
                    let res = timeout(
                        TokioDuration::from_secs(1),
                        tokio::net::TcpStream::connect(&addr),
                    )
                    .await;
                    match res {
                        Ok(Ok(_)) => latencies.push(start.elapsed().as_millis() as u64),
                        _ => fails += 1,
                    }
                }
                let total = probe_count as u64;
                let success = total.saturating_sub(fails);
                let avg = if !latencies.is_empty() {
                    latencies.iter().sum::<u64>() / latencies.len() as u64
                } else {
                    0
                };
                let loss = (fails as f64) / (total as f64) * 100.0;
                (alias, success, total, avg, loss)
            }));
        }

        let results = join_all(probes).await;

        // Build report
        let mut report = format!(
            "🟢 测试完成，完成时间：{}\n结果：\n",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        for res in results {
            if let Ok((alias, success, total, avg, loss)) = res {
                let line = if success == total {
                    format!("{}: ✔ 全部成功，平均延迟 {} ms\n", alias, avg)
                } else if success == 0 {
                    format!("{}: ❌ 全部失败\n", alias)
                } else {
                    format!(
                        "{}: 部分成功，平均延迟 {} ms，丢包率 {:.1}%\n",
                        alias, avg, loss
                    )
                };
                report.push_str(&line);
            }
        }

        let _ = bot_clone.edit_message_text(chat_id, msg_id, report).await;
    });

    Ok(())
}

/// Handle the `/graph` command (stub for now)
pub async fn graph_command(bot: Bot, chat_id: ChatId, cfg: &Config, db: Arc<Db>) -> CmdResult {
    // To be implemented: draw_graph & send
    Ok(())
}
