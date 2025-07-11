use std::collections::BTreeMap;
use std::sync::Arc;
use chrono::{DateTime, Duration, TimeZone, Utc};
use teloxide::Bot;
use teloxide::requests::Requester;
use crate::db::Db;

pub async fn draw_uptime (
    bot: Bot,
    chat_id: teloxide::types::ChatId,
    db: Arc<Db>,
) -> anyhow::Result<()> {
    let lookback = 1;
    let now =  Utc::now();
    let since = now - Duration::hours(lookback);
    
    let metrics = db.query_metrics(since).await?;

    // 2. 用嵌套的 BTreeMap 存桶：alias -> (window_start -> total_loss)
    let mut alias_buckets: BTreeMap<String, BTreeMap<DateTime<Utc>, f64>> = BTreeMap::new();

    for (alias, ts, _latency, loss) in metrics {
        // 计算这个时间点对应的 15 分钟窗口起点
        let secs = ts.timestamp();
        let window_start = secs - (secs % (15 * 60));
        let bucket_ts = Utc.timestamp_opt(window_start, 0).single().unwrap();

        // 累加到对应 alias + 窗口
        alias_buckets
            .entry(alias.clone())
            .or_default()
            .entry(bucket_ts)
            .and_modify(|sum| *sum += loss)
            .or_insert(loss);
    }

    // // 3. 对每个 alias、每个窗口计算状态并输出
    // for (alias, buckets) in alias_buckets {
    //     println!("=== alias: {} ===", alias);
    //     for (window_start, total_loss) in buckets {
    //         let status = if (total_loss - 100.0).abs() < std::f64::EPSILON {
    //             "== 100%"
    //         } else if total_loss > 50.0 {
    //             "> 50%"
    //         } else {
    //             "< 50%"
    //         };
    //         println!(
    //             "[{} - {}): loss_sum = {:.2}%, status = {}",
    //             window_start.format("%Y-%m-%d %H:%M"),
    //             (window_start + Duration::minutes(15)).format("%H:%M"),
    //             total_loss,
    //             status
    //         );
    //     }
    //     println!();
    // }
    
    let mut message_to_send: String = format!("过去{} 小时延迟\n", lookback);
    for (alias, buckets) in alias_buckets {
        let mut line_text: String = format!("[{}]: ", alias);
        for(_window_start, total_loss) in buckets {
            let status = if total_loss <= (50.0 - f64::EPSILON) {
                "🟩"
            } else if total_loss > (50.0 - f64::EPSILON) && total_loss < (100.0 - f64::EPSILON) {
                "🟨"
            } else {
                "🟥"
            };
            line_text.push_str(&status);
        }
        line_text.push('\n');
        message_to_send.push_str(&line_text);
    }
    let _ = bot.send_message(chat_id, message_to_send).await;
    Ok(())
}