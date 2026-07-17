use superhuman_docs::operations;

use crate::ffi::*;
use crate::json::{
    append_row_metadata, column_list_from_json, prepare_columns, rows_from_json,
    table_list_from_json,
};
use crate::sdk::{non_empty_string, SdkClient};

fn load_columns(
    sdk: &SdkClient,
    doc_id: &str,
    table_id: &str,
    include_system_columns: bool,
) -> Result<RustExtColumnList, String> {
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
        let items = vec_from_raw_parts(page.items, page.count);
        all.extend(items);
        page_token = page.next_page_token.as_str().to_string();
        page.next_page_token.free();
        if page_token.is_empty() {
            break;
        }
    }
    if include_system_columns {
        append_row_metadata(&mut all);
    }
    prepare_columns(&mut all);
    let (items, count) = vec_into_raw_parts(all);
    Ok(RustExtColumnList {
        items,
        count,
        next_page_token: RustExtString::default(),
    })
}

pub(crate) fn load_catalog(config: RustExtClientConfig) -> Result<RustExtCatalog, String> {
    let sdk = SdkClient::new(config)?;
    let doc_id = config.resource.as_str().to_string();
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
        let tables = vec_from_raw_parts(page.items, page.count);
        for table in tables {
            let columns = load_columns(
                &sdk,
                &doc_id,
                table.id.as_str(),
                config.include_system_columns,
            )?;
            catalog_tables.push(RustExtCatalogTable {
                id: alloc_string(table.id.as_str()),
                name: alloc_string(table.name.as_str()),
                capabilities: table.capabilities,
                columns: columns.items,
                column_count: columns.count,
            });
            table.id.free();
            table.name.free();
        }
        page_token = page.next_page_token.as_str().to_string();
        page.next_page_token.free();
        if page_token.is_empty() {
            break;
        }
    }
    let (tables, table_count) = vec_into_raw_parts(catalog_tables);
    Ok(RustExtCatalog {
        tables,
        table_count,
    })
}

fn list_rows(
    sdk: &SdkClient,
    doc_id: &str,
    table_id: &str,
    request: CodaRowsRequest,
) -> Result<CodaRowsResponse, String> {
    let sort_by = rows_sort_by(&request.sort_by)?;
    let body = sdk.execute(|client| {
        client.tables().rows().list(operations::ListRowsInput {
            doc_id: doc_id.to_string(),
            table_id_or_name: table_id.to_string(),
            query: non_empty_string(&request.query),
            sort_by,
            use_column_names: Some(false),
            value_format: Some(operations::ValueFormat::Rich),
            visible_only: Some(false),
            limit: Some(request.limit as i32),
            page_token: non_empty_string(&request.page_token),
            sync_token: non_empty_string(&request.sync_token),
        })
    })?;
    rows_from_json(&body)
}

fn rows_sort_by(value: &str) -> Result<Option<operations::RowsSortBy>, String> {
    match value {
        "" => Ok(None),
        "createdAt" => Ok(Some(operations::RowsSortBy::CreatedAt)),
        "natural" => Ok(Some(operations::RowsSortBy::Natural)),
        "updatedAt" => Ok(Some(operations::RowsSortBy::UpdatedAt)),
        _ => Err(format!("unsupported row sort order: {value}")),
    }
}

pub(crate) struct ScanHandle {
    sdk: SdkClient,
    doc_id: String,
    table_id: String,
    query: String,
    sort_by: String,
    limit: u64,
    next_page_token: String,
    next_sync_token: String,
    sync_check_done: bool,
    finished: bool,
}

impl ScanHandle {
    pub(crate) fn new(
        config: RustExtClientConfig,
        table_id: RustExtString,
        request: RustExtScanRequest,
    ) -> Result<Self, String> {
        Ok(Self {
            sdk: SdkClient::new(config)?,
            doc_id: config.resource.as_str().to_string(),
            table_id: table_id.as_str().to_string(),
            query: request.filter.as_str().to_string(),
            sort_by: request.order.as_str().to_string(),
            limit: if request.limit == 0 {
                500
            } else {
                request.limit.min(500)
            },
            next_page_token: String::new(),
            next_sync_token: String::new(),
            sync_check_done: false,
            finished: false,
        })
    }

    fn request(&mut self) -> CodaRowsRequest {
        let sync_token = if self.next_page_token.is_empty()
            && !self.next_sync_token.is_empty()
            && !self.sync_check_done
        {
            self.sync_check_done = true;
            self.next_sync_token.clone()
        } else {
            String::new()
        };
        CodaRowsRequest {
            page_token: self.next_page_token.clone(),
            query: self.query.clone(),
            sort_by: self.sort_by.clone(),
            sync_token,
            limit: self.limit,
        }
    }

    pub(crate) fn next_batch(&mut self) -> Result<RustExtScanBatch, String> {
        while !self.finished {
            let request = self.request();
            let response = list_rows(&self.sdk, &self.doc_id, &self.table_id, request)?;
            self.next_page_token = response.next_page_token.as_str().to_string();
            self.next_sync_token = response.next_sync_token.as_str().to_string();
            response.next_page_token.free();
            response.next_sync_token.free();
            if self.next_page_token.is_empty()
                && (self.next_sync_token.is_empty() || self.sync_check_done)
            {
                self.finished = true;
            }

            let rows = vec_from_raw_parts(response.rows, response.row_count);
            let mut visible_rows = Vec::with_capacity(rows.len());
            for row in rows {
                if row.deleted {
                    for cell in vec_from_raw_parts(row.cells, row.cell_count) {
                        cell.column_id.free();
                        cell.value.free();
                    }
                    row.id.free();
                    row.created_at.free();
                    row.updated_at.free();
                } else {
                    visible_rows.push(row);
                }
            }
            let (rows, row_count) = vec_into_raw_parts(visible_rows);
            if row_count > 0 || self.finished {
                return Ok(RustExtScanBatch {
                    rows,
                    row_count,
                    finished: self.finished,
                });
            }
        }
        Ok(RustExtScanBatch {
            finished: true,
            ..Default::default()
        })
    }
}
