use anyhow::Result;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub token: String,
    pub log_level: Option<String>,
    pub socks5_proxy: Option<String>,
    pub admins: Vec<i64>,
    pub targets: Vec<TargetConfig>,
    pub probe_count: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TargetConfig {
    pub address: String,
    pub alias: String,
}

impl Config {
    /// 从 `config.toml` 读取并解析出 `Config`
    pub fn load(path: &str) -> Result<Self> {
        let s = fs::read_to_string(path)?;
        let cfg: Config = toml::from_str(&s)?;
        Ok(cfg)
    }

    /// 获取日志级别（默认 "info"）
    pub fn log_level(&self) -> &str {
        self.log_level.as_deref().unwrap_or("info")
    }
}
