use crate::ffi::*;
use crate::model::{SuperhumanDocsColumn, SuperhumanDocsRow};
use serde_json::{Map, Value};

fn unfence_rich_text(value: &str) -> &str {
    let Some(inner) = value
        .strip_prefix("```")
        .and_then(|inner| inner.strip_suffix("```"))
    else {
        return value;
    };
    let inner = inner.strip_prefix('\n').unwrap_or(inner);
    inner.strip_suffix('\n').unwrap_or(inner)
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(inner) => Some(unfence_rich_text(inner).to_string()),
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

fn normalized_percent(value: &Value) -> Option<String> {
    let scalar = scalar_text(value)?;
    if scalar.parse::<f64>().is_ok() {
        return Some(scalar);
    }
    if !scalar.contains('%') {
        return None;
    }
    let percentage = scalar
        .chars()
        .filter(|ch| ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.' | ','))
        .collect::<String>()
        .replace(',', ".");
    let percentage = percentage.parse::<f64>().ok()?;
    Some((percentage / 100.0).to_string())
}

fn normalized_value(format_type: &str, value: &Value) -> Option<String> {
    let normalized_type = format_type.to_ascii_lowercase();
    let text = match normalized_type.as_str() {
        "currency" => projected_object(value, &["currency", "amount"])?,
        "image" => projected_object(value, &["name", "url", "height", "width", "status"])?,
        "person" => projected_object(value, &["name", "email"])?,
        "link" | "hyperlink" => projected_object(value, &["name", "url"])?,
        "lookup" => projected_object(value, &["name", "url", "tableId", "tableUrl", "rowId"])?,
        "duration" => {
            let scalar = scalar_text(value)?;
            if scalar.parse::<f64>().is_ok() {
                format!("{scalar} days")
            } else {
                scalar
            }
        }
        "percent" => normalized_percent(value)?,
        "checkbox" | "text" | "email" | "select" | "number" | "slider" | "scale" | "date"
        | "datetime" | "time" => scalar_text(value)?,
        _ => value.to_string(),
    };
    Some(text)
}

pub(crate) fn scan_value(
    column: &SuperhumanDocsColumn,
    row: &SuperhumanDocsRow,
) -> RustExtScanValue {
    let column_id = column.id.as_str();
    if column.capabilities & RUST_EXT_COLUMN_SYSTEM != 0 {
        let value = if column_id.eq_ignore_ascii_case("createdAt") {
            row.created_at.as_str()
        } else if column_id.eq_ignore_ascii_case("updatedAt") {
            row.updated_at.as_str()
        } else {
            ""
        };
        if value.is_empty() {
            return RustExtScanValue::default();
        }
        return RustExtScanValue {
            is_null: false,
            value: borrow_string(value),
            ..Default::default()
        };
    }
    for cell in &row.cells {
        if !cell.column_id.eq_ignore_ascii_case(column_id) {
            continue;
        }
        if cell.value.is_null() {
            return RustExtScanValue::default();
        }
        if column.is_array {
            let values = match cell.value.as_array() {
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
                let value_text = match normalized_value(&column.format_type, value) {
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
                array_values,
                array_count,
                ..Default::default()
            };
        }
        let value_text = match normalized_value(&column.format_type, &cell.value) {
            Some(value) => value,
            None => return RustExtScanValue::default(),
        };
        return RustExtScanValue {
            is_null: false,
            value_owned: true,
            value: alloc_string(&value_text),
            ..Default::default()
        };
    }
    RustExtScanValue::default()
}
