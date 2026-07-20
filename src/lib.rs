//! AntiCaptcha image solver for Vortex.

pub mod api;

#[cfg(target_family = "wasm")]
mod plugin_api;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptchaRequest {
    pub challenge_id: String,
    pub challenge_type: String,
    pub challenge_url: String,
    pub image_data: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SolverResponse<'a> {
    pub status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution: Option<&'a str>,
}

pub fn handle_can_solve(input: &str) -> Result<String, String> {
    let request = parse_request(input)?;
    Ok((request.challenge_type == "image" && request.image_data.is_some()).to_string())
}

pub fn image_data(input: &str) -> Result<String, String> {
    let request = parse_request(input)?;
    if request.challenge_type != "image" {
        return Err("unsupported CAPTCHA type".into());
    }
    request
        .image_data
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "CAPTCHA image is missing".into())
}

pub fn solved_response(solution: &str) -> Result<String, String> {
    serde_json::to_string(&SolverResponse {
        status: "solved",
        solution: Some(solution),
    })
    .map_err(|_| "could not encode solver response".into())
}

pub fn unavailable_response() -> Result<String, String> {
    serde_json::to_string(&SolverResponse {
        status: "unavailable",
        solution: None,
    })
    .map_err(|_| "could not encode solver response".into())
}

fn parse_request(input: &str) -> Result<CaptchaRequest, String> {
    serde_json::from_str(input).map_err(|_| "invalid CAPTCHA request".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_image_challenges_with_bytes_are_supported() {
        let image = r#"{"challenge_id":"1","challenge_type":"image","challenge_url":"https://example.test","image_data":"aW1hZ2U="}"#;
        let text = r#"{"challenge_id":"1","challenge_type":"text_input","challenge_url":"https://example.test","image_data":null}"#;
        assert_eq!(handle_can_solve(image).unwrap(), "true");
        assert_eq!(handle_can_solve(text).unwrap(), "false");
    }

    #[test]
    fn response_contains_no_service_or_credential_metadata() {
        let value: serde_json::Value =
            serde_json::from_str(&solved_response("abc123").unwrap()).unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "status": "solved", "solution": "abc123" })
        );
    }
}
