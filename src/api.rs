/// Blocking HTTP client for the BytePlus ModelArk Video Generation API.
///
/// Endpoints:
///   POST /contents/generations/tasks        -- create
///   GET  /contents/generations/tasks/{id}   -- retrieve
///   DELETE /contents/generations/tasks/{id} -- cancel (queued only)
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::AppError;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

pub struct ApiClient {
    base_url: String,
    api_key: String,
    http: reqwest::blocking::Client,
}

impl ApiClient {
    pub fn new(base_url: &str, api_key: &str) -> Result<Self, AppError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent(concat!("seedance/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(AppError::from)?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            http,
        })
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    pub fn create_task(&self, req: &CreateTaskRequest) -> Result<CreateTaskResponse, AppError> {
        let url = format!("{}/contents/generations/tasks", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(req)
            .send()?;
        parse_json(resp)
    }

    pub fn get_task(&self, id: &str) -> Result<TaskInfo, AppError> {
        let url = format!("{}/contents/generations/tasks/{}", self.base_url, id);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()?;
        parse_json(resp)
    }

    pub fn cancel_task(&self, id: &str) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/contents/generations/tasks/{}", self.base_url, id);
        let resp = self
            .http
            .delete(&url)
            .header("Authorization", self.auth_header())
            .send()?;
        // Some DELETE endpoints return empty body on success; handle both.
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(parse_error_body(status, &body));
        }
        if body.trim().is_empty() {
            return Ok(serde_json::json!({ "id": id, "status": "cancelled" }));
        }
        serde_json::from_str(&body).map_err(|e| AppError::Transient(e.to_string()))
    }

    /// Download the generated video to a local file. Returns bytes written.
    pub fn download_video(
        &self,
        video_url: &str,
        out: &std::path::Path,
    ) -> Result<u64, AppError> {
        let mut resp = self.http.get(video_url).send()?;
        if !resp.status().is_success() {
            return Err(AppError::Transient(format!(
                "download failed: HTTP {}",
                resp.status()
            )));
        }
        if let Some(parent) = out.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(out)?;
        let bytes = std::io::copy(&mut resp, &mut file)?;
        Ok(bytes)
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(resp: reqwest::blocking::Response) -> Result<T, AppError> {
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(parse_error_body(status, &body));
    }
    serde_json::from_str::<T>(&body).map_err(|e| {
        AppError::Transient(format!("failed to parse API response ({e}): {body}"))
    })
}

fn parse_error_body(status: reqwest::StatusCode, body: &str) -> AppError {
    // BytePlus ModelArk errors look like:
    //   { "error": { "code": "XXX", "message": "..." } }
    // Fall back to raw text if the body isn't JSON.
    #[derive(Deserialize)]
    struct Wrapper {
        error: Option<ApiErrorPayload>,
    }
    #[derive(Deserialize)]
    struct ApiErrorPayload {
        code: Option<String>,
        message: Option<String>,
    }

    if status.as_u16() == 429 {
        return AppError::RateLimited(body.to_string());
    }

    if let Ok(w) = serde_json::from_str::<Wrapper>(body)
        && let Some(err) = w.error
    {
        return AppError::Api {
            code: err.code.unwrap_or_else(|| status.as_u16().to_string()),
            message: err.message.unwrap_or_else(|| body.to_string()),
        };
    }
    AppError::Api {
        code: status.as_u16().to_string(),
        message: if body.is_empty() {
            status.canonical_reason().unwrap_or("unknown").to_string()
        } else {
            body.to_string()
        },
    }
}

// ── Request / Response shapes ──────────────────────────────────────────────

#[derive(Serialize, Debug)]
pub struct CreateTaskRequest {
    pub model: String,
    pub content: Vec<ContentItem>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_audio: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
    Text {
        text: String,
    },
    ImageUrl {
        image_url: UrlObject,
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<String>,
    },
    VideoUrl {
        video_url: UrlObject,
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<String>,
    },
    AudioUrl {
        audio_url: UrlObject,
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<String>,
    },
}

#[derive(Serialize, Debug)]
pub struct UrlObject {
    pub url: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct CreateTaskResponse {
    pub id: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct TaskInfo {
    pub id: String,
    #[serde(default)]
    pub model: Option<String>,
    pub status: String,
    #[serde(default)]
    pub error: Option<ApiErrorInner>,
    #[serde(default)]
    pub content: Option<TaskContent>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub ratio: Option<String>,
    #[serde(default)]
    pub duration: Option<i32>,
    #[serde(default)]
    pub framespersecond: Option<i32>,
    #[serde(default)]
    pub generate_audio: Option<bool>,
    #[serde(default)]
    pub service_tier: Option<String>,
    #[serde(default)]
    pub execution_expires_after: Option<i64>,
    #[serde(default)]
    pub safety_identifier: Option<String>,
}

impl TaskInfo {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status.as_str(),
            "succeeded" | "failed" | "cancelled" | "expired"
        )
    }

    pub fn video_url(&self) -> Option<&str> {
        self.content.as_ref().and_then(|c| c.video_url.as_deref())
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct ApiErrorInner {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct TaskContent {
    #[serde(default)]
    pub video_url: Option<String>,
    #[serde(default)]
    pub last_frame_url: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Usage {
    #[serde(default)]
    pub completion_tokens: Option<i64>,
    #[serde(default)]
    pub total_tokens: Option<i64>,
}
