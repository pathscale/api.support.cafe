use parking_lot::Mutex;

use endpoint_libs::libs::log::{LogLevel, LogReloadHandle};
use endpoint_libs::libs::toolbox::CustomError;

use crate::codegen::model::{EnumErrorCode, LogLevel as ModelLogLevel};

pub struct LogService {
    reload_handle: LogReloadHandle,
    current_level: Mutex<LogLevel>,
}

impl LogService {
    pub fn new(reload_handle: LogReloadHandle, initial_level: LogLevel) -> Self {
        Self {
            reload_handle,
            current_level: Mutex::new(initial_level),
        }
    }

    pub fn set_level(&self, level: ModelLogLevel) -> Result<(), CustomError> {
        let libs_level: LogLevel = level.into();
        self.reload_handle
            .set_log_level(libs_level)
            .map_err(|e| CustomError::new(EnumErrorCode::Xxx, format!("Failed to set log level: {e}")))?;
        *self.current_level.lock() = libs_level;
        Ok(())
    }

    pub fn get_level(&self) -> ModelLogLevel {
        (*self.current_level.lock()).into()
    }
}

impl From<ModelLogLevel> for LogLevel {
    fn from(level: ModelLogLevel) -> Self {
        match level {
            ModelLogLevel::Trace => LogLevel::Trace,
            ModelLogLevel::Debug => LogLevel::Debug,
            ModelLogLevel::Info => LogLevel::Info,
            ModelLogLevel::Warn => LogLevel::Warn,
            ModelLogLevel::Error => LogLevel::Error,
        }
    }
}

impl From<LogLevel> for ModelLogLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => ModelLogLevel::Trace,
            LogLevel::Debug => ModelLogLevel::Debug,
            LogLevel::Info => ModelLogLevel::Info,
            LogLevel::Warn => ModelLogLevel::Warn,
            LogLevel::Error => ModelLogLevel::Error,
            LogLevel::Off | LogLevel::Detail => ModelLogLevel::Trace,
        }
    }
}