use std::ffi::{c_char, c_void};

use crate::ffi::*;
use crate::model::column_from_handle;
use crate::mutation::build_equality_query;

#[no_mangle]
pub extern "C" fn rust_ext_build_equality_query(
    column: *mut c_void,
    value: RustExtInputValue,
    out_query: *mut RustExtString,
    out_description: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(
        err,
        "failed to build Superhuman Docs equality query",
        || {
            let column = column_from_handle(column)?;
            let (query, description) = build_equality_query(&column.id, &column.name, value)?;
            write_out(out_query, query)?;
            write_out(out_description, description)?;
            Ok(())
        },
    )
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_sort_by(column: *mut c_void, out: *mut RustExtString) -> bool {
    let column = match column_from_handle(column) {
        Ok(column) => column,
        Err(_) => return false,
    };
    if column.capabilities & RUST_EXT_COLUMN_SORT_ASC != 0 {
        write_out(out, alloc_string(&column.id)).is_ok()
    } else {
        let _ = write_out(out, RustExtString::default());
        false
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_can_filter_equality(column: *mut c_void) -> bool {
    column_from_handle(column)
        .map(|column| column.capabilities & RUST_EXT_COLUMN_FILTER_EQUALITY != 0)
        .unwrap_or(false)
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_function_name() -> *const c_char {
    c_static(b"superhuman_docs_scan\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_query_label() -> *const c_char {
    c_static(b"Superhuman Docs Query\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_sort_label() -> *const c_char {
    c_static(b"Superhuman Docs Sort\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_limit_label() -> *const c_char {
    c_static(b"Superhuman Docs Limit\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_column_index_out_of_range_message() -> *const c_char {
    c_static(b"Superhuman Docs scan column index out of range\0")
}
