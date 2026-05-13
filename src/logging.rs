//! 日志系统
//!
//! 基于 `tracing` + `tracing-subscriber` + `tracing-appender` 实现。
//! 特性：
//! * **双输出** — 控制台（可读格式） + 文件（JSON 格式）
//! * **按月归档** — 文件名 `gridow.2026-05.log`，月份切换时自动创建新文件
//! * **线程安全** — `Arc<Mutex<LogFileManager>>` 保证多线程写入安全
//!
//! # 实现要点
//! 自定义 `SharedLogWriter` 实现 `MakeWriter` trait，
//! 包装一个内部的 `LogFileManager`，后者负责在月份变更时切换输出文件。

use chrono::Local;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::{fmt, EnvFilter, Registry};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// 日志文件管理器
///
/// 持有当前日志文件的句柄和当前月份字符串。
/// 当 `write` 调用时检测月份是否变更，若变更则自动打开新文件。
struct LogFileManager {
    file: Option<fs::File>,
    current_month: String,
    log_dir: PathBuf,
    prefix: String,
}

impl LogFileManager {
    fn new(log_dir: &PathBuf, prefix: &str) -> Self {
        let now = Local::now();
        let current_month = now.format("%Y-%m").to_string();

        Self {
            file: None,
            current_month,
            log_dir: log_dir.clone(),
            prefix: prefix.to_string(),
        }
    }

    /// 拼接日志文件路径：`{log_dir}/{prefix}.{YYYY-MM}.log`
    fn get_file_path(&self) -> PathBuf {
        self.log_dir.join(format!("{}.{}.log", self.prefix, self.current_month))
    }

    /// 获取或创建当前月份的日志文件句柄
    ///
    /// 若当前月份与上次不同，先关闭旧文件再创建新文件（实现按月滚动）。
    fn get_or_create_file(&mut self) -> io::Result<&fs::File> {
        let now = Local::now();
        let month = now.format("%Y-%m").to_string();

        // 月份变更 → 丢弃旧文件句柄，写入时将自动创建新文件
        if month != self.current_month {
            self.file = None;
            self.current_month = month;
        }

        if self.file.is_none() {
            let file_path = self.get_file_path();
            let file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)?;
            self.file = Some(file);
        }

        Ok(self.file.as_ref().unwrap())
    }

    /// 向日志文件写入数据并立即 flush
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = self.get_or_create_file()?;
        file.write_all(buf)?;
        file.flush()?;
        Ok(buf.len())
    }
}

/// 线程安全的日志写入器
///
/// 通过 `Arc<Mutex<>>` 共享 `LogFileManager`，
/// 实现 `Write` 和 `MakeWriter` trait 以被 `tracing-subscriber` 使用。
struct SharedLogWriter {
    manager: Arc<Mutex<LogFileManager>>,
}

impl SharedLogWriter {
    fn new(log_dir: &str, prefix: &str) -> Self {
        let log_path = PathBuf::from(log_dir);
        if !log_path.exists() {
            fs::create_dir_all(&log_path).expect("Failed to create log directory");
        }

        Self {
            manager: Arc::new(Mutex::new(LogFileManager::new(&log_path, prefix))),
        }
    }
}

impl Write for SharedLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut manager = self.manager.lock().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Failed to acquire lock")
        })?;
        manager.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `MakeWriter` 实现：每次 `make_writer` 返回一个新的 `SharedLogWriter`，
/// 但其内部的 `LogFileManager`（含文件句柄）是共享的。
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLogWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        Self {
            manager: self.manager.clone(),
        }
    }
}

// 安全标记：`SharedLogWriter` 通过 `Arc<Mutex<>>` 保证内部可变性的线程安全
unsafe impl Send for SharedLogWriter {}
unsafe impl Sync for SharedLogWriter {}

/// 初始化日志系统
///
/// 配置两个 Layer：
/// * **Console Layer** — 输出到 stdout，可读格式
/// * **File Layer**   — 输出到文件，JSON 格式，按月滚动
///
/// 同时配置 `EnvFilter`，可通过 `RUST_LOG` 环境变量动态控制日志级别。
pub fn init_logging(log_dir: &str) {
    let writer = SharedLogWriter::new(log_dir, "gridow");

    // 控制台输出：人类可读格式
    let console_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_writer(std::io::stdout);

    // 文件输出：JSON 格式，便于日志收集工具解析
    let file_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_writer(writer)
        .json()
        .with_ansi(false);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "gridow_web=debug,tower_http=debug".into());

    Registry::default()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!(
        "Logging initialized, log directory: {}, current time: {}",
        log_dir,
        Local::now().to_rfc3339()
    );
}
