use serde_json::Value;

use crate::model::{SuperhumanDocsCell, SuperhumanDocsRow, SuperhumanDocsRowsResponse};

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
