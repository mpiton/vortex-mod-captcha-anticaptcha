use std::collections::HashMap;

use serde::{Deserialize, Serialize};

const CREATE_TASK_URL: &str = "https://api.anti-captcha.com/createTask";
const GET_TASK_RESULT_URL: &str = "https://api.anti-captcha.com/getTaskResult";
const GET_BALANCE_URL: &str = "https://api.anti-captcha.com/getBalance";
const MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;
const MAX_SOLUTION_BYTES: usize = 4_096;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("could not encode AntiCaptcha request")]
    Encode,
    #[error("AntiCaptcha host response is invalid")]
    HostResponse,
    #[error("AntiCaptcha HTTP request failed")]
    Http,
    #[error("AntiCaptcha API rejected the request")]
    Rejected,
    #[error("AntiCaptcha API response is invalid")]
    InvalidResponse,
}

#[derive(Serialize)]
struct HttpRequest {
    method: &'static str,
    url: &'static str,
    headers: HashMap<&'static str, &'static str>,
    body: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpResponse {
    status: u16,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskRequest<'a> {
    client_key: &'a str,
    task: ImageToTextTask<'a>,
}

#[derive(Serialize)]
struct ImageToTextTask<'a> {
    #[serde(rename = "type")]
    task_type: &'static str,
    body: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskRequest<'a> {
    client_key: &'a str,
    task_id: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialRequest<'a> {
    client_key: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskResponse {
    error_id: u32,
    task_id: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskResultResponse {
    error_id: u32,
    status: Option<String>,
    solution: Option<TaskSolution>,
}

#[derive(Deserialize)]
struct TaskSolution {
    text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BalanceResponse {
    error_id: u32,
    balance: Option<f64>,
}

#[derive(Debug, PartialEq)]
pub enum TaskResult {
    Processing,
    Ready(String),
}

pub fn build_create_task_request(api_key: &str, image_data: &str) -> Result<String, ApiError> {
    let body = serde_json::to_string(&CreateTaskRequest {
        client_key: api_key,
        task: ImageToTextTask {
            task_type: "ImageToTextTask",
            body: image_data,
        },
    })
    .map_err(|_| ApiError::Encode)?;
    encode_http(CREATE_TASK_URL, body)
}

pub fn build_get_task_result_request(api_key: &str, task_id: u64) -> Result<String, ApiError> {
    let body = serde_json::to_string(&TaskRequest {
        client_key: api_key,
        task_id,
    })
    .map_err(|_| ApiError::Encode)?;
    encode_http(GET_TASK_RESULT_URL, body)
}

pub fn build_get_balance_request(api_key: &str) -> Result<String, ApiError> {
    let body = serde_json::to_string(&CredentialRequest {
        client_key: api_key,
    })
    .map_err(|_| ApiError::Encode)?;
    encode_http(GET_BALANCE_URL, body)
}

pub fn parse_create_task_response(raw: &str) -> Result<u64, ApiError> {
    let body = success_body(raw)?;
    let response: CreateTaskResponse =
        serde_json::from_str(&body).map_err(|_| ApiError::InvalidResponse)?;
    if response.error_id != 0 {
        return Err(ApiError::Rejected);
    }
    response
        .task_id
        .filter(|id| *id > 0)
        .ok_or(ApiError::InvalidResponse)
}

pub fn parse_task_result_response(raw: &str) -> Result<TaskResult, ApiError> {
    let body = success_body(raw)?;
    let response: TaskResultResponse =
        serde_json::from_str(&body).map_err(|_| ApiError::InvalidResponse)?;
    if response.error_id != 0 {
        return Err(ApiError::Rejected);
    }
    match (response.status.as_deref(), response.solution) {
        (Some("processing"), None) => Ok(TaskResult::Processing),
        (Some("ready"), Some(solution)) => {
            let text = solution.text.trim().to_string();
            if text.is_empty() || text.len() > MAX_SOLUTION_BYTES {
                return Err(ApiError::InvalidResponse);
            }
            Ok(TaskResult::Ready(text))
        }
        _ => Err(ApiError::InvalidResponse),
    }
}

pub fn parse_balance_response(raw: &str) -> Result<f64, ApiError> {
    let body = success_body(raw)?;
    let response: BalanceResponse =
        serde_json::from_str(&body).map_err(|_| ApiError::InvalidResponse)?;
    if response.error_id != 0 {
        return Err(ApiError::Rejected);
    }
    response
        .balance
        .filter(|balance| balance.is_finite() && *balance >= 0.0)
        .ok_or(ApiError::InvalidResponse)
}

fn encode_http(url: &'static str, body: String) -> Result<String, ApiError> {
    let mut headers = HashMap::new();
    headers.insert("Content-Type", "application/json");
    serde_json::to_string(&HttpRequest {
        method: "POST",
        url,
        headers,
        body,
    })
    .map_err(|_| ApiError::Encode)
}

fn success_body(raw: &str) -> Result<String, ApiError> {
    let response: HttpResponse = serde_json::from_str(raw).map_err(|_| ApiError::HostResponse)?;
    let _ = response.headers;
    if !(200..300).contains(&response.status) || response.body.len() > MAX_RESPONSE_BODY_BYTES {
        return Err(ApiError::Http);
    }
    Ok(response.body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_response(body: serde_json::Value) -> String {
        serde_json::json!({ "status": 200, "headers": {}, "body": body.to_string() }).to_string()
    }

    #[test]
    fn create_task_uses_the_fixed_endpoint_and_image_task_contract() {
        let value: serde_json::Value =
            serde_json::from_str(&build_create_task_request("secret", "aW1hZ2U=").unwrap())
                .unwrap();
        assert_eq!(value["method"], "POST");
        assert_eq!(value["url"], CREATE_TASK_URL);
        let body: serde_json::Value =
            serde_json::from_str(value["body"].as_str().unwrap()).unwrap();
        assert_eq!(body["clientKey"], "secret");
        assert_eq!(body["task"]["type"], "ImageToTextTask");
        assert_eq!(body["task"]["body"], "aW1hZ2U=");
    }

    #[test]
    fn task_lifecycle_parses_processing_and_trimmed_solution() {
        assert_eq!(
            parse_create_task_response(&host_response(serde_json::json!({
                "errorId": 0,
                "taskId": 42
            })))
            .unwrap(),
            42
        );
        assert_eq!(
            parse_task_result_response(&host_response(serde_json::json!({
                "errorId": 0,
                "status": "processing"
            })))
            .unwrap(),
            TaskResult::Processing
        );
        assert_eq!(
            parse_task_result_response(&host_response(serde_json::json!({
                "errorId": 0,
                "status": "ready",
                "solution": { "text": "  abc123 " }
            })))
            .unwrap(),
            TaskResult::Ready("abc123".into())
        );
    }

    #[test]
    fn service_errors_are_opaque() {
        let error = parse_create_task_response(&host_response(serde_json::json!({
            "errorId": 1,
            "errorDescription": "untrusted service diagnostic"
        })))
        .unwrap_err();
        assert!(!error.to_string().contains("untrusted"));
    }
}
