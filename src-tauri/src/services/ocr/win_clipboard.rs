//! Windows：arboard 仅可靠读 PNG / CF_DIBV5；补充 CF_DIB / CF_BITMAP。

use std::io::Cursor;

use clipboard_win::Getter;
use image::codecs::bmp::BmpDecoder;
use image::{DynamicImage, RgbaImage};

use crate::errors::AppError;

/// 读取 CF_DIB 或 CF_BITMAP，解码为 RGBA。
pub fn read_cf_dib_or_bitmap() -> Result<RgbaImage, AppError> {
    let _clip =
        clipboard_win::Clipboard::new_attempts(10).map_err(|error| AppError::InternalError {
            message: format!("clipboard unavailable: {error}"),
        })?;

    // CF_DIB：BITMAPINFOHEADER + 像素（无 BITMAPFILEHEADER）
    if clipboard_win::is_format_avail(clipboard_win::formats::CF_DIB) {
        let mut data = Vec::new();
        if clipboard_win::raw::get_vec(clipboard_win::formats::CF_DIB, &mut data).is_ok() {
            if let Ok(rgba) = decode_dib_pixels(&data) {
                return Ok(rgba);
            }
        }
    }

    // CF_BITMAP → 完整 BMP 字节（含文件头）
    let mut bmp = Vec::new();
    if clipboard_win::formats::Bitmap
        .read_clipboard(&mut bmp)
        .is_ok()
        && !bmp.is_empty()
    {
        let rgba = image::load_from_memory(&bmp)
            .map_err(|error| AppError::OcrBadImage {
                message: format!("failed to decode CF_BITMAP: {error}"),
            })?
            .into_rgba8();
        return Ok(rgba);
    }

    Err(AppError::OcrClipboardEmpty {
        message: "no DIB/BITMAP in clipboard".into(),
    })
}

fn decode_dib_pixels(dib: &[u8]) -> Result<RgbaImage, AppError> {
    let decoder = BmpDecoder::new_without_file_header(Cursor::new(dib)).map_err(|error| {
        AppError::OcrBadImage {
            message: format!("failed to decode CF_DIB: {error}"),
        }
    })?;
    DynamicImage::from_decoder(decoder)
        .map_err(|error| AppError::OcrBadImage {
            message: format!("failed to decode CF_DIB pixels: {error}"),
        })
        .map(|img| img.into_rgba8())
}
