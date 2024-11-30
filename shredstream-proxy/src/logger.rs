use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicU8, Ordering};
use chrono::Utc;
use lazy_static::lazy_static;

#[derive(Debug, Clone, Copy)]
pub enum LogMode {
    Disabled = 0,
    ConsoleOnly = 1,
    FileOnly = 2,
    Both = 3,
}

impl From<u8> for LogMode {
    fn from(value: u8) -> Self {
        match value {
            0 => LogMode::Disabled,
            1 => LogMode::ConsoleOnly,
            2 => LogMode::FileOnly,
            3 => LogMode::Both,
            _ => LogMode::Disabled,
        }
    }
}

lazy_static! {
    static ref LOG_MODE: AtomicU8 = AtomicU8::new(LogMode::Both as u8);
}

pub fn set_log_mode(mode: LogMode) {
    LOG_MODE.store(mode as u8, Ordering::SeqCst);
}

pub fn get_log_mode() -> LogMode {
    LogMode::from(LOG_MODE.load(Ordering::SeqCst))
}

pub fn log(message: &str, log_file: &str) {
    let mode = get_log_mode();
    if matches!(mode, LogMode::Disabled) {
        return;
    }

    let now = Utc::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f");
    let log_message = format!("[{}] {}", timestamp, message);

    // Print to stdout if mode is ConsoleOnly or Both
    if matches!(mode, LogMode::ConsoleOnly | LogMode::Both) {
        println!("{}", log_message);
    }

    // Write to log file if mode is FileOnly or Both
    if matches!(mode, LogMode::FileOnly | LogMode::Both) {
        if let Ok(mut file) = OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_file)
        {
            let _ = writeln!(file, "{}", log_message);
        }
    }
}

#[macro_export]
macro_rules! log_info {
    ($log_file:expr, $($arg:tt)*) => {
        $crate::logger::log(&format!($($arg)*), $log_file);
    };
}
