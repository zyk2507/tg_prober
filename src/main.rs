// src/main.rs

mod cmd;
mod commands;
mod config;
mod db;
mod monitor;

use anyhow::Result;
use chrono::Local;
use env_logger::Builder;
use log::{debug, info};
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use teloxide::Bot;
use tokio::sync::Mutex;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    // —— 加载配置 —— //
    let cfg = config::Config::load("config.toml")?;

    // —— 初始化日志 —— //
    Builder::new()
        .format(|buf, rec| {
            writeln!(
                buf,
                "[{} {:<5}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                rec.level(),
                rec.args()
            )
        })
        .filter_level(cfg.log_level().parse()?)
        .init();
    info!("日志级别 = {}", cfg.log_level());

    // —— 初始化数据库 —— //
    let db = db::Db::new("db.db").await?;
    let db = Arc::new(db); // shareable cloneable Db
    info!("Database Initialization Complete");
    // —— 构造监测目标列表 —— //
    let targets: Vec<(SocketAddr, String)> = cfg
        .targets
        .iter()
        .map(|t| (t.address.parse().unwrap(), t.alias.clone()))
        .collect();
    info!("targets: {:?}", targets);

    // —— 启动后台监测任务 —— //
    monitor::spawn_monitor(cfg.clone(), db.clone(), targets.clone());
    info!("Spawning {} targets", targets.len());

    // —— 启动 Telegram 命令分发 —— //
    let bot = Bot::new(cfg.token.clone());
    cmd::cmd_dispatch(bot, cfg, db, targets).await;
    info!("Dispatcher stopped");
    info!("Starting to Process Telegram Messages");
    Ok(())
}
