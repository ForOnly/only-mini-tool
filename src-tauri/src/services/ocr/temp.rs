use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat};
use tauri::AppHandle;

use crate::errors::AppError;
use crate::infrastructure::filesystem::{ensure_dirs, ocr_temp_dir};

const ALLOWED_EXT: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif", "webp"];
/// 粘贴/字节入库上限（与前端 pasteFromBlob 一致）。
const MAX_STAGE_BYTES: usize = 8 * 1024 * 1024;

pub fn ensure_ocr_temp(app: &AppHandle) -> Result<PathBuf, AppError> {
    ensure_dirs(app)?;
    ocr_temp_dir(app)
}

fn unique_path(dir: &Path, ext: &str) -> PathBuf {
    let name = format!("{}.{}", uuid::Uuid::new_v4(), ext);
    dir.join(name)
}

fn normalize_ext(path: &Path) -> Result<String, AppError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if ALLOWED_EXT.contains(&ext.as_str()) {
        Ok(ext)
    } else {
        Err(AppError::OcrBadImage {
            message: format!("unsupported image type: {ext}"),
        })
    }
}

/// 将用户文件拷入 OCR 临时目录，返回新 path。
pub fn stage_image_file(app: &AppHandle, source: &str) -> Result<String, AppError> {
    let src = PathBuf::from(source);
    if !src.is_file() {
        return Err(AppError::OcrBadImage {
            message: format!("image file not found: {source}"),
        });
    }
    let ext = normalize_ext(&src)?;
    let dir = ensure_ocr_temp(app)?;
    let dest = unique_path(&dir, &ext);
    std::fs::copy(&src, &dest).map_err(|error| AppError::InternalError {
        message: format!("failed to stage image: {error}"),
    })?;
    Ok(dest.to_string_lossy().into_owned())
}

/// 将内存中的图片字节写入 OCR 临时目录。
pub fn stage_image_bytes(app: &AppHandle, bytes: &[u8], ext: &str) -> Result<String, AppError> {
    if bytes.is_empty() {
        return Err(AppError::OcrBadImage {
            message: "image bytes are empty".into(),
        });
    }
    if bytes.len() > MAX_STAGE_BYTES {
        return Err(AppError::OcrImageTooLarge {
            message: format!(
                "image exceeds {} bytes limit ({})",
                MAX_STAGE_BYTES,
                bytes.len()
            ),
        });
    }
    let normalized = ext.trim().to_ascii_lowercase();
    let ext = if ALLOWED_EXT.contains(&normalized.as_str()) {
        normalized
    } else {
        "png".into()
    };
    let dir = ensure_ocr_temp(app)?;
    let dest = unique_path(&dir, &ext);
    std::fs::write(&dest, bytes).map_err(|error| AppError::InternalError {
        message: format!("failed to write staged image bytes: {error}"),
    })?;
    Ok(dest.to_string_lossy().into_owned())
}

/// 将 RGBA 写入 OCR 临时 PNG。
pub(super) fn write_rgba_temp_png(
    app: &AppHandle,
    rgba: image::RgbaImage,
) -> Result<String, AppError> {
    let dir = ensure_ocr_temp(app)?;
    let dest = unique_path(&dir, "png");
    DynamicImage::ImageRgba8(rgba)
        .save_with_format(&dest, ImageFormat::Png)
        .map_err(|error| AppError::InternalError {
            message: format!("failed to write clipboard image: {error}"),
        })?;
    Ok(dest.to_string_lossy().into_owned())
}

pub fn save_clipboard_image(app: &AppHandle) -> Result<String, AppError> {
    // 1) arboard：PNG / CF_DIBV5
    {
        let mut clipboard = arboard::Clipboard::new().map_err(|error| AppError::InternalError {
            message: format!("clipboard unavailable: {error}"),
        })?;
        if let Ok(data) = clipboard.get_image() {
            let width = data.width as u32;
            let height = data.height as u32;
            let rgba = image::RgbaImage::from_raw(width, height, data.bytes.into_owned())
                .ok_or_else(|| AppError::OcrBadImage {
                    message: "clipboard image dimensions mismatch".into(),
                })?;
            return write_rgba_temp_png(app, rgba);
        }
    }

    // 2) Windows：CF_DIB / CF_BITMAP（常见截图源）
    #[cfg(windows)]
    if let Ok(rgba) = super::win_clipboard::read_cf_dib_or_bitmap() {
        return write_rgba_temp_png(app, rgba);
    }

    // 3) Explorer 等复制的图片文件路径（CF_HDROP）
    {
        let mut clipboard = arboard::Clipboard::new().map_err(|error| AppError::InternalError {
            message: format!("clipboard unavailable: {error}"),
        })?;
        if let Ok(paths) = clipboard.get().file_list() {
            for path in paths {
                if path.is_file() && normalize_ext(&path).is_ok() {
                    return stage_image_file(app, &path.to_string_lossy());
                }
            }
        }
    }

    Err(AppError::OcrClipboardEmpty {
        message: "no image in clipboard".into(),
    })
}

/// 顺时针旋转 degrees（v1 仅 90 的倍数），写回新临时 PNG。
pub fn rotate_image_orientation(
    app: &AppHandle,
    path: &str,
    degrees: i32,
) -> Result<String, AppError> {
    let turns = match degrees.rem_euclid(360) {
        0 => 0,
        90 => 1,
        180 => 2,
        270 => 3,
        other => {
            return Err(AppError::ValidationError {
                message: format!("unsupported rotate degrees: {other}"),
            });
        }
    };

    let src = PathBuf::from(path);
    if !src.is_file() {
        return Err(AppError::OcrBadImage {
            message: format!("image file not found: {path}"),
        });
    }

    let mut rotated = image::open(&src).map_err(|error| AppError::OcrBadImage {
        message: format!("failed to decode image: {error}"),
    })?;
    for _ in 0..turns {
        rotated = rotated.rotate90();
    }

    let dir = ensure_ocr_temp(app)?;
    let dest = unique_path(&dir, "png");
    rotated
        .save_with_format(&dest, ImageFormat::Png)
        .map_err(|error| AppError::InternalError {
            message: format!("failed to write rotated image: {error}"),
        })?;

    Ok(dest.to_string_lossy().into_owned())
}

pub fn clear_ocr_temp(app: &AppHandle) -> Result<(), AppError> {
    let dir = ensure_ocr_temp(app)?;
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&dir).map_err(|error| AppError::InternalError {
        message: format!("failed to read ocr temp: {error}"),
    })? {
        let entry = entry.map_err(|error| AppError::InternalError {
            message: format!("failed to list ocr temp: {error}"),
        })?;
        let path = entry.path();
        if path.is_file() {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

pub fn read_image_bytes(path: &str) -> Result<Vec<u8>, AppError> {
    let src = PathBuf::from(path);
    if !src.is_file() {
        return Err(AppError::OcrBadImage {
            message: format!("image file not found: {path}"),
        });
    }
    std::fs::read(&src).map_err(|error| AppError::OcrBadImage {
        message: format!("failed to read image: {error}"),
    })
}
