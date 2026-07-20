use superhuman_docs::operations;

use crate::ffi::{vec_into_raw_parts, RustExtCatalog};
use crate::json::ffi::ffi_catalog_table;
use crate::json::{
    append_row_metadata, column_list_from_json, prepare_columns, table_list_from_json,
};
use crate::model::{SuperhumanDocsClientConfig, SuperhumanDocsColumn};
use crate::sdk::SdkClient;

fn load_columns(
    sdk: &SdkClient,
    doc_id: &str,
    table_id: &str,
    include_system_columns: bool,
) -> Result<Vec<SuperhumanDocsColumn>, String> {
    let mut all = Vec::new();
    let mut page_token = String::new();
    loop {
        let body = sdk.execute(|client| {
            client
                .tables()
                .columns()
                .list(operations::ListColumnsInput {
                    doc_id: doc_id.to_string(),
                    table_id_or_name: table_id.to_string(),
                    limit: Some(100),
                    page_token: (!page_token.is_empty()).then(|| page_token.clone()),
                    visible_only: Some(false),
                })
        })?;
        let page = column_list_from_json(&body)?;
        all.extend(page.items);
        page_token = page.next_page_token;
        if page_token.is_empty() {
            break;
        }
    }
    if include_system_columns {
        append_row_metadata(&mut all);
    }
    prepare_columns(&mut all);
    Ok(all)
}

pub(crate) fn load_catalog(config: &SuperhumanDocsClientConfig) -> Result<RustExtCatalog, String> {
    let sdk = SdkClient::new(config)?;
    let doc_id = config.resource.clone();
    let mut catalog_tables = Vec::new();
    let mut page_token = String::new();
    loop {
        let body = sdk.execute(|client| {
            client.tables().list(operations::ListTablesInput {
                doc_id: doc_id.clone(),
                limit: Some(100),
                page_token: (!page_token.is_empty()).then(|| page_token.clone()),
                sort_by: None,
                table_types: None,
            })
        })?;
        let page = table_list_from_json(&body)?;
        for table in page.items {
            let columns = load_columns(&sdk, &doc_id, &table.id, config.include_system_columns)?;
            catalog_tables.push((table, columns));
        }
        page_token = page.next_page_token;
        if page_token.is_empty() {
            break;
        }
    }
    let (tables, table_count) = vec_into_raw_parts(
        catalog_tables
            .into_iter()
            .map(|(table, columns)| ffi_catalog_table(table, columns))
            .collect(),
    );
    Ok(RustExtCatalog {
        tables,
        table_count,
    })
}
