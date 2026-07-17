use std::ffi::{c_char, c_void};

use crate::attach::resolve_attach;
use crate::client::{load_catalog, ScanHandle};
use crate::constants::*;
use crate::ffi::*;
use crate::json::{free_catalog, free_scan_batch};
use crate::model::{
    client_config, column_from_handle, row_from_handle, table_from_handle,
    SuperhumanDocsClientConfig,
};
use crate::mutation::{build_equality_query, delete_rows, insert_rows, update_rows};
use crate::scan::scan_value;
use crate::secret::{create_secret, free_secret};

fn host_callback_result(success: bool, error: RustExtError, fallback: &str) -> Result<(), String> {
    let message = error.message.as_str().to_string();
    error.message.free();
    if success {
        Ok(())
    } else if message.is_empty() {
        Err(fallback.to_string())
    } else {
        Err(message)
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_create_secret(
    input: RustExtSecretCreateInput,
    out: *mut RustExtSecretCreateResult,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to create extension secret", || {
        write_out(out, create_secret(input)?)
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_free_secret(secret: RustExtSecretCreateResult) {
    free_secret(secret);
}

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

#[no_mangle]
pub extern "C" fn rust_ext_extension_load(
    host: *const RustExtDuckDbHost,
    loader: *mut c_void,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to load Superhuman Docs extension", || {
        let host = RustExtDuckDbHost::from_ptr(host)?;
        let mut host_error = RustExtError::default();
        let success =
            host.set_description(loader, c_static(EXTENSION_DESCRIPTION), &mut host_error);
        host_callback_result(success, host_error, "failed to set extension description")?;
        let parameters = [
            RustExtSecretParameter {
                name: c_static_string(TOKEN_OPTION),
                logical_type: borrow_string("VARCHAR"),
            },
            RustExtSecretParameter {
                name: c_static_string(TOKEN_ENV_OPTION),
                logical_type: borrow_string("VARCHAR"),
            },
        ];
        let registration = RustExtSecretRegistration {
            secret_type: c_static_string(SECRET_TYPE),
            provider: c_static_string(SECRET_PROVIDER),
            extension: c_static_string(EXTENSION_NAME),
            parameters: parameters.as_ptr(),
            parameter_count: parameters.len(),
        };
        let mut host_error = RustExtError::default();
        let success = host.register_secret(loader, registration, &mut host_error);
        host_callback_result(success, host_error, "failed to register config secret")?;
        let mut host_error = RustExtError::default();
        let success =
            host.register_storage_extension(loader, c_static(EXTENSION_NAME), &mut host_error);
        host_callback_result(success, host_error, "failed to register storage extension")?;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_resolve_attach(
    path: RustExtString,
    host: *const RustExtAttachHost,
    userdata: *mut c_void,
    out: *mut RustExtAttachConfig,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(
        err,
        "failed to resolve Superhuman Docs attach config",
        || write_out(out, resolve_attach(path, host, userdata)?),
    )
}

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
pub extern "C" fn rust_ext_scan_value(
    column: *mut c_void,
    row: *mut c_void,
    out: *mut RustExtScanValue,
) -> bool {
    let column = match column_from_handle(column) {
        Ok(column) => column,
        Err(_) => return false,
    };
    let row = match row_from_handle(row) {
        Ok(row) => row,
        Err(_) => return false,
    };
    write_out(out, scan_value(column, row)).is_ok()
}

#[no_mangle]
pub extern "C" fn rust_ext_free_scan_value(value: RustExtScanValue) {
    if value.value_owned {
        value.value.free();
    }
    for item in vec_from_raw_parts(value.array_values, value.array_count) {
        if !item.is_null {
            item.value.free();
        }
    }
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

#[no_mangle]
pub extern "C" fn rust_ext_scan_open(
    config: RustExtClientConfig,
    table: *mut c_void,
    request: RustExtScanRequest,
    out: *mut *mut c_void,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to open Superhuman Docs scan", || {
        let handle = Box::new(ScanHandle::new(
            client_config(config)?,
            table_from_handle(table)?,
            request,
        )?);
        write_out(out, Box::into_raw(handle).cast::<c_void>())
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_next(
    scan: *mut c_void,
    out: *mut RustExtScanBatch,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(
        err,
        "failed to fetch next Superhuman Docs scan batch",
        || {
            let handle = mut_from_raw(scan.cast::<ScanHandle>(), "scan handle")?;
            write_out(out, handle.next_batch()?)
        },
    )
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_close(scan: *mut c_void) {
    if !scan.is_null() {
        drop(unsafe { Box::from_raw(scan.cast::<ScanHandle>()) });
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_client_load_catalog(
    config: RustExtClientConfig,
    out: *mut RustExtCatalog,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to load Superhuman Docs catalog", || {
        write_out(out, load_catalog(client_config(config)?)?)
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_client_insert_rows(
    config: RustExtClientConfig,
    table: *mut c_void,
    columns_ptr: *const RustExtWriteColumn,
    column_count: usize,
    values_ptr: *const RustExtInputValue,
    row_count: usize,
    value_column_count: usize,
    affected_count: *mut usize,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to insert Superhuman Docs rows", || {
        let columns = slice_from_raw_parts(columns_ptr, column_count);
        let values = slice_from_raw_parts(values_ptr, row_count * value_column_count);
        write_out(
            affected_count,
            insert_rows(
                client_config(config)?,
                table_from_handle(table)?,
                columns,
                values,
                row_count,
                value_column_count,
            )?,
        )
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_client_update_rows(
    config: RustExtClientConfig,
    table: *mut c_void,
    row_ids_ptr: *const RustExtString,
    row_count: usize,
    columns_ptr: *const RustExtWriteColumn,
    column_count: usize,
    values_ptr: *const RustExtInputValue,
    affected_count: *mut usize,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to update Superhuman Docs rows", || {
        let row_ids = slice_from_raw_parts(row_ids_ptr, row_count);
        let columns = slice_from_raw_parts(columns_ptr, column_count);
        let values = slice_from_raw_parts(values_ptr, row_count * column_count);
        write_out(
            affected_count,
            update_rows(
                client_config(config)?,
                table_from_handle(table)?,
                row_ids,
                columns,
                values,
            )?,
        )
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_client_delete_rows(
    config: RustExtClientConfig,
    table: *mut c_void,
    row_ids_ptr: *const RustExtString,
    count: usize,
    affected_count: *mut usize,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to delete Superhuman Docs rows", || {
        let row_ids = slice_from_raw_parts(row_ids_ptr, count);
        write_out(
            affected_count,
            delete_rows(client_config(config)?, table_from_handle(table)?, row_ids)?,
        )
    })
}

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
pub extern "C" fn rust_ext_free_scan_batch(batch: RustExtScanBatch) {
    free_scan_batch(batch);
}

#[no_mangle]
pub extern "C" fn rust_ext_free_catalog(catalog: RustExtCatalog) {
    free_catalog(catalog);
}

#[no_mangle]
pub extern "C" fn rust_ext_free_attach_config(config: RustExtAttachConfig) {
    if !config.handle.is_null() {
        drop(unsafe { Box::from_raw(config.handle.cast::<SuperhumanDocsClientConfig>()) });
    }
}
