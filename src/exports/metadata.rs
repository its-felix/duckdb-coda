use std::ffi::c_char;

use crate::constants::EXTENSION_NAME;
use crate::ffi::*;

#[no_mangle]
pub extern "C" fn rust_ext_extension_name() -> *const c_char {
    c_static(EXTENSION_NAME)
}

#[no_mangle]
pub extern "C" fn rust_ext_unsupported_update_expression_message() -> *const c_char {
    c_static(b"Unsupported Superhuman Docs UPDATE expression type\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_row_id_column_name() -> *const c_char {
    c_static(b"rowid\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_insert_operator_name() -> *const c_char {
    c_static(b"SUPERHUMAN_DOCS_INSERT\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_update_operator_name() -> *const c_char {
    c_static(b"SUPERHUMAN_DOCS_UPDATE\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_delete_operator_name() -> *const c_char {
    c_static(b"SUPERHUMAN_DOCS_DELETE\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_dml_not_supported_message(operation: u8) -> *const c_char {
    match operation {
        0 => c_static(b"Superhuman Docs INSERT is not supported for this table\0"),
        1 => c_static(b"Superhuman Docs UPDATE is not supported for this table\0"),
        2 => c_static(b"Superhuman Docs DELETE is not supported for this table\0"),
        _ => c_static(b"Superhuman Docs DML is not supported for this table\0"),
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_ddl_not_supported_message(operation: u8) -> *const c_char {
    match operation {
        0 => c_static(b"Superhuman Docs DDL is not supported: CREATE SCHEMA\0"),
        1 => c_static(b"Superhuman Docs DDL is not supported: DROP SCHEMA\0"),
        2 => c_static(b"Superhuman Docs DDL is not supported: CREATE TABLE AS\0"),
        3 => c_static(b"Superhuman Docs DDL is not supported: CREATE INDEX\0"),
        4 => c_static(b"Superhuman Docs DDL is not supported: CREATE FUNCTION\0"),
        5 => c_static(b"Superhuman Docs DDL is not supported: CREATE TABLE\0"),
        6 => c_static(b"Superhuman Docs DDL is not supported: CREATE VIEW\0"),
        7 => c_static(b"Superhuman Docs DDL is not supported: CREATE SEQUENCE\0"),
        8 => c_static(b"Superhuman Docs DDL is not supported: CREATE TABLE FUNCTION\0"),
        9 => c_static(b"Superhuman Docs DDL is not supported: CREATE COPY FUNCTION\0"),
        10 => c_static(b"Superhuman Docs DDL is not supported: CREATE PRAGMA FUNCTION\0"),
        11 => c_static(b"Superhuman Docs DDL is not supported: CREATE COLLATION\0"),
        12 => c_static(b"Superhuman Docs DDL is not supported: CREATE TYPE\0"),
        13 => c_static(b"Superhuman Docs DDL is not supported: ALTER\0"),
        _ => c_static(b"Superhuman Docs DDL is not supported\0"),
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_ddl_drop_entry_not_supported_message(
    name_ptr: *const c_char,
    name_len: usize,
    out: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to build Superhuman Docs DDL error", || {
        write_out(
            out,
            alloc_string(&format!(
                "Superhuman Docs DDL is not supported: DROP {}",
                str_from_raw(name_ptr, name_len)
            )),
        )
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_database_size_not_available_message() -> *const c_char {
    c_static(b"Superhuman Docs database size is not available\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_returning_not_supported_message(operation: u8) -> *const c_char {
    match operation {
        0 => c_static(b"RETURNING is not supported for Superhuman Docs INSERT\0"),
        1 => c_static(b"RETURNING is not supported for Superhuman Docs UPDATE\0"),
        2 => c_static(b"RETURNING is not supported for Superhuman Docs DELETE\0"),
        _ => c_static(b"RETURNING is not supported for this Superhuman Docs extension operation\0"),
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_explicit_transaction_not_supported_message() -> *const c_char {
    c_static(b"Superhuman Docs does not support explicit DuckDB transactions; use autocommit statements\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_transaction_rollback_not_supported_message() -> *const c_char {
    c_static(b"Superhuman Docs writes cannot be rolled back after they are sent; use autocommit statements\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_supports_explicit_transactions() -> bool {
    false
}

#[no_mangle]
pub extern "C" fn rust_ext_supports_transaction_rollback() -> bool {
    false
}
