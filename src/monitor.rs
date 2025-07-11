// src/monitor.rs
use crate::{config::Config, db::Db};
use chrono::Utc;
use log::debug;
use std::sync::Arc;
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{net::TcpStream, time};

pub fn spawn_monitor(
    cfg: Config,
    db: Arc<Db>, // ← must be Arc<Db>, not Db or Arc<Mutex<...>>
    targets: Vec<(SocketAddr, String)>,
) {
    tokio::spawn(async move {
        debug!("Spawning monitor");
        let interval = Duration::from_secs(cfg.probe_count as u64);
        loop {
            debug!("Checking interval");
            let now = Utc::now();
            for (sock, alias) in &targets {
                let mut latencies: Vec<f64> = Vec::new();
                let mut fails = 0;
                for _ in 0..cfg.probe_count {
                    let start = Instant::now();
                    if time::timeout(Duration::from_secs(1), TcpStream::connect(sock))
                        .await
                        .is_err()
                    {
                        fails += 1;
                    } else {
                        latencies.push(start.elapsed().as_millis() as f64);
                    }
                }
                let avg = if latencies.is_empty() {
                    0.0
                } else {
                    latencies.iter().sum::<f64>() / latencies.len() as f64
                };
                let loss = fails as f64 / (cfg.probe_count as f64) * 100.0;

                if let Err(e) = db.insert_metric(alias, now, avg, loss).await {
                    log::error!("写入 metrics 失败 [{}]: {}", alias, e);
                }
            }
            time::sleep(interval).await;
        }
    });
}
