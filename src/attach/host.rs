use std::ffi::{c_char, c_void};

use crate::constants::{SECRET_TYPE, TOKEN_OPTION};
use crate::ffi::*;

pub(super) fn owned_option(
    host: *const RustExtAttachHost,
    userdata: *mut c_void,
    name: *const c_char,
) -> Result<String, String> {
    let value = get_option(host, userdata, name)?;
    let owned = value.as_str().to_string();
    value.free();
    Ok(owned)
}

pub(super) fn owned_secret(
    host: *const RustExtAttachHost,
    userdata: *mut c_void,
    scope: RustExtString,
) -> Result<String, String> {
    let value = lookup_secret(host, userdata, scope)?;
    let owned = value.as_str().to_string();
    value.free();
    Ok(owned)
}

pub(crate) fn read_environment_variable(name: &str) -> Result<String, String> {
    std::env::var(name)
        .map_err(|error| format!("failed to read environment variable {name}: {error}"))
}

fn get_option(
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

fn lookup_secret(
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
