//! Logging initialization and configuration.
//!
//! Provides logging setup with file output, rotation, and Windows path expansion.

use flexi_logger::{
    Cleanup, Criterion, DeferredNow, Duplicate, FileSpec, Logger, LoggerHandle, Naming, Record,
    WriteMode,
};
use log::LevelFilter;
use std::path::PathBuf;

/// Log level enum matching CLI flags.
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    /// Converts LogLevel to log::LevelFilter.
    pub fn to_level_filter(self) -> LevelFilter {
        match self {
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Warning => LevelFilter::Warn,
            LogLevel::Error => LevelFilter::Error,
        }
    }
}

/// Gets the logs directory path, expanding ~/.megatile/logs to Windows user profile.
fn get_logs_dir() -> Result<PathBuf, String> {
    // On Windows, use USERPROFILE environment variable
    let home_dir = std::env::var("USERPROFILE")
        .map_err(|_| "Failed to get USERPROFILE environment variable".to_string())?;

    let mut logs_path = PathBuf::from(home_dir);
    logs_path.push(".megatile");
    logs_path.push("logs");

    Ok(logs_path)
}

/// Formats log messages with timestamp, level, module, and message.
fn format_log(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(
        w,
        "{} [{}] {}: {}",
        now.format("%Y-%m-%d %H:%M:%S%.3f"),
        record.level(),
        record.module_path().unwrap_or("<unknown>"),
        record.args()
    )
}

/// Initializes logging with the specified log level.
///
/// # Arguments
/// * `level` - The log level to use
///
/// # Returns
/// * `Result<LoggerHandle, String>` - Logger handle on success, error message on failure
pub fn init_logging(level: LogLevel) -> Result<LoggerHandle, String> {
    let logs_dir = get_logs_dir()?;

    // Create logs directory if it doesn't exist
    std::fs::create_dir_all(&logs_dir).map_err(|e| {
        format!(
            "Failed to create logs directory {}: {}",
            logs_dir.display(),
            e
        )
    })?;

    // Configure file spec with timestamp in filename
    let file_spec = FileSpec::default()
        .directory(&logs_dir)
        .basename("megatile")
        .suffix("log");

    // Configure logger with rotation and cleanup
    let logger = Logger::try_with_str(level.to_level_filter().to_string())
        .map_err(|e| format!("Failed to create logger: {}", e))?
        .format(format_log)
        .log_to_file(file_spec)
        .write_mode(WriteMode::BufferAndFlush)
        .rotate(
            Criterion::Size(10 * 1024 * 1024), // 10 MB
            Naming::Timestamps,
            Cleanup::KeepLogFiles(7), // Keep 7 log files
        )
        .duplicate_to_stderr(Duplicate::Error)
        .append()
        .start()
        .map_err(|e| format!("Failed to start logger: {}", e))?;

    Ok(logger)
}
