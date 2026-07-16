use crate::ffi::*;
use serde_json::{Map, Value};

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(inner) => Some(inner.clone()),
        Value::Bool(_) | Value::Number(_) => Some(value.to_string()),
        Value::Object(inner) => inner
            .get("name")
            .or_else(|| inner.get("value"))
            .and_then(scalar_text),
        Value::Array(_) => None,
    }
}

fn projected_object(value: &Value, fields: &[&str]) -> Option<String> {
    let source = value.as_object()?;
    let mut projected = Map::with_capacity(fields.len());
    for field in fields {
        projected.insert(
            (*field).to_string(),
            source.get(*field).cloned().unwrap_or(Value::Null),
        );
    }
    Some(Value::Object(projected).to_string())
}

fn normalized_value(logical_type: i32, value: &Value) -> Option<(String, bool)> {
    let text = match logical_type {
        RUST_EXT_LOGICAL_JSON => value.to_string(),
        RUST_EXT_LOGICAL_CURRENCY => projected_object(value, &["currency", "amount"])?,
        RUST_EXT_LOGICAL_IMAGE => {
            projected_object(value, &["name", "url", "height", "width", "status"])?
        }
        RUST_EXT_LOGICAL_PERSON => projected_object(value, &["name", "email"])?,
        RUST_EXT_LOGICAL_HYPERLINK => projected_object(value, &["name", "url"])?,
        RUST_EXT_LOGICAL_LOOKUP => {
            projected_object(value, &["name", "url", "tableId", "tableUrl", "rowId"])?
        }
        RUST_EXT_LOGICAL_INTERVAL => {
            let scalar = scalar_text(value)?;
            if scalar.parse::<f64>().is_ok() {
                format!("{scalar} days")
            } else {
                scalar
            }
        }
        _ => scalar_text(value)?,
    };
    let bool_value = logical_type == RUST_EXT_LOGICAL_BOOLEAN
        && (value.as_bool() == Some(true) || text.eq_ignore_ascii_case("true"));
    Some((text, bool_value))
}

pub(crate) fn scan_value(column: RustExtColumn, row: RustExtRow) -> RustExtScanValue {
    let column_id = column.id.as_str();
    if column.capabilities & RUST_EXT_COLUMN_SYSTEM != 0 {
        let value = if column_id.eq_ignore_ascii_case("createdAt") {
            row.created_at
        } else if column_id.eq_ignore_ascii_case("updatedAt") {
            row.updated_at
        } else {
            RustExtString::default()
        };
        if value.as_str().is_empty() {
            return RustExtScanValue::default();
        }
        return RustExtScanValue {
            is_null: false,
            value_type: 3,
            value,
            ..Default::default()
        };
    }
    if row.cells.is_null() {
        return RustExtScanValue::default();
    }
    let cells = slice_from_raw_parts(row.cells, row.cell_count);
    for cell in cells {
        if !cell.column_id.as_str().eq_ignore_ascii_case(column_id) {
            continue;
        }
        if cell.value_type == 0 || cell.value_type == 1 {
            return RustExtScanValue::default();
        }
        let parsed: Value = match serde_json::from_str(cell.value.as_str()) {
            Ok(value) => value,
            Err(_) => return RustExtScanValue::default(),
        };
        if column.capabilities & RUST_EXT_COLUMN_ARRAY != 0 {
            let values = match parsed.as_array() {
                Some(values) => values,
                None => return RustExtScanValue::default(),
            };
            let mut array_values = Vec::with_capacity(values.len());
            for value in values {
                if value.is_null() {
                    array_values.push(RustExtArrayValue {
                        is_null: true,
                        value: RustExtString::default(),
                    });
                    continue;
                }
                let (value_text, _) = match normalized_value(column.logical_type, value) {
                    Some(value) => value,
                    None => {
                        array_values.push(RustExtArrayValue {
                            is_null: true,
                            value: RustExtString::default(),
                        });
                        continue;
                    }
                };
                array_values.push(RustExtArrayValue {
                    is_null: false,
                    value: alloc_string(&value_text),
                });
            }
            let (array_values, array_count) = vec_into_raw_parts(array_values);
            return RustExtScanValue {
                is_null: false,
                value_type: cell.value_type,
                array_values,
                array_count,
                ..Default::default()
            };
        }
        let (value_text, bool_value) = match normalized_value(column.logical_type, &parsed) {
            Some(value) => value,
            None => return RustExtScanValue::default(),
        };
        return RustExtScanValue {
            is_null: false,
            value_type: cell.value_type,
            bool_value,
            value_owned: true,
            value: alloc_string(&value_text),
            ..Default::default()
        };
    }
    RustExtScanValue::default()
}
