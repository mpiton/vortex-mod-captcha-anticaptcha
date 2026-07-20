use std::path::PathBuf;

use extism::{Function, UserData, Val, PTR};

fn wasm_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/wasm32-wasip1/release/vortex_mod_captcha_anticaptcha.wasm")
}

fn get_credential() -> Function {
    Function::new(
        "get_credential",
        [PTR],
        [PTR],
        UserData::<()>::default(),
        |plugin, inputs, outputs, _user_data: UserData<()>| {
            let service: String = plugin.memory_get_val(&inputs[0])?;
            if service != "vortex-mod-captcha-anticaptcha" {
                return Err(extism::Error::msg("credential scope mismatch"));
            }
            let credential = r#"{"username":"api-key","password":"test-secret"}"#;
            let handle = plugin.memory_new(credential)?;
            outputs[0] = Val::I64(handle.offset() as i64);
            Ok(())
        },
    )
}

fn http_request() -> Function {
    Function::new(
        "http_request",
        [PTR],
        [PTR],
        UserData::<()>::default(),
        |plugin, inputs, outputs, _user_data: UserData<()>| {
            let request: String = plugin.memory_get_val(&inputs[0])?;
            let request: serde_json::Value = serde_json::from_str(&request)?;
            let body: serde_json::Value = serde_json::from_str(
                request["body"]
                    .as_str()
                    .ok_or_else(|| extism::Error::msg("missing body"))?,
            )?;
            if body["clientKey"] != "test-secret" {
                return Err(extism::Error::msg("credential was not forwarded"));
            }
            let api_body = match request["url"].as_str() {
                Some("https://api.anti-captcha.com/createTask") => {
                    if body["task"]["type"] != "ImageToTextTask" {
                        return Err(extism::Error::msg("wrong task type"));
                    }
                    serde_json::json!({ "errorId": 0, "taskId": 7 })
                }
                Some("https://api.anti-captcha.com/getTaskResult") => serde_json::json!({
                    "errorId": 0,
                    "status": "ready",
                    "solution": { "text": "service-answer" }
                }),
                Some("https://api.anti-captcha.com/getBalance") => {
                    serde_json::json!({ "errorId": 0, "balance": 12.5 })
                }
                _ => return Err(extism::Error::msg("unexpected endpoint")),
            };
            let response = serde_json::json!({
                "status": 200,
                "headers": {},
                "body": api_body.to_string()
            })
            .to_string();
            let handle = plugin.memory_new(&response)?;
            outputs[0] = Val::I64(handle.offset() as i64);
            Ok(())
        },
    )
}

#[test]
fn wasm_exports_complete_the_paid_service_lifecycle() {
    let path = wasm_path();
    assert!(
        path.is_file(),
        "release WASM must be built before smoke tests"
    );
    let manifest = extism::Manifest::new([extism::Wasm::file(path)]);
    let mut plugin = extism::Plugin::new(&manifest, [http_request(), get_credential()], true)
        .expect("load WASM");
    let input = r#"{"challenge_id":"captcha-1","challenge_type":"image","challenge_url":"https://example.test","image_data":"aW1hZ2U="}"#;

    let supported: String = plugin.call("can_solve", input).expect("can_solve");
    let solved: String = plugin.call("solve", input).expect("solve");
    let balance: String = plugin.call("get_balance", "").expect("get_balance");

    assert_eq!(supported, "true");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&solved).unwrap(),
        serde_json::json!({ "status": "solved", "solution": "service-answer" })
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&balance).unwrap(),
        serde_json::json!({ "balance": 12.5 })
    );
}
