use superhuman_docs::operations;

use crate::ffi::{RustExtInputValue, RustExtString, RustExtWriteColumn};
use crate::model::{SuperhumanDocsClientConfig, SuperhumanDocsTable};
use crate::sdk::SdkClient;

mod payload;
mod status;

pub(crate) use payload::{build_equality_query, insert_payload, update_payload};
use status::wait_for_mutation;

pub(crate) fn insert_rows(
    config: &SuperhumanDocsClientConfig,
    table: &SuperhumanDocsTable,
    columns: &[RustExtWriteColumn],
    values: &[RustExtInputValue],
    row_count: usize,
    value_column_count: usize,
) -> Result<usize, String> {
    let sdk = SdkClient::new(config)?;
    let payload = insert_payload(
        columns,
        values,
        row_count,
        value_column_count,
        table.capabilities,
    )?;
    let response = sdk.execute(|client| {
        client
            .tables()
            .rows()
            .upsert_rows(operations::UpsertRowsInput {
                doc_id: config.resource.clone(),
                table_id_or_name: table.id.clone(),
                disable_parsing: Some(false),
                payload,
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
        let payload = update_payload(
            columns,
            &values[start..start + columns.len()],
            table.capabilities,
        )?;
        let response = sdk.execute(|client| {
            client.tables().rows().update(operations::UpdateRowInput {
                doc_id: config.resource.clone(),
                table_id_or_name: table.id.clone(),
                row_id_or_name: row_id.as_str().to_string(),
                disable_parsing: Some(false),
                payload,
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
