use superhuman_docs::{operations, Request, DEFAULT_BASE_URL};

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
fn checked_response(credential: &str, sdk_request: Request) -> Result<ureq::Response, String> {
    let agent = ureq::AgentBuilder::new().build();
    let method = sdk_request.method.as_str();
    let http_request = agent
        .request(method, &sdk_request.url)
        .set("Authorization", &format!("Bearer {credential}"))
        .set("Content-Type", "application/json");
    let response = match match sdk_request.body {
        Some(body) => http_request.send_string(&body),
        None => http_request.call(),
    } {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(error) => return Err(error.to_string()),
    };
    if response.status() != sdk_request.expected_status {
        return Err(format!(
            "{} returned HTTP {}, expected {}",
            sdk_request.operation,
            response.status(),
            sdk_request.expected_status
        ));
    }
    Ok(response)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn send_request(
    config: RustExtClientConfig,
    sdk_request: Request,
) -> Result<String, String> {
    let response = checked_response(config.credential.as_str(), sdk_request)?;
    response.into_string().map_err(|e| e.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn validate_token_at(base_url: &str, credential: &str) -> Result<(), String> {
    let request = operations::build_whoami(&normalize_api_base(base_url))
        .map_err(|error| error.to_string())?;
    checked_response(credential, request).map(|_| ())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn validate_token(credential: &str) -> Result<(), String> {
    validate_token_at(DEFAULT_BASE_URL, credential)
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

#[cfg(target_arch = "wasm32")]
pub(crate) fn validate_token(_credential: &str) -> Result<(), String> {
    Err("Whoami is not available in DuckDB-Wasm builds".to_string())
}
