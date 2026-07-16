use serde_json::Value;

use crate::ffi::*;

pub(crate) fn json_value_type(value: &Value) -> u8 {
    match value {
        Value::Null => 1,
        Value::Bool(_) => 2,
        Value::String(_) => 3,
        _ => 4,
    }
}

pub(crate) fn json_value_string(value: &Value) -> String {
    value.to_string()
}

pub(crate) fn logical_type(format_type: &str, _is_array: bool) -> i32 {
    match format_type.to_ascii_lowercase().as_str() {
        "checkbox" => RUST_EXT_LOGICAL_BOOLEAN,
        "text" | "email" | "select" => RUST_EXT_LOGICAL_VARCHAR,
        "number" | "percent" | "slider" | "scale" => RUST_EXT_LOGICAL_DECIMAL,
        "date" => RUST_EXT_LOGICAL_DATE,
        "datetime" => RUST_EXT_LOGICAL_TIMESTAMP_TZ,
        "time" => RUST_EXT_LOGICAL_TIME,
        "duration" => RUST_EXT_LOGICAL_INTERVAL,
        "currency" => RUST_EXT_LOGICAL_CURRENCY,
        "image" => RUST_EXT_LOGICAL_IMAGE,
        "person" => RUST_EXT_LOGICAL_PERSON,
        // The public API currently calls this format `link`; accept `hyperlink` as well
        // because that is the corresponding rich value's terminology.
        "link" | "hyperlink" => RUST_EXT_LOGICAL_HYPERLINK,
        "lookup" => RUST_EXT_LOGICAL_LOOKUP,
        _ => RUST_EXT_LOGICAL_JSON,
    }
}

pub(crate) fn table_list_from_json(body: &str) -> Result<RustExtTableList, String> {
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
        tables.push(RustExtTable {
            id: alloc_string(
                item.get("id")
                    .and_then(Value::as_str)
                    .ok_or("missing table id")?,
            ),
            name: alloc_string(
                item.get("name")
                    .and_then(Value::as_str)
                    .ok_or("missing table name")?,
            ),
            capabilities,
        });
    }
    let (items_ptr, count) = vec_into_raw_parts(tables);
    Ok(RustExtTableList {
        items: items_ptr,
        count,
        next_page_token: alloc_string(
            root.get("nextPageToken")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
    })
}

pub(crate) fn column_list_from_json(body: &str) -> Result<RustExtColumnList, String> {
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
        }
        if !calculated {
            capabilities |= RUST_EXT_COLUMN_EDITABLE;
        }
        if !is_array {
            capabilities |= RUST_EXT_COLUMN_FILTER_EQUALITY;
        } else {
            capabilities |= RUST_EXT_COLUMN_ARRAY;
        }
        columns.push(RustExtColumn {
            id: alloc_string(
                item.get("id")
                    .and_then(Value::as_str)
                    .ok_or("missing column id")?,
            ),
            name: alloc_string(
                item.get("name")
                    .and_then(Value::as_str)
                    .ok_or("missing column name")?,
            ),
            type_name: alloc_string(format_type),
            capabilities,
            logical_type: logical_type(format_type, is_array),
        });
    }
    let (items_ptr, count) = vec_into_raw_parts(columns);
    Ok(RustExtColumnList {
        items: items_ptr,
        count,
        next_page_token: alloc_string(
            root.get("nextPageToken")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
    })
}

pub(crate) fn rows_from_json(body: &str) -> Result<CodaRowsResponse, String> {
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
                cells.push(RustExtCell {
                    column_id: alloc_string(column_id),
                    value_type: json_value_type(value),
                    value: alloc_string(&json_value_string(value)),
                });
            }
        }
        let (cells_ptr, cell_count) = vec_into_raw_parts(cells);
        rows.push(RustExtRow {
            id: alloc_string(
                item.get("id")
                    .and_then(Value::as_str)
                    .ok_or("missing row id")?,
            ),
            created_at: alloc_string(item.get("createdAt").and_then(Value::as_str).unwrap_or("")),
            updated_at: alloc_string(item.get("updatedAt").and_then(Value::as_str).unwrap_or("")),
            deleted: item
                .get("deleted")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || item
                    .get("isDeleted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            cells: cells_ptr,
            cell_count,
        });
    }
    let (rows_ptr, row_count) = vec_into_raw_parts(rows);
    Ok(CodaRowsResponse {
        rows: rows_ptr,
        row_count,
        next_page_token: alloc_string(
            root.get("nextPageToken")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        next_sync_token: alloc_string(
            root.get("nextSyncToken")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
    })
}

pub(crate) fn free_columns(list: RustExtColumnList) {
    for item in vec_from_raw_parts(list.items, list.count) {
        item.id.free();
        item.name.free();
        item.type_name.free();
    }
    list.next_page_token.free();
}

pub(crate) fn free_rows_partial(rows: *mut RustExtRow, row_count: usize) {
    for row in vec_from_raw_parts(rows, row_count) {
        row.id.free();
        row.created_at.free();
        row.updated_at.free();
        for cell in vec_from_raw_parts(row.cells, row.cell_count) {
            cell.column_id.free();
            cell.value.free();
        }
    }
}

#[cfg(test)]
pub(crate) fn free_coda_rows_response(response: CodaRowsResponse) {
    free_rows_partial(response.rows, response.row_count);
    response.next_page_token.free();
    response.next_sync_token.free();
}

pub(crate) fn free_scan_batch(batch: RustExtScanBatch) {
    free_rows_partial(batch.rows, batch.row_count);
}

pub(crate) fn free_catalog(catalog: RustExtCatalog) {
    for table in vec_from_raw_parts(catalog.tables, catalog.table_count) {
        table.id.free();
        table.name.free();
        free_columns(RustExtColumnList {
            items: table.columns,
            count: table.column_count,
            next_page_token: RustExtString::default(),
        });
    }
}

pub(crate) fn prepare_columns(columns: &mut [RustExtColumn]) {
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
            .any(|column| column.name.as_str().eq_ignore_ascii_case(&candidate))
        {
            suffix += 1;
            candidate = format!("{base}_{suffix}");
        }
        if candidate != original {
            columns[idx].name.free();
            columns[idx].name = alloc_string(&candidate);
        }
    }
}

pub(crate) fn append_row_metadata(columns: &mut Vec<RustExtColumn>) {
    for name in ["createdAt", "updatedAt"] {
        let mut capabilities = RUST_EXT_COLUMN_GENERATED | RUST_EXT_COLUMN_SYSTEM;
        capabilities |= RUST_EXT_COLUMN_SORT_ASC;
        columns.push(RustExtColumn {
            id: alloc_string(name),
            name: alloc_string(name),
            type_name: alloc_string("timestampTz"),
            capabilities,
            logical_type: RUST_EXT_LOGICAL_TIMESTAMP_TZ,
            ..Default::default()
        });
    }
}
