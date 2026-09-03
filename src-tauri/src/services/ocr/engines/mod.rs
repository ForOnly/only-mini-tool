//! 内置 OCR 引擎实现。

mod baidu;
mod paddle;

use super::config::OcrEngine;

pub fn register_all() -> Vec<Box<dyn OcrEngine>> {
    vec![
        Box::new(baidu::BaiduEngine),
        Box::new(paddle::PaddleEngine),
    ]
}
