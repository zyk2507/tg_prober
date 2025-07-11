// src/db.rs
use chrono::{DateTime, Utc};
use rusqlite::{ffi, params, Connection, Error, ErrorCode, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 数据库客户端，内部持有一个异步互斥的 rusqlite::Connection
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    /// 打开数据库并初始化表结构
    pub async fn new(path: &str) -> Result<Self> {
        // 由于 rusqlite::Connection!Send，必须在同步上下文打开
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS subscriptions (
                chat_id   INTEGER PRIMARY KEY
            );
            CREATE TABLE IF NOT EXISTS metrics (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                alias     TEXT    NOT NULL,
                ts        DATETIME NOT NULL,
                latency   REAL    NOT NULL,
                loss_rate REAL    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_metrics_ts_alias
                ON metrics(ts, alias);
        "#,
        )?;
        Ok(Db {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 添加订阅
    pub async fn add_subscription(&self, chat_id: i64) -> Result<()> {
        let mut c = self.conn.lock().await;
        c.execute(
            "INSERT OR IGNORE INTO subscriptions(chat_id) VALUES(?1)",
            params![chat_id],
        )?;
        Ok(())
    }

    /// 取消订阅
    pub async fn remove_subscription(&self, chat_id: i64) -> Result<()> {
        let mut c = self.conn.lock().await;
        c.execute(
            "DELETE FROM subscriptions WHERE chat_id=?1",
            params![chat_id],
        )?;
        Ok(())
    }

    /// 检查是否已订阅
    pub async fn is_subscribed(&self, chat_id: i64) -> Result<bool> {
        let c = self.conn.lock().await;
        let exists: i32 = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE chat_id=?1)",
            params![chat_id],
            |r| r.get(0),
        )?;
        Ok(exists != 0)
    }

    /// 插入一次探测结果
    pub async fn insert_metric(
        &self,
        alias: &str,
        ts: DateTime<Utc>,
        latency: f64,
        loss_rate: f64,
    ) -> Result<()> {
        let mut c = self.conn.lock().await;
        c.execute(
            "INSERT INTO metrics(alias, ts, latency, loss_rate) VALUES(?1,?2,?3,?4)",
            params![alias, ts.naive_utc(), latency, loss_rate],
        )?;
        Ok(())
    }

    /// 查询过去 N 小时的延迟数据
    pub async fn query_metrics(
        &self,
        since: DateTime<Utc>,
    ) -> rusqlite::Result<Vec<(String, DateTime<Utc>, f64, f64)>> {
        let since_naive = since.naive_utc();
        let conn = self.conn.clone();

        // 1. spawn_blocking 并显式标注闭包返回 rusqlite::Result<…>
        let handle = tokio::task::spawn_blocking(
            move || -> rusqlite::Result<Vec<(String, DateTime<Utc>, f64, f64)>> {
                let c = conn.blocking_lock();
                let mut stmt = c.prepare(
                    "SELECT alias, ts, latency, loss_rate 
                     FROM metrics 
                     WHERE ts>=?1 
                     ORDER BY ts",
                )?;
                let rows = stmt.query_map(params![since_naive], |r| {
                    let alias: String = r.get(0)?;
                    let naive: chrono::NaiveDateTime = r.get(1)?;
                    let ts = DateTime::from_naive_utc_and_offset(naive, Utc);
                    let lat: f64 = r.get(2)?;
                    let loss: f64 = r.get(3)?;
                    Ok((alias, ts, lat, loss))
                })?;

                // collect() 会产出 rusqlite::Result<Vec<…>>
                rows.collect()
            },
        );

        // 2. 先处理 JoinError，然后再处理 closure 内部的 rusqlite::Error
        let rows = handle
            .await
            .map_err(|e| {
                Error::SqliteFailure(
                    ffi::Error::new(ErrorCode::Unknown as i32),
                    Some(format!("JoinError: {}", e)),
                )
            })?
            ?;

        Ok(rows)
    }
}
