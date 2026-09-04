//! 百度 handwriting OCR。

use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::Engine as _;
use serde_json::Value;

use crate::domain::{OcrRect, OcrResult, OcrWord};
use crate::errors::AppError;

use super::super::config::{EngineConfig, EngineFieldSpec, EngineSpec, OcrEngine};

pub struct BaiduEngine;

const SPEC: EngineSpec = EngineSpec {
    id: "baidu",
    fields: &[
        EngineFieldSpec::password("api_key", true),
        EngineFieldSpec::password("secret_key", true),
    ],
};

/// 进程内 access_token 缓存（不落库）。
struct CachedToken {
    api_key: String,
    secret_key: String,
    token: String,
    expires_at: Instant,
}

fn token_cache() -> &'static Mutex<Option<CachedToken>> {
    static CACHE: Mutex<Option<CachedToken>> = Mutex::new(None);
    &CACHE
}

/// 密钥变更或显式失效时清空缓存。
pub fn invalidate_token_cache() {
    if let Ok(mut guard) = token_cache().lock() {
        *guard = None;
    }
}

#[async_trait]
impl OcrEngine for BaiduEngine {
    fn spec(&self) -> &'static EngineSpec {
        &SPEC
    }

    async fn recognize(&self, image: &[u8], cfg: &EngineConfig) -> Result<OcrResult, AppError> {
        let api_key = cfg.require("api_key")?;
        let secret_key = cfg.require("secret_key")?;
        let token = self.get_access_token(api_key, secret_key).await?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(image);
        let client = reqwest::Client::new();
        let url = format!(
            "https://aip.baidubce.com/rest/2.0/ocr/v1/handwriting?access_token={token}"
        );
        let resp = client
            .post(url)
            .header("content-type", "application/x-www-form-urlencoded")
            .form(&[("image", b64)])
            .send()
            .await
            .map_err(|error| AppError::OcrNetwork {
                message: format!("baidu handwriting request failed: {error}"),
            })?;

        let status = resp.status();
        let body = resp.json::<Value>().await.map_err(|error| AppError::OcrNetwork {
            message: format!("baidu handwriting response invalid: {error}"),
        })?;

        if !status.is_success() {
            return Err(AppError::OcrEngine {
                message: format!("baidu HTTP {status}: {body}"),
            });
        }

        if let Some(code) = body.get("error_code") {
            let code_num = parse_baidu_error_code(code);
            let msg = body
                .get("error_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            // 非法 token：清缓存后重试一次
            if matches!(code_num, 110 | 111) {
                invalidate_token_cache();
                let token = self.fetch_access_token(api_key, secret_key).await?;
                return self.recognize_with_token(image, &token).await;
            }
            return Err(AppError::OcrEngine {
                message: format!("baidu error {code}: {msg}"),
            });
        }

        Self::parse_words_result(body)
    }
}

/// 兼容百度 error_code 为数字或数字字符串。
fn parse_baidu_error_code(code: &Value) -> i64 {
    code.as_i64()
        .or_else(|| code.as_u64().map(|v| v as i64))
        .or_else(|| code.as_str().and_then(|s| s.trim().parse().ok()))
        .unwrap_or(-1)
}

impl BaiduEngine {
    async fn recognize_with_token(&self, image: &[u8], token: &str) -> Result<OcrResult, AppError> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(image);
        let client = reqwest::Client::new();
        let url = format!(
            "https://aip.baidubce.com/rest/2.0/ocr/v1/handwriting?access_token={token}"
        );
        let resp = client
            .post(url)
            .header("content-type", "application/x-www-form-urlencoded")
            .form(&[("image", b64)])
            .send()
            .await
            .map_err(|error| AppError::OcrNetwork {
                message: format!("baidu handwriting request failed: {error}"),
            })?;

        let status = resp.status();
        let body = resp.json::<Value>().await.map_err(|error| AppError::OcrNetwork {
            message: format!("baidu handwriting response invalid: {error}"),
        })?;

        if !status.is_success() {
            return Err(AppError::OcrEngine {
                message: format!("baidu HTTP {status}: {body}"),
            });
        }

        if let Some(code) = body.get("error_code") {
            let msg = body
                .get("error_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(AppError::OcrEngine {
                message: format!("baidu error {code}: {msg}"),
            });
        }

        Self::parse_words_result(body)
    }

    fn parse_words_result(body: Value) -> Result<OcrResult, AppError> {
        let mut words = Vec::new();
        if let Some(arr) = body.get("words_result").and_then(|v| v.as_array()) {
            for (i, item) in arr.iter().enumerate() {
                let loc = &item["location"];
                words.push(OcrWord {
                    text: item["words"].as_str().unwrap_or("").to_string(),
                    rect: OcrRect {
                        left: loc["left"].as_i64().unwrap_or(0) as i32,
                        top: loc["top"].as_i64().unwrap_or(0) as i32,
                        width: loc["width"].as_i64().unwrap_or(0) as i32,
                        height: loc["height"].as_i64().unwrap_or(0) as i32,
                    },
                    confidence: Some(1.0),
                    line: Some(i as i32),
                });
            }
        }

        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(OcrResult {
            engine: SPEC.id.into(),
            words,
            text,
        })
    }

    async fn get_access_token(&self, api_key: &str, secret: &str) -> Result<String, AppError> {
        {
            let guard = token_cache().lock().map_err(|_| AppError::InternalError {
                message: "baidu token cache poisoned".into(),
            })?;
            if let Some(cached) = guard.as_ref() {
                if cached.api_key == api_key
                    && cached.secret_key == secret
                    && Instant::now() < cached.expires_at
                {
                    return Ok(cached.token.clone());
                }
            }
        }
        self.fetch_access_token(api_key, secret).await
    }

    async fn fetch_access_token(&self, api_key: &str, secret: &str) -> Result<String, AppError> {
        let client = reqwest::Client::new();
        let resp = client
            .post("https://aip.baidubce.com/oauth/2.0/token")
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", api_key),
                ("client_secret", secret),
            ])
            .send()
            .await
            .map_err(|error| AppError::OcrNetwork {
                message: format!("baidu oauth request failed: {error}"),
            })?;

        let body = resp.json::<Value>().await.map_err(|error| AppError::OcrNetwork {
            message: format!("baidu oauth response invalid: {error}"),
        })?;

        let token = body
            .get("access_token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                let msg = body
                    .get("error_description")
                    .or_else(|| body.get("error"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("failed to obtain access_token");
                AppError::OcrEngine {
                    message: msg.to_string(),
                }
            })?;

        // 提前 120s 过期，避免边界失败；缺省按 30 天
        let expires_in = body
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(2_592_000);
        let skew = 120u64;
        let ttl = expires_in.saturating_sub(skew).max(60);
        let expires_at = Instant::now() + Duration::from_secs(ttl);

        let mut guard = token_cache().lock().map_err(|_| AppError::InternalError {
            message: "baidu token cache poisoned".into(),
        })?;
        *guard = Some(CachedToken {
            api_key: api_key.to_string(),
            secret_key: secret.to_string(),
            token: token.clone(),
            expires_at,
        });

        Ok(token)
    }
}
