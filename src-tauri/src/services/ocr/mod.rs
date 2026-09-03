//! OCR 引擎抽象、内置引擎与临时文件服务。

mod config;
mod engines;
mod registry;
mod service;
mod temp;
#[cfg(windows)]
mod win_clipboard;

pub use registry::OcrRegistry;
pub use service::OcrService;
