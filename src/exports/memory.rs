use std::ffi::c_char;

use crate::ffi::*;
use crate::model::SuperhumanDocsClientConfig;

#[no_mangle]
pub extern "C" fn rust_ext_alloc_string(
    ptr: *const c_char,
    len: usize,
    out: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to allocate Superhuman Docs string", || {
        write_out(out, alloc_string(str_from_raw(ptr, len)))
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_free_string(value: RustExtString) {
    value.free();
}

#[no_mangle]
pub extern "C" fn rust_ext_free_error(err: RustExtError) {
    err.message.free();
}

#[no_mangle]
pub extern "C" fn rust_ext_free_attach_config(config: RustExtAttachConfig) {
    if !config.handle.is_null() {
        drop(unsafe { Box::from_raw(config.handle.cast::<SuperhumanDocsClientConfig>()) });
    }
}
