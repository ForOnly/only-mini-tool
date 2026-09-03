//! 百度 handwriting OCR。

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
            let msg = body
                .get("error_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(AppError::OcrEngine {
                message: format!("baidu error {code}: {msg}"),
            });
        }

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
}

impl BaiduEngine {
    async fn get_access_token(&self, api_key: &str, secret: &str) -> Result<String, AppError> {
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

        body.get("access_token")
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
            })
    }
}
