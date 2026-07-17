use std::ffi::c_void;

use serde_json::Value;

use crate::ffi::*;
use crate::model::{
    SuperhumanDocsCell, SuperhumanDocsColumn, SuperhumanDocsPage, SuperhumanDocsRow,
    SuperhumanDocsRowsResponse, SuperhumanDocsTable,
};

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

pub(crate) fn rows_from_json(body: &str) -> Result<SuperhumanDocsRowsResponse, String> {
    let root: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let items = root
        .get("items")
        .and_then(Value::as_array)
        .ok_or("missing items array")?;
    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        let mut cells = Vec::new();
        if let Some(values) = item.get("values").and_then(Value::as_object) {
            for (column_id, value) in values {
                cells.push(SuperhumanDocsCell {
                    column_id: column_id.clone(),
                    value: value.clone(),
                });
            }
        }
        rows.push(SuperhumanDocsRow {
            id: item
                .get("id")
                .and_then(Value::as_str)
                .ok_or("missing row id")?
                .to_string(),
            created_at: item
                .get("createdAt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            updated_at: item
                .get("updatedAt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            deleted: item
                .get("deleted")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || item
                    .get("isDeleted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            cells,
        });
    }
    Ok(SuperhumanDocsRowsResponse {
        rows,
        next_page_token: root
            .get("nextPageToken")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        next_sync_token: root
            .get("nextSyncToken")
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

fn ffi_column(column: SuperhumanDocsColumn) -> RustExtColumn {
    let handle = Box::into_raw(Box::new(column));
    let column = unsafe { &*handle };
    RustExtColumn {
        handle: handle.cast::<c_void>(),
        name: borrow_string(&column.name),
        logical_type: borrow_string(&column.duckdb_type),
        value_type_alias: borrow_string(&column.duckdb_type_alias),
        capabilities: column.capabilities,
    }
}

pub(crate) fn ffi_catalog_table(
    table: SuperhumanDocsTable,
    columns: Vec<SuperhumanDocsColumn>,
) -> RustExtCatalogTable {
    let handle = Box::into_raw(Box::new(table));
    let table = unsafe { &*handle };
    let (columns, column_count) = vec_into_raw_parts(columns.into_iter().map(ffi_column).collect());
    RustExtCatalogTable {
        handle: handle.cast::<c_void>(),
        name: borrow_string(&table.name),
        capabilities: table.capabilities,
        columns,
        column_count,
    }
}

pub(crate) fn ffi_scan_batch(rows: Vec<SuperhumanDocsRow>, finished: bool) -> RustExtScanBatch {
    let rows = rows
        .into_iter()
        .map(|row| {
            let handle = Box::into_raw(Box::new(row));
            let row = unsafe { &*handle };
            RustExtScanRow {
                handle: handle.cast::<c_void>(),
                row_id: borrow_string(&row.id),
            }
        })
        .collect();
    let (rows, row_count) = vec_into_raw_parts(rows);
    RustExtScanBatch {
        rows,
        row_count,
        finished,
    }
}

pub(crate) fn free_scan_batch(batch: RustExtScanBatch) {
    for row in vec_from_raw_parts(batch.rows, batch.row_count) {
        if !row.handle.is_null() {
            drop(unsafe { Box::from_raw(row.handle.cast::<SuperhumanDocsRow>()) });
        }
    }
}

pub(crate) fn free_catalog(catalog: RustExtCatalog) {
    for table in vec_from_raw_parts(catalog.tables, catalog.table_count) {
        for column in vec_from_raw_parts(table.columns, table.column_count) {
            if !column.handle.is_null() {
                drop(unsafe { Box::from_raw(column.handle.cast::<SuperhumanDocsColumn>()) });
            }
        }
        if !table.handle.is_null() {
            drop(unsafe { Box::from_raw(table.handle.cast::<SuperhumanDocsTable>()) });
        }
    }
}
