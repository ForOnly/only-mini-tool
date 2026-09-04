//! PaddleOCR 官方在线 API（AI Studio Access Token + 异步 jobs）。

use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde_json::Value;
use tokio::time::{sleep, Duration};

use crate::domain::{OcrRect, OcrResult, OcrWord};
use crate::errors::AppError;

use super::super::config::{EngineConfig, EngineFieldSpec, EngineSpec, OcrEngine};

pub struct PaddleEngine;

const SPEC: EngineSpec = EngineSpec {
    id: "paddle",
    fields: &[
        EngineFieldSpec::password("access_token", true),
        EngineFieldSpec::text("model", false),
    ],
};

const DEFAULT_BASE_URL: &str = "https://paddleocr.aistudio-app.com";
const DEFAULT_MODEL: &str = "PP-OCRv5";
const JOBS_PATH: &str = "/api/v2/ocr/jobs";
const POLL_INTERVAL: Duration = Duration::from_secs(1);

const OPTIONAL_PAYLOAD: &str =
    r#"{"useDocOrientationClassify":false,"useDocUnwarping":false,"useTextlineOrientation":false}"#;

#[async_trait]
impl OcrEngine for PaddleEngine {
    fn spec(&self) -> &'static EngineSpec {
        &SPEC
    }

    async fn recognize(&self, image: &[u8], cfg: &EngineConfig) -> Result<OcrResult, AppError> {
        let token = cfg.require("access_token")?;
        let model = cfg
            .get("model")
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(DEFAULT_MODEL);

        let auth_client = reqwest::Client::new();
        let job_id = submit_job(&auth_client, token, model, image).await?;
        let json_url = poll_until_done(&auth_client, token, &job_id).await?;
        // BOS 预签名 URL 不可带 Authorization
        let plain_client = reqwest::Client::new();
        let jsonl = download_jsonl(&plain_client, &json_url).await?;
        let words = parse_jsonl_ocr(&jsonl)?;
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

fn jobs_url() -> String {
    format!("{DEFAULT_BASE_URL}{JOBS_PATH}")
}

fn auth_header(token: &str) -> String {
    format!("bearer {token}")
}

async fn submit_job(
    client: &reqwest::Client,
    token: &str,
    model: &str,
    image: &[u8],
) -> Result<String, AppError> {
    let part = Part::bytes(image.to_vec())
        .file_name("image.png")
        .mime_str("application/octet-stream")
        .map_err(|error| AppError::InternalError {
            message: format!("multipart part: {error}"),
        })?;

    let form = Form::new()
        .text("model", model.to_string())
        .text("optionalPayload", OPTIONAL_PAYLOAD.to_string())
        .part("file", part);

    let resp = client
        .post(jobs_url())
        .header("Authorization", auth_header(token))
        .multipart(form)
        .send()
        .await
        .map_err(|error| AppError::OcrNetwork {
            message: format!("paddle job submit failed: {error}"),
        })?;

    let status = resp.status();
    let body = resp
        .json::<Value>()
        .await
        .map_err(|error| AppError::OcrNetwork {
            message: format!("paddle job submit response invalid: {error}"),
        })?;

    if !status.is_success() {
        return Err(AppError::OcrEngine {
            message: format!("paddle job submit HTTP {status}: {body}"),
        });
    }

    body.pointer("/data/jobId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::OcrEngine {
            message: format!("paddle job submit missing jobId: {body}"),
        })
}

async fn poll_until_done(
    client: &reqwest::Client,
    token: &str,
    job_id: &str,
) -> Result<String, AppError> {
    let url = format!("{}/{job_id}", jobs_url());
    loop {
        let resp = client
            .get(&url)
            .header("Authorization", auth_header(token))
            .send()
            .await
            .map_err(|error| AppError::OcrNetwork {
                message: format!("paddle job poll failed: {error}"),
            })?;

        let status = resp.status();
        let body = resp
            .json::<Value>()
            .await
            .map_err(|error| AppError::OcrNetwork {
                message: format!("paddle job poll response invalid: {error}"),
            })?;

        if !status.is_success() {
            return Err(AppError::OcrEngine {
                message: format!("paddle job poll HTTP {status}: {body}"),
            });
        }

        let state = body
            .pointer("/data/state")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match state {
            "done" => {
                return body
                    .pointer("/data/resultUrl/jsonUrl")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| AppError::OcrEngine {
                        message: format!("paddle job done but missing jsonUrl: {body}"),
                    });
            }
            "failed" => {
                let msg = body
                    .pointer("/data/errorMsg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("job failed");
                return Err(AppError::OcrEngine {
                    message: format!("paddle job failed: {msg}"),
                });
            }
            "pending" | "running" | "" => {
                sleep(POLL_INTERVAL).await;
            }
            other => {
                return Err(AppError::OcrEngine {
                    message: format!("paddle unknown job state: {other}"),
                });
            }
        }
    }
}

async fn download_jsonl(client: &reqwest::Client, json_url: &str) -> Result<String, AppError> {
    let resp = client
        .get(json_url)
        .send()
        .await
        .map_err(|error| AppError::OcrNetwork {
            message: format!("paddle jsonUrl download failed: {error}"),
        })?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::OcrEngine {
            message: format!("paddle jsonUrl HTTP {status}: {text}"),
        });
    }

    resp.text().await.map_err(|error| AppError::OcrNetwork {
        message: format!("paddle jsonUrl body invalid: {error}"),
    })
}

fn parse_jsonl_ocr(jsonl: &str) -> Result<Vec<OcrWord>, AppError> {
    let mut words = Vec::new();
    for (line_no, line) in jsonl.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let root: Value = serde_json::from_str(line).map_err(|error| AppError::OcrEngine {
            message: format!("paddle jsonl line {} invalid: {error}", line_no + 1),
        })?;

        let pruned_list = collect_pruned(&root);
        if pruned_list.is_empty() {
            return Err(AppError::OcrEngine {
                message: format!(
                    "paddle jsonl line {} missing ocrResults/prunedResult",
                    line_no + 1
                ),
            });
        }
        for pruned in pruned_list {
            words.extend(pruned_to_words(pruned));
        }
    }
    Ok(words)
}

fn collect_pruned(root: &Value) -> Vec<&Value> {
    let mut out = Vec::new();
    if let Some(arr) = root
        .pointer("/result/ocrResults")
        .or_else(|| root.pointer("/ocrResults"))
        .and_then(|v| v.as_array())
    {
        for item in arr {
            if let Some(pruned) = item.get("prunedResult") {
                out.push(pruned);
            }
        }
    } else if root.get("prunedResult").is_some() {
        if let Some(pruned) = root.get("prunedResult") {
            out.push(pruned);
        }
    } else if root.get("rec_texts").is_some() {
        out.push(root);
    }
    out
}

fn pruned_to_words(pruned: &Value) -> Vec<OcrWord> {
    let texts = pruned
        .get("rec_texts")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let scores = pruned
        .get("rec_scores")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let boxes = pruned
        .get("rec_boxes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let polys = pruned
        .get("rec_polys")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut words = Vec::new();
    for (i, text_val) in texts.iter().enumerate() {
        let text = text_val.as_str().unwrap_or("").to_string();
        if text.is_empty() {
            continue;
        }
        let confidence = scores.get(i).and_then(|v| v.as_f64());
        let rect = boxes
            .get(i)
            .map(rec_box_to_rect)
            .or_else(|| polys.get(i).map(poly_to_rect))
            .unwrap_or(OcrRect {
                left: 0,
                top: 0,
                width: 0,
                height: 0,
            });
        words.push(OcrWord {
            text,
            rect,
            confidence,
            line: Some(i as i32),
        });
    }
    words
}

/// `rec_boxes` 行：`[xmin, ymin, xmax, ymax]`
fn rec_box_to_rect(box_val: &Value) -> OcrRect {
    let arr = box_val.as_array();
    if let Some(a) = arr {
        if a.len() >= 4 {
            let xmin = a[0].as_f64().unwrap_or(0.0);
            let ymin = a[1].as_f64().unwrap_or(0.0);
            let xmax = a[2].as_f64().unwrap_or(0.0);
            let ymax = a[3].as_f64().unwrap_or(0.0);
            return OcrRect {
                left: xmin.round() as i32,
                top: ymin.round() as i32,
                width: (xmax - xmin).round().max(0.0) as i32,
                height: (ymax - ymin).round().max(0.0) as i32,
            };
        }
    }
    OcrRect {
        left: 0,
        top: 0,
        width: 0,
        height: 0,
    }
}

fn poly_to_rect(poly: &Value) -> OcrRect {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    if let Some(arr) = poly.as_array() {
        for pt in arr {
            if let Some(pair) = pt.as_array() {
                if pair.len() >= 2 {
                    xs.push(pair[0].as_f64().unwrap_or(0.0));
                    ys.push(pair[1].as_f64().unwrap_or(0.0));
                }
            }
        }
    }
    if xs.is_empty() || ys.is_empty() {
        return OcrRect {
            left: 0,
            top: 0,
            width: 0,
            height: 0,
        };
    }
    let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    OcrRect {
        left: min_x.round() as i32,
        top: min_y.round() as i32,
        width: (max_x - min_x).round().max(0.0) as i32,
        height: (max_y - min_y).round().max(0.0) as i32,
    }
}
