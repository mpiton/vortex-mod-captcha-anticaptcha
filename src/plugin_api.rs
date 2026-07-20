use std::time::Duration;

use extism_pdk::*;
use serde::Deserialize;

use crate::api::{
    build_create_task_request, build_get_balance_request, build_get_task_result_request,
    parse_balance_response, parse_create_task_response, parse_task_result_response, TaskResult,
};
use crate::{handle_can_solve, image_data, solved_response, unavailable_response};

const SERVICE_NAME: &str = "vortex-mod-captcha-anticaptcha";
const MAX_POLLS: usize = 30;
const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialResponse {
    username: String,
    password: String,
}

#[host_fn]
extern "ExtismHost" {
    fn http_request(request: String) -> String;
    fn get_credential(service: String) -> String;
}

#[plugin_fn]
pub fn can_solve(input: String) -> FnResult<String> {
    handle_can_solve(&input).map_err(plugin_error)
}

#[plugin_fn]
pub fn solve(input: String) -> FnResult<String> {
    let image = image_data(&input).map_err(plugin_error)?;
    let api_key = read_api_key()?;
    let create_request =
        build_create_task_request(&api_key, &image).map_err(api_error_to_plugin)?;
    let create_response = call_http(create_request)?;
    let task_id = parse_create_task_response(&create_response).map_err(api_error_to_plugin)?;

    for poll in 0..MAX_POLLS {
        if poll > 0 {
            std::thread::sleep(POLL_INTERVAL);
        }
        let request =
            build_get_task_result_request(&api_key, task_id).map_err(api_error_to_plugin)?;
        let response = call_http(request)?;
        match parse_task_result_response(&response).map_err(api_error_to_plugin)? {
            TaskResult::Processing => {}
            TaskResult::Ready(solution) => {
                return solved_response(&solution).map_err(plugin_error);
            }
        }
    }
    unavailable_response().map_err(plugin_error)
}

#[plugin_fn]
pub fn get_balance(_input: String) -> FnResult<String> {
    let api_key = read_api_key()?;
    let request = build_get_balance_request(&api_key).map_err(api_error_to_plugin)?;
    let response = call_http(request)?;
    let balance = parse_balance_response(&response).map_err(api_error_to_plugin)?;
    Ok(serde_json::json!({ "balance": balance }).to_string())
}

fn read_api_key() -> FnResult<String> {
    // SAFETY: the host scopes credential access to this plugin's exact name.
    let raw = unsafe { get_credential(SERVICE_NAME.to_string())? };
    let credential: CredentialResponse = serde_json::from_str(&raw)
        .map_err(|_| plugin_error("invalid credential response".into()))?;
    let _ = credential.username;
    let api_key = credential.password.trim().to_string();
    if api_key.is_empty() || api_key.len() > 1_024 {
        return Err(plugin_error("AntiCaptcha credential is invalid".into()));
    }
    Ok(api_key)
}

fn call_http(request: String) -> FnResult<String> {
    // SAFETY: Vortex registers the capability-gated typed HTTP host function.
    Ok(unsafe { http_request(request)? })
}

fn api_error_to_plugin(error: crate::api::ApiError) -> WithReturnCode<extism_pdk::Error> {
    plugin_error(error.to_string())
}

fn plugin_error(message: String) -> WithReturnCode<extism_pdk::Error> {
    extism_pdk::Error::msg(message).into()
}
