use serde_json::{json, Value};
use superhuman_docs::operations;

use crate::ffi::*;
use crate::sdk::SdkClient;

pub(crate) fn input_value_json(value: RustExtInputValue) -> Value {
    match value.value_type {
        0 => Value::Null,
        1 => Value::Bool(value.bool_value),
        2 => json!(value.int_value),
        3 => json!(value.uint_value),
        4 => json!(value.double_value),
        5 => Value::String(value.string_value.as_str().to_string()),
        _ => Value::Null,
    }
}

pub(crate) fn write_cells(columns: &[RustExtWriteColumn], values: &[RustExtInputValue]) -> Value {
    let mut cells = Vec::new();
    for (column, value) in columns.iter().zip(values.iter()) {
        if column.capabilities & RUST_EXT_COLUMN_EDITABLE == 0 {
            continue;
        }
        cells.push(json!({
            "column": column.id.as_str(),
            "value": input_value_json(*value),
        }));
    }
    json!({ "cells": cells })
}

pub(crate) fn insert_body(
    columns: &[RustExtWriteColumn],
    values: &[RustExtInputValue],
    row_count: usize,
    value_column_count: usize,
    table_capabilities: u32,
) -> Result<String, String> {
    if table_capabilities & RUST_EXT_TABLE_INSERT == 0 {
        return Err("insert is unsupported for this table".to_string());
    }
    if value_column_count < columns.len() || values.len() < row_count * value_column_count {
        return Err("invalid insert value shape".to_string());
    }
    let mut rows = Vec::with_capacity(row_count);
    for row_index in 0..row_count {
        let start = row_index * value_column_count;
        rows.push(write_cells(
            columns,
            &values[start..start + value_column_count],
        ));
    }
    Ok(json!({ "rows": rows }).to_string())
}

pub(crate) fn update_body(
    columns: &[RustExtWriteColumn],
    values: &[RustExtInputValue],
    table_capabilities: u32,
) -> Result<String, String> {
    if table_capabilities & RUST_EXT_TABLE_UPDATE == 0 {
        return Err("update is unsupported for this table".to_string());
    }
    if columns.len() != values.len() {
        return Err("invalid update value shape".to_string());
    }
    Ok(json!({ "row": write_cells(columns, values) }).to_string())
}

pub(crate) fn insert_rows(
    config: RustExtClientConfig,
    table_id: RustExtString,
    columns: &[RustExtWriteColumn],
    values: &[RustExtInputValue],
    row_count: usize,
    value_column_count: usize,
    table_capabilities: u32,
) -> Result<usize, String> {
    let sdk = SdkClient::new(config)?;
    let body = insert_body(
        columns,
        values,
        row_count,
        value_column_count,
        table_capabilities,
    )?;
    sdk.execute_with_body(body, |client| {
        client
            .tables()
            .rows()
            .upsert_rows(operations::UpsertRowsInput {
                doc_id: config.resource.as_str().to_string(),
                table_id_or_name: table_id.as_str().to_string(),
                disable_parsing: Some(false),
                payload: operations::RowsUpsert {
                    rows: Vec::new(),
                    key_columns: None,
                },
            })
    })?;
    Ok(row_count)
}

pub(crate) fn update_rows(
    config: RustExtClientConfig,
    table_id: RustExtString,
    row_ids: &[RustExtString],
    columns: &[RustExtWriteColumn],
    values: &[RustExtInputValue],
    table_capabilities: u32,
) -> Result<usize, String> {
    if columns.is_empty() && !row_ids.is_empty() {
        return Err("update column count mismatch".to_string());
    }
    if values.len() != row_ids.len() * columns.len() {
        return Err("update value count mismatch".to_string());
    }
    let sdk = SdkClient::new(config)?;
    for (row_index, row_id) in row_ids.iter().enumerate() {
        let start = row_index * columns.len();
        let body = update_body(
            columns,
            &values[start..start + columns.len()],
            table_capabilities,
        )?;
        sdk.execute_with_body(body, |client| {
            client.tables().rows().update(operations::UpdateRowInput {
                doc_id: config.resource.as_str().to_string(),
                table_id_or_name: table_id.as_str().to_string(),
                row_id_or_name: row_id.as_str().to_string(),
                disable_parsing: Some(false),
                payload: operations::RowUpdate {
                    row: operations::RowEdit { cells: Vec::new() },
                },
            })
        })?;
    }
    Ok(row_ids.len())
}

pub(crate) fn delete_rows(
    config: RustExtClientConfig,
    table_id: RustExtString,
    row_ids: &[RustExtString],
) -> Result<usize, String> {
    if row_ids.is_empty() {
        return Ok(0);
    }
    let sdk = SdkClient::new(config)?;
    sdk.execute(|client| {
        client
            .tables()
            .rows()
            .delete_rows(operations::DeleteRowsInput {
                doc_id: config.resource.as_str().to_string(),
                table_id_or_name: table_id.as_str().to_string(),
                payload: operations::RowsDelete {
                    row_ids: row_ids
                        .iter()
                        .map(|row_id| row_id.as_str().to_string())
                        .collect(),
                },
            })
    })?;
    Ok(row_ids.len())
}

pub(crate) fn build_equality_query(
    column_id: &str,
    column_name: &str,
    value: RustExtInputValue,
) -> Result<(RustExtString, RustExtString), String> {
    if value.value_type == 0 {
        return Err("null query value".to_string());
    }
    let literal = input_value_json(value).to_string();
    let description = match value.value_type {
        1 => value.bool_value.to_string(),
        2 => value.int_value.to_string(),
        3 => value.uint_value.to_string(),
        4 => value.double_value.to_string(),
        5 => value.string_value.as_str().to_string(),
        _ => "NULL".to_string(),
    };
    Ok((
        alloc_string(&format!("{column_id}:{literal}")),
        alloc_string(&format!("{column_name} = {description}")),
    ))
}
