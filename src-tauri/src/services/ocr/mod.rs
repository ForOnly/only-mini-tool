//! OCR 引擎抽象、内置引擎与临时文件服务。

mod config;
mod engines;
mod registry;
mod secrets;
mod service;
mod temp;
#[cfg(windows)]
mod win_clipboard;

pub use registry::OcrRegistry;
pub use secrets::wipe_engine_secrets_on_schema_reset;
pub use service::OcrService;
