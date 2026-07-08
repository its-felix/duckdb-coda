use std::ffi::{c_char, c_void};

use superhuman_docs::DEFAULT_BASE_URL;

use crate::constants::{
    API_BASE_OPTION, INCLUDE_ROW_METADATA_OPTION, SECRET_SCOPE_PREFIX, SECRET_TYPE, TOKEN_OPTION,
};
use crate::ffi::*;
use crate::sdk::normalize_api_base;

pub(crate) fn resolve_attach(
    path: RustExtString,
    host: *const RustExtAttachHost,
    userdata: *mut c_void,
) -> Result<RustExtAttachConfig, String> {
    RustExtAttachHost::from_ptr(host)?;
    let credential = get_option(host, userdata, TOKEN_OPTION.as_ptr().cast())?;
    let endpoint = get_option(host, userdata, API_BASE_OPTION.as_ptr().cast())?;
    let include_row_metadata_option =
        get_option(host, userdata, INCLUDE_ROW_METADATA_OPTION.as_ptr().cast())?;
    let resource = path
        .as_str()
        .strip_prefix(SECRET_SCOPE_PREFIX)
        .unwrap_or(path.as_str());
    if resource.is_empty() {
        return Err("empty doc id".to_string());
    }
    let include_system_columns = match include_row_metadata_option.as_str() {
        "" | "false" | "FALSE" | "False" | "0" => false,
        "true" | "TRUE" | "True" | "1" => true,
        _ => return Err("invalid boolean attach option".to_string()),
    };
    let mut result = RustExtAttachConfig {
        resource: alloc_string(resource),
        credential: alloc_string(credential.as_str()),
        endpoint: alloc_string(&normalize_api_base(if endpoint.as_str().is_empty() {
            DEFAULT_BASE_URL
        } else {
            endpoint.as_str()
        })),
        primary_secret_scope: alloc_string(&format!("{SECRET_SCOPE_PREFIX}{resource}")),
        fallback_secret_scope: alloc_string(SECRET_SCOPE_PREFIX),
        include_system_columns,
    };
    credential.free();
    endpoint.free();
    include_row_metadata_option.free();
    if result.credential.as_str().is_empty() {
        let secret_token = lookup_secret(host, userdata, result.primary_secret_scope)?;
        if !secret_token.as_str().is_empty() {
            result.credential = secret_token;
        } else {
            secret_token.free();
            let fallback_token = lookup_secret(host, userdata, result.fallback_secret_scope)?;
            if fallback_token.as_str().is_empty() {
                fallback_token.free();
                return Err("missing credential".to_string());
            }
            result.credential = fallback_token;
        }
    }
    Ok(result)
}

pub(crate) fn get_option(
    host: *const RustExtAttachHost,
    userdata: *mut c_void,
    name: *const c_char,
) -> Result<RustExtString, String> {
    let host = RustExtAttachHost::from_ptr(host)?;
    let mut out = RustExtString::default();
    let mut err = RustExtError::default();
    if host.get_option(userdata, name, &mut out, &mut err) {
        Ok(out)
    } else {
        let message = err.message.as_str().to_string();
        err.message.free();
        Err(if message.is_empty() {
            "host attach option failed".to_string()
        } else {
            message
        })
    }
}

pub(crate) fn lookup_secret(
    host: *const RustExtAttachHost,
    userdata: *mut c_void,
    scope: RustExtString,
) -> Result<RustExtString, String> {
    let host = RustExtAttachHost::from_ptr(host)?;
    let mut out = RustExtString::default();
    let mut err = RustExtError::default();
    if host.lookup_secret(
        userdata,
        scope,
        SECRET_TYPE.as_ptr().cast(),
        TOKEN_OPTION.as_ptr().cast(),
        &mut out,
        &mut err,
    ) {
        Ok(out)
    } else {
        let message = err.message.as_str().to_string();
        err.message.free();
        Err(if message.is_empty() {
            "host secret lookup failed".to_string()
        } else {
            message
        })
    }
}
