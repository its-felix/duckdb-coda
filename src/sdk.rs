use superhuman_docs::{Request, DEFAULT_BASE_URL};

use crate::ffi::RustExtClientConfig;

pub(crate) fn endpoint(config: RustExtClientConfig) -> String {
    let base = config.endpoint.as_str();
    normalize_api_base(if base.is_empty() {
        DEFAULT_BASE_URL
    } else {
        base
    })
}

pub(crate) fn normalize_api_base(base: &str) -> String {
    base.trim_end_matches('/').to_string()
}

pub(crate) fn non_empty_string(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn send_request(
    config: RustExtClientConfig,
    sdk_request: Request,
) -> Result<String, String> {
    let credential = config.credential.as_str();
    let agent = ureq::AgentBuilder::new().build();
    let method = sdk_request.method.as_str();
    let http_request = agent
        .request(method, &sdk_request.url)
        .set("Authorization", &format!("Bearer {credential}"))
        .set("Content-Type", "application/json");
    let response = match sdk_request.body {
        Some(body) => http_request.send_string(&body),
        None => http_request.call(),
    }
    .map_err(|e| e.to_string())?;
    if response.status() != sdk_request.expected_status {
        return Err(format!(
            "{} returned HTTP {}, expected {}",
            sdk_request.operation,
            response.status(),
            sdk_request.expected_status
        ));
    }
    response.into_string().map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn send_request(
    _config: RustExtClientConfig,
    sdk_request: Request,
) -> Result<String, String> {
    Err(format!(
        "{} is not available in DuckDB-Wasm builds",
        sdk_request.operation
    ))
}
