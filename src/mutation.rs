use serde_json::{json, Value};
use std::thread;
use std::time::{Duration, Instant};
use superhuman_docs::operations;

use crate::ffi::*;
use crate::model::{column_from_handle, SuperhumanDocsClientConfig, SuperhumanDocsTable};
use crate::sdk::SdkClient;

const MUTATION_POLL_INTERVAL: Duration = Duration::from_secs(1);

fn mutation_request_id(body: &str) -> Result<String, String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("invalid mutation response: {error}"))?;
    root.get("requestId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| "Superhuman Docs mutation response did not contain requestId".to_string())
}

fn mutation_status(body: &str) -> Result<(bool, Option<String>), String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("invalid mutation status response: {error}"))?;
    let completed = root
        .get("completed")
        .and_then(Value::as_bool)
        .ok_or("mutation status response did not contain completed")?;
    let warning = match root.get("warning") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value.trim().is_empty() => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => return Err("mutation status warning was not a string".to_string()),
    };
    Ok((completed, warning))
}

fn poll_mutation_status<Fetch, Elapsed, Sleep>(
    request_id: &str,
    timeout: Duration,
    allow_warnings: bool,
    mut fetch: Fetch,
    mut elapsed: Elapsed,
    mut sleep: Sleep,
) -> Result<(), String>
where
    Fetch: FnMut() -> Result<String, String>,
    Elapsed: FnMut() -> Duration,
    Sleep: FnMut(Duration),
{
    let mut first_check = true;
    loop {
        if !first_check && elapsed() >= timeout {
            return Err(format!(
                "Superhuman Docs mutation {request_id} did not complete within {} seconds; remote completion is unknown and may occur later",
                timeout.as_secs()
            ));
        }
        first_check = false;

        let body = fetch().map_err(|error| {
            format!("failed to check Superhuman Docs mutation {request_id}: {error}")
        })?;
        let (completed, warning) = mutation_status(&body).map_err(|error| {
            format!("failed to check Superhuman Docs mutation {request_id}: {error}")
        })?;
        if completed {
            if let Some(warning) = warning {
                if !allow_warnings {
                    return Err(format!(
                        "Superhuman Docs mutation {request_id} completed with a warning and cannot be rolled back: {warning}"
                    ));
                }
            }
            return Ok(());
        }

        let elapsed_now = elapsed();
        if elapsed_now >= timeout {
            return Err(format!(
                "Superhuman Docs mutation {request_id} did not complete within {} seconds; remote completion is unknown and may occur later",
                timeout.as_secs()
            ));
        }
        sleep(MUTATION_POLL_INTERVAL.min(timeout - elapsed_now));
    }
}

fn wait_for_mutation(
    sdk: &SdkClient,
    config: &SuperhumanDocsClientConfig,
    response_body: &str,
) -> Result<(), String> {
    if !config.wait_for_mutations {
        return Ok(());
    }
    let request_id = mutation_request_id(response_body)?;
    let timeout = Duration::from_secs(config.mutation_timeout_seconds);
    let started = Instant::now();
    poll_mutation_status(
        &request_id,
        timeout,
        config.allow_mutation_warnings,
        || {
            let response = sdk.execute_accepting_status(404, |client| {
                client
                    .mutation_status()
                    .read(operations::GetMutationStatusInput {
                        request_id: request_id.clone(),
                    })
            })?;
            Ok(response.unwrap_or_else(|| r#"{"completed":false}"#.to_string()))
        },
        || started.elapsed(),
        thread::sleep,
    )
}

pub(crate) fn input_value_json(value: RustExtInputValue) -> Result<Value, String> {
    match value.value_type {
        0 => Ok(Value::Null),
        1 => Ok(Value::Bool(value.bool_value)),
        2 => Ok(json!(value.int_value)),
        3 => Ok(json!(value.uint_value)),
        4 => Ok(json!(value.double_value)),
        5 => Ok(Value::String(value.string_value.as_str().to_string())),
        RUST_EXT_INPUT_JSON => serde_json::from_str(value.string_value.as_str())
            .map_err(|error| format!("invalid composite DuckDB value: {error}")),
        _ => Err(format!(
            "unsupported DuckDB input value type: {}",
            value.value_type
        )),
    }
}

fn object_field(value: &Value, fields: &[&str]) -> Option<Value> {
    let object = value.as_object()?;
    fields
        .iter()
        .filter_map(|field| object.get(*field))
        .find(|field| !field.is_null() && field.as_str() != Some(""))
        .cloned()
}

fn simple_cell_value(format_type: &str, value: &Value) -> Result<Value, String> {
    if value.is_null() {
        return Err("Superhuman Docs cell arrays cannot contain NULL".to_string());
    }
    let normalized_type = format_type.to_ascii_lowercase();
    let simple = match normalized_type.as_str() {
        // The rows API accepts only primitive values (or arrays of primitives) for edits.
        // Rich JSON-LD objects are a read representation and must be reduced to the
        // primitive that the destination column parser understands.
        "currency" => object_field(value, &["amount"]),
        "image" => object_field(value, &["url"]),
        "person" => object_field(value, &["email", "name"]),
        "link" | "hyperlink" => object_field(value, &["url"]),
        "lookup" => object_field(value, &["rowId", "name"]),
        _ => Some(value.clone()),
    }
    .ok_or_else(|| format!("{format_type} value is missing its writable field"))?;

    if simple.is_object() || simple.is_array() || simple.is_null() {
        return Err(format!(
            "{format_type} values cannot be serialized to a Superhuman Docs cell primitive"
        ));
    }
    Ok(simple)
}

fn cell_value_json(
    column: &crate::model::SuperhumanDocsColumn,
    value: RustExtInputValue,
) -> Result<Value, String> {
    let value = input_value_json(value)?;
    if column.is_array {
        let values = value
            .as_array()
            .ok_or_else(|| format!("{} expects an array value, received {}", column.name, value))?;
        return values
            .iter()
            .map(|value| simple_cell_value(&column.format_type, value))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    simple_cell_value(&column.format_type, &value)
}

pub(crate) fn write_cells(
    columns: &[RustExtWriteColumn],
    values: &[RustExtInputValue],
    omit_nulls: bool,
) -> Result<Value, String> {
    let mut cells = Vec::new();
    for (column, value) in columns.iter().zip(values.iter()) {
        if column.capabilities & RUST_EXT_COLUMN_EDITABLE == 0 {
            continue;
        }
        if value.value_type == RUST_EXT_INPUT_NULL {
            if omit_nulls {
                continue;
            }
            return Err("Superhuman Docs does not support updating a cell to NULL".to_string());
        }
        let column = column_from_handle(column.handle)?;
        cells.push(json!({
            "column": column.id,
            "value": cell_value_json(column, *value)?,
        }));
    }
    Ok(json!({ "cells": cells }))
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
            true,
        )?);
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
    Ok(json!({ "row": write_cells(columns, values, false)? }).to_string())
}

pub(crate) fn insert_rows(
    config: &SuperhumanDocsClientConfig,
    table: &SuperhumanDocsTable,
    columns: &[RustExtWriteColumn],
    values: &[RustExtInputValue],
    row_count: usize,
    value_column_count: usize,
) -> Result<usize, String> {
    let sdk = SdkClient::new(config)?;
    let body = insert_body(
        columns,
        values,
        row_count,
        value_column_count,
        table.capabilities,
    )?;
    let response = sdk.execute_with_body(body, |client| {
        client
            .tables()
            .rows()
            .upsert_rows(operations::UpsertRowsInput {
                doc_id: config.resource.clone(),
                table_id_or_name: table.id.clone(),
                disable_parsing: Some(false),
                payload: operations::RowsUpsert {
                    rows: Vec::new(),
                    key_columns: None,
                },
            })
    })?;
    wait_for_mutation(&sdk, config, &response)?;
    Ok(row_count)
}

pub(crate) fn update_rows(
    config: &SuperhumanDocsClientConfig,
    table: &SuperhumanDocsTable,
    row_ids: &[RustExtString],
    columns: &[RustExtWriteColumn],
    values: &[RustExtInputValue],
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
            table.capabilities,
        )?;
        let response = sdk.execute_with_body(body, |client| {
            client.tables().rows().update(operations::UpdateRowInput {
                doc_id: config.resource.clone(),
                table_id_or_name: table.id.clone(),
                row_id_or_name: row_id.as_str().to_string(),
                disable_parsing: Some(false),
                payload: operations::RowUpdate {
                    row: operations::RowEdit { cells: Vec::new() },
                },
            })
        })?;
        wait_for_mutation(&sdk, config, &response)?;
    }
    Ok(row_ids.len())
}

pub(crate) fn delete_rows(
    config: &SuperhumanDocsClientConfig,
    table: &SuperhumanDocsTable,
    row_ids: &[RustExtString],
) -> Result<usize, String> {
    if row_ids.is_empty() {
        return Ok(0);
    }
    let sdk = SdkClient::new(config)?;
    let response = sdk.execute(|client| {
        client
            .tables()
            .rows()
            .delete_rows(operations::DeleteRowsInput {
                doc_id: config.resource.clone(),
                table_id_or_name: table.id.clone(),
                payload: operations::RowsDelete {
                    row_ids: row_ids
                        .iter()
                        .map(|row_id| row_id.as_str().to_string())
                        .collect(),
                },
            })
    })?;
    wait_for_mutation(&sdk, config, &response)?;
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
    let literal = input_value_json(value)?.to_string();
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::VecDeque;

    use super::*;

    #[test]
    fn mutation_response_requires_request_id() {
        assert_eq!(
            mutation_request_id(r#"{"requestId":"request-1"}"#).unwrap(),
            "request-1"
        );
        assert!(mutation_request_id("{}").unwrap_err().contains("requestId"));
        assert!(mutation_request_id("not-json")
            .unwrap_err()
            .contains("invalid mutation response"));
    }

    #[test]
    fn mutation_status_requires_boolean_completion_and_string_warning() {
        assert_eq!(
            mutation_status(r#"{"completed":true}"#).unwrap(),
            (true, None)
        );
        assert_eq!(
            mutation_status(r#"{"completed":true,"warning":"caveat"}"#).unwrap(),
            (true, Some("caveat".to_string()))
        );
        assert!(mutation_status("{}").unwrap_err().contains("completed"));
        assert!(mutation_status(r#"{"completed":true,"warning":1}"#)
            .unwrap_err()
            .contains("not a string"));
    }

    #[test]
    fn polling_checks_immediately_and_retries_until_complete() {
        let responses = Cell::new(0_u32);
        let elapsed = Cell::new(Duration::ZERO);
        poll_mutation_status(
            "request-1",
            Duration::from_secs(5),
            false,
            || {
                let count = responses.get() + 1;
                responses.set(count);
                Ok(if count == 1 {
                    r#"{"completed":false}"#.to_string()
                } else {
                    r#"{"completed":true}"#.to_string()
                })
            },
            || elapsed.get(),
            |duration| elapsed.set(elapsed.get() + duration),
        )
        .unwrap();
        assert_eq!(responses.get(), 2);
        assert_eq!(elapsed.get(), Duration::from_secs(1));
    }

    #[test]
    fn polling_applies_warning_policy() {
        let error = poll_mutation_status(
            "request-warning",
            Duration::from_secs(5),
            false,
            || Ok(r#"{"completed":true,"warning":"partial result"}"#.to_string()),
            || Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert!(error.contains("completed with a warning"));
        assert!(error.contains("cannot be rolled back"));

        poll_mutation_status(
            "request-warning",
            Duration::from_secs(5),
            true,
            || Ok(r#"{"completed":true,"warning":"partial result"}"#.to_string()),
            || Duration::ZERO,
            |_| {},
        )
        .unwrap();
    }

    #[test]
    fn polling_timeout_reports_ambiguous_remote_state() {
        let elapsed = Cell::new(Duration::ZERO);
        let mut responses = VecDeque::from([
            r#"{"completed":false}"#.to_string(),
            r#"{"completed":false}"#.to_string(),
        ]);
        let error = poll_mutation_status(
            "request-timeout",
            Duration::from_secs(1),
            false,
            || Ok(responses.pop_front().unwrap()),
            || elapsed.get(),
            |duration| elapsed.set(elapsed.get() + duration),
        )
        .unwrap_err();
        assert!(error.contains("did not complete within 1 seconds"));
        assert!(error.contains("may occur later"));
    }
}
