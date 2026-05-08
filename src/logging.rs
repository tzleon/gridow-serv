use chrono::Local;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::{fmt, EnvFilter, Registry};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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

    fn get_file_path(&self) -> PathBuf {
        self.log_dir.join(format!("{}.{}.log", self.prefix, self.current_month))
    }

    fn get_or_create_file(&mut self) -> io::Result<&fs::File> {
        let now = Local::now();
        let month = now.format("%Y-%m").to_string();
        
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

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = self.get_or_create_file()?;
        file.write_all(buf)?;
        file.flush()?;
        Ok(buf.len())
    }
}

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

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLogWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        Self {
            manager: self.manager.clone(),
        }
    }
}

unsafe impl Send for SharedLogWriter {}
unsafe impl Sync for SharedLogWriter {}

pub fn init_logging(log_dir: &str) {
    let writer = SharedLogWriter::new(log_dir, "gridow");

    let console_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_writer(std::io::stdout);

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

pub fn get_current_log_file(log_dir: &str) -> String {
    let now = Local::now();
    let filename = format!("gridow.{}.log", now.format("%Y-%m"));
    PathBuf::from(log_dir).join(filename).display().to_string()
}