use std::ffi::{c_char, c_void};

use crate::attach::{read_environment_variable, resolve_attach};
use crate::client::{load_catalog, ScanHandle};
use crate::constants::*;
use crate::ffi::*;
use crate::json::{free_catalog, free_scan_batch};
use crate::mutation::{build_equality_query, delete_rows, insert_rows, update_rows};
use crate::scan::scan_value;
use crate::sdk::validate_token;

#[no_mangle]
pub extern "C" fn rust_ext_secret_config_missing_message(
    secret_type_ptr: *const c_char,
    secret_type_len: usize,
    out: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to build Coda secret config error", || {
        write_out(
            out,
            alloc_string(&format!(
                "No Coda secret configuration registered for type: {}",
                str_from_raw(secret_type_ptr, secret_type_len)
            )),
        )
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_secret_unknown_parameter_message(
    secret_type_ptr: *const c_char,
    secret_type_len: usize,
    parameter_ptr: *const c_char,
    parameter_len: usize,
    out: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to build Coda secret parameter error", || {
        write_out(
            out,
            alloc_string(&format!(
                "Unknown named parameter for {} secret: {}",
                str_from_raw(secret_type_ptr, secret_type_len),
                str_from_raw(parameter_ptr, parameter_len)
            )),
        )
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_secret_canonical_parameter_name(
    secret_key_ptr: *const c_char,
    secret_key_len: usize,
    parameter_ptr: *const c_char,
    parameter_len: usize,
    out: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to canonicalize Coda secret parameter", || {
        let secret_key = str_from_raw(secret_key_ptr, secret_key_len);
        let parameter = str_from_raw(parameter_ptr, parameter_len);
        write_out(
            out,
            if secret_key.eq_ignore_ascii_case(parameter) {
                alloc_string(secret_key)
            } else {
                RustExtString::default()
            },
        )
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_validate_secret_token(
    token_ptr: *const c_char,
    token_len: usize,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to validate Coda secret", || {
        validate_token(str_from_raw(token_ptr, token_len))
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_read_environment_variable(
    name_ptr: *const c_char,
    name_len: usize,
    out: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(
        err,
        "failed to resolve Coda token environment variable",
        || {
            write_out(
                out,
                alloc_string(&read_environment_variable(str_from_raw(
                    name_ptr, name_len,
                ))?),
            )
        },
    )
}

#[no_mangle]
pub extern "C" fn rust_ext_extension_name() -> *const c_char {
    c_static(EXTENSION_NAME)
}

#[no_mangle]
pub extern "C" fn rust_ext_unsupported_update_expression_message() -> *const c_char {
    c_static(b"Unsupported Coda UPDATE expression type\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_row_id_column_name() -> *const c_char {
    c_static(b"rowid\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_insert_operator_name() -> *const c_char {
    c_static(b"CODA_INSERT\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_update_operator_name() -> *const c_char {
    c_static(b"CODA_UPDATE\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_delete_operator_name() -> *const c_char {
    c_static(b"CODA_DELETE\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_dml_not_supported_message(operation: u8) -> *const c_char {
    match operation {
        0 => c_static(b"Coda INSERT is not supported for this table\0"),
        1 => c_static(b"Coda UPDATE is not supported for this table\0"),
        2 => c_static(b"Coda DELETE is not supported for this table\0"),
        _ => c_static(b"Coda DML is not supported for this table\0"),
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_ddl_not_supported_message(operation: u8) -> *const c_char {
    match operation {
        0 => c_static(b"Coda DDL is not supported: CREATE SCHEMA\0"),
        1 => c_static(b"Coda DDL is not supported: DROP SCHEMA\0"),
        2 => c_static(b"Coda DDL is not supported: CREATE TABLE AS\0"),
        3 => c_static(b"Coda DDL is not supported: CREATE INDEX\0"),
        4 => c_static(b"Coda DDL is not supported: CREATE FUNCTION\0"),
        5 => c_static(b"Coda DDL is not supported: CREATE TABLE\0"),
        6 => c_static(b"Coda DDL is not supported: CREATE VIEW\0"),
        7 => c_static(b"Coda DDL is not supported: CREATE SEQUENCE\0"),
        8 => c_static(b"Coda DDL is not supported: CREATE TABLE FUNCTION\0"),
        9 => c_static(b"Coda DDL is not supported: CREATE COPY FUNCTION\0"),
        10 => c_static(b"Coda DDL is not supported: CREATE PRAGMA FUNCTION\0"),
        11 => c_static(b"Coda DDL is not supported: CREATE COLLATION\0"),
        12 => c_static(b"Coda DDL is not supported: CREATE TYPE\0"),
        13 => c_static(b"Coda DDL is not supported: ALTER\0"),
        _ => c_static(b"Coda DDL is not supported\0"),
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_ddl_drop_entry_not_supported_message(
    name_ptr: *const c_char,
    name_len: usize,
    out: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to build Coda DDL error", || {
        write_out(
            out,
            alloc_string(&format!(
                "Coda DDL is not supported: DROP {}",
                str_from_raw(name_ptr, name_len)
            )),
        )
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_database_size_not_available_message() -> *const c_char {
    c_static(b"Coda database size is not available\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_returning_not_supported_message(operation: u8) -> *const c_char {
    match operation {
        0 => c_static(b"RETURNING is not supported for Coda INSERT\0"),
        1 => c_static(b"RETURNING is not supported for Coda UPDATE\0"),
        2 => c_static(b"RETURNING is not supported for Coda DELETE\0"),
        _ => c_static(b"RETURNING is not supported for this Coda extension operation\0"),
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_explicit_transaction_not_supported_message() -> *const c_char {
    c_static(b"Coda does not support explicit DuckDB transactions; use autocommit statements\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_transaction_rollback_not_supported_message() -> *const c_char {
    c_static(b"Coda writes cannot be rolled back after they are sent; use autocommit statements\0")
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
    ffi_bool(err, "failed to load Coda extension", || {
        let host = RustExtDuckDbHost::from_ptr(host)?;
        if !host.set_description(loader, c_static(EXTENSION_DESCRIPTION), err) {
            return Err("failed to set extension description".to_string());
        }
        if !host.register_config_secret(
            loader,
            c_static(SECRET_TYPE),
            c_static(SECRET_PROVIDER),
            c_static(EXTENSION_NAME),
            c_static(SECRET_SCOPE_PREFIX_C),
            c_static(TOKEN_OPTION),
            c_static(TOKEN_ENV_OPTION),
            err,
        ) {
            return Err("failed to register config secret".to_string());
        }
        if !host.register_storage_extension(loader, c_static(EXTENSION_NAME), err) {
            return Err("failed to register storage extension".to_string());
        }
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
    ffi_bool(err, "failed to resolve Coda attach config", || {
        write_out(out, resolve_attach(path, host, userdata)?)
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_build_equality_query(
    column_id_ptr: *const c_char,
    column_id_len: usize,
    column_name_ptr: *const c_char,
    column_name_len: usize,
    value: RustExtInputValue,
    out_query: *mut RustExtString,
    out_description: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to build Coda equality query", || {
        let (query, description) = build_equality_query(
            str_from_raw(column_id_ptr, column_id_len),
            str_from_raw(column_name_ptr, column_name_len),
            value,
        )?;
        write_out(out_query, query)?;
        write_out(out_description, description)?;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_value(
    column: RustExtColumn,
    row: RustExtRow,
    out: *mut RustExtScanValue,
) -> bool {
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
pub extern "C" fn rust_ext_scan_sort_by(column: RustExtColumn, out: *mut RustExtString) -> bool {
    let id = column.id.as_str();
    if column.capabilities & RUST_EXT_COLUMN_SORT_ASC != 0 {
        write_out(out, alloc_string(id)).is_ok()
    } else {
        let _ = write_out(out, RustExtString::default());
        false
    }
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_can_filter_equality(column: RustExtColumn) -> bool {
    column.capabilities & RUST_EXT_COLUMN_FILTER_EQUALITY != 0
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_function_name() -> *const c_char {
    c_static(b"coda_scan\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_query_label() -> *const c_char {
    c_static(b"Coda Query\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_sort_label() -> *const c_char {
    c_static(b"Coda Sort\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_limit_label() -> *const c_char {
    c_static(b"Coda Limit\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_column_index_out_of_range_message() -> *const c_char {
    c_static(b"Coda scan column index out of range\0")
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_open(
    config: RustExtClientConfig,
    table_id: RustExtString,
    request: RustExtScanRequest,
    out: *mut *mut c_void,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to open Coda scan", || {
        let handle = Box::new(ScanHandle::new(config, table_id, request)?);
        write_out(out, Box::into_raw(handle).cast::<c_void>())
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_scan_next(
    scan: *mut c_void,
    out: *mut RustExtScanBatch,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to fetch next Coda scan batch", || {
        let handle = mut_from_raw(scan.cast::<ScanHandle>(), "scan handle")?;
        write_out(out, handle.next_batch()?)
    })
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
    ffi_bool(err, "failed to load Coda catalog", || {
        write_out(out, load_catalog(config)?)
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_client_insert_rows(
    config: RustExtClientConfig,
    table_id: RustExtString,
    columns_ptr: *const RustExtWriteColumn,
    column_count: usize,
    values_ptr: *const RustExtInputValue,
    row_count: usize,
    value_column_count: usize,
    table_capabilities: u32,
    affected_count: *mut usize,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to insert Coda rows", || {
        let columns = slice_from_raw_parts(columns_ptr, column_count);
        let values = slice_from_raw_parts(values_ptr, row_count * value_column_count);
        write_out(
            affected_count,
            insert_rows(
                config,
                table_id,
                columns,
                values,
                row_count,
                value_column_count,
                table_capabilities,
            )?,
        )
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_client_update_rows(
    config: RustExtClientConfig,
    table_id: RustExtString,
    row_ids_ptr: *const RustExtString,
    row_count: usize,
    columns_ptr: *const RustExtWriteColumn,
    column_count: usize,
    values_ptr: *const RustExtInputValue,
    table_capabilities: u32,
    affected_count: *mut usize,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to update Coda rows", || {
        let row_ids = slice_from_raw_parts(row_ids_ptr, row_count);
        let columns = slice_from_raw_parts(columns_ptr, column_count);
        let values = slice_from_raw_parts(values_ptr, row_count * column_count);
        write_out(
            affected_count,
            update_rows(
                config,
                table_id,
                row_ids,
                columns,
                values,
                table_capabilities,
            )?,
        )
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_client_delete_rows(
    config: RustExtClientConfig,
    table_id: RustExtString,
    row_ids_ptr: *const RustExtString,
    count: usize,
    affected_count: *mut usize,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to delete Coda rows", || {
        let row_ids = slice_from_raw_parts(row_ids_ptr, count);
        write_out(affected_count, delete_rows(config, table_id, row_ids)?)
    })
}

#[no_mangle]
pub extern "C" fn rust_ext_alloc_string(
    ptr: *const c_char,
    len: usize,
    out: *mut RustExtString,
    err: *mut RustExtError,
) -> bool {
    ffi_bool(err, "failed to allocate Coda string", || {
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
    config.resource.free();
    config.credential.free();
    config.endpoint.free();
    config.primary_secret_scope.free();
    config.fallback_secret_scope.free();
}
