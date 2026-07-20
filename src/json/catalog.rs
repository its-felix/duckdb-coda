use serde_json::Value;

use crate::ffi::{
    RUST_EXT_COLUMN_EDITABLE, RUST_EXT_COLUMN_FILTER_EQUALITY, RUST_EXT_COLUMN_GENERATED,
    RUST_EXT_COLUMN_SORT_ASC, RUST_EXT_COLUMN_SYSTEM, RUST_EXT_TABLE_DELETE, RUST_EXT_TABLE_INSERT,
    RUST_EXT_TABLE_ROW_ID, RUST_EXT_TABLE_UPDATE, RUST_EXT_TABLE_VIEW,
};
use crate::model::{SuperhumanDocsColumn, SuperhumanDocsPage, SuperhumanDocsTable};

pub(crate) fn logical_type(format_type: &str, is_array: bool) -> String {
    let scalar = match format_type.to_ascii_lowercase().as_str() {
        "checkbox" => "BOOLEAN",
        "text" | "email" | "select" => "VARCHAR",
        "number" | "percent" | "slider" | "scale" => "DECIMAL(38,20)",
        "date" => "DATE",
        "datetime" => "TIMESTAMPTZ",
        "time" => "TIME",
        "duration" => "INTERVAL",
        "currency" => "STRUCT(currency VARCHAR, amount DECIMAL(38,20))",
        "image" => "STRUCT(name VARCHAR, url VARCHAR, height DOUBLE, width DOUBLE, status VARCHAR)",
        "person" => "STRUCT(name VARCHAR, email VARCHAR)",
        "link" | "hyperlink" => "STRUCT(name VARCHAR, url VARCHAR)",
        "lookup" => {
            "STRUCT(name VARCHAR, url VARCHAR, tableId VARCHAR, tableUrl VARCHAR, rowId VARCHAR)"
        }
        _ => "VARCHAR",
    };
    if is_array {
        format!("{scalar}[]")
    } else {
        scalar.to_string()
    }
}

pub(crate) fn logical_type_alias(format_type: &str) -> String {
    match format_type.to_ascii_lowercase().as_str() {
        "checkbox" | "text" | "email" | "select" | "number" | "percent" | "slider" | "scale"
        | "date" | "datetime" | "time" | "duration" | "currency" | "image" | "person" | "link"
        | "hyperlink" | "lookup" => String::new(),
        _ => "JSON".to_string(),
    }
}

pub(crate) fn table_list_from_json(
    body: &str,
) -> Result<SuperhumanDocsPage<SuperhumanDocsTable>, String> {
    let root: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let items = root
        .get("items")
        .and_then(Value::as_array)
        .ok_or("missing items array")?;
    let mut tables = Vec::with_capacity(items.len());
    for item in items {
        let table_type = item
            .get("tableType")
            .and_then(Value::as_str)
            .unwrap_or("table");
        let is_view = !table_type.eq_ignore_ascii_case("table");
        let capabilities = if is_view {
            RUST_EXT_TABLE_VIEW | RUST_EXT_TABLE_ROW_ID
        } else {
            RUST_EXT_TABLE_INSERT
                | RUST_EXT_TABLE_UPDATE
                | RUST_EXT_TABLE_DELETE
                | RUST_EXT_TABLE_ROW_ID
        };
        tables.push(SuperhumanDocsTable {
            id: item
                .get("id")
                .and_then(Value::as_str)
                .ok_or("missing table id")?
                .to_string(),
            name: item
                .get("name")
                .and_then(Value::as_str)
                .ok_or("missing table name")?
                .to_string(),
            capabilities,
        });
    }
    Ok(SuperhumanDocsPage {
        items: tables,
        next_page_token: root
            .get("nextPageToken")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

pub(crate) fn column_list_from_json(
    body: &str,
) -> Result<SuperhumanDocsPage<SuperhumanDocsColumn>, String> {
    let root: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let items = root
        .get("items")
        .and_then(Value::as_array)
        .ok_or("missing items array")?;
    let mut columns = Vec::with_capacity(items.len());
    for item in items {
        let format = item.get("format").and_then(Value::as_object);
        let format_type = format
            .and_then(|inner| inner.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("text");
        let is_array = format
            .and_then(|inner| inner.get("isArray"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let calculated = item
            .get("calculated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut capabilities = 0;
        if calculated {
            capabilities |= RUST_EXT_COLUMN_GENERATED;
        } else {
            capabilities |= RUST_EXT_COLUMN_EDITABLE;
        }
        if !is_array {
            capabilities |= RUST_EXT_COLUMN_FILTER_EQUALITY;
        }
        columns.push(SuperhumanDocsColumn {
            id: item
                .get("id")
                .and_then(Value::as_str)
                .ok_or("missing column id")?
                .to_string(),
            name: item
                .get("name")
                .and_then(Value::as_str)
                .ok_or("missing column name")?
                .to_string(),
            format_type: format_type.to_string(),
            duckdb_type: logical_type(format_type, is_array),
            duckdb_type_alias: logical_type_alias(format_type),
            is_array,
            capabilities,
        });
    }
    Ok(SuperhumanDocsPage {
        items: columns,
        next_page_token: root
            .get("nextPageToken")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

pub(crate) fn prepare_columns(columns: &mut [SuperhumanDocsColumn]) {
    for idx in 0..columns.len() {
        let original = columns[idx].name.as_str();
        let base = if original.is_empty() {
            "column"
        } else {
            original
        };
        let mut candidate = base.to_string();
        let mut suffix = 1;
        while columns[..idx]
            .iter()
            .any(|column| column.name.eq_ignore_ascii_case(&candidate))
        {
            suffix += 1;
            candidate = format!("{base}_{suffix}");
        }
        columns[idx].name = candidate;
    }
}

pub(crate) fn append_row_metadata(columns: &mut Vec<SuperhumanDocsColumn>) {
    for name in ["createdAt", "updatedAt"] {
        columns.push(SuperhumanDocsColumn {
            id: name.to_string(),
            name: name.to_string(),
            format_type: "datetime".to_string(),
            duckdb_type: logical_type("datetime", false),
            duckdb_type_alias: String::new(),
            is_array: false,
            capabilities: RUST_EXT_COLUMN_GENERATED
                | RUST_EXT_COLUMN_SYSTEM
                | RUST_EXT_COLUMN_SORT_ASC,
        });
    }
}
