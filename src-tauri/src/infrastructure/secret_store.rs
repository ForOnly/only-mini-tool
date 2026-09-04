//! OS 凭据库（Windows Credential Manager 等）存 OCR 密码字段。

use keyring::Entry;

use crate::errors::AppError;

const SERVICE: &str = "only-mini-tool";

fn entry(key: &str) -> Result<Entry, AppError> {
    Entry::new(SERVICE, key).map_err(|error| AppError::InternalError {
        message: format!("keyring entry failed for {key}: {error}"),
    })
}

/// 读取密钥；无条目时返回 `None`。
pub fn get(key: &str) -> Result<Option<String>, AppError> {
    let entry = entry(key)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(AppError::InternalError {
            message: format!("keyring get failed for {key}: {error}"),
        }),
    }
}

/// 写入密钥；空字符串则删除条目。
pub fn set(key: &str, value: &str) -> Result<(), AppError> {
    if value.is_empty() {
        return delete(key);
    }
    entry(key)?
        .set_password(value)
        .map_err(|error| AppError::InternalError {
            message: format!("keyring set failed for {key}: {error}"),
        })
}

pub fn delete(key: &str) -> Result<(), AppError> {
    let entry = entry(key)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(AppError::InternalError {
            message: format!("keyring delete failed for {key}: {error}"),
        }),
    }
}
