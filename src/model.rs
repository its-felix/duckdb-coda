use serde_json::Value;

use crate::ffi::*;

pub(crate) struct SuperhumanDocsClientConfig {
    pub(crate) resource: String,
    pub(crate) credential: String,
    pub(crate) endpoint: String,
    pub(crate) include_system_columns: bool,
    pub(crate) wait_for_mutations: bool,
    pub(crate) mutation_timeout_seconds: u64,
    pub(crate) allow_mutation_warnings: bool,
}

pub(crate) struct SuperhumanDocsTable {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) capabilities: u32,
}

pub(crate) struct SuperhumanDocsColumn {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) format_type: String,
    pub(crate) duckdb_type: String,
    pub(crate) duckdb_type_alias: String,
    pub(crate) is_array: bool,
    pub(crate) capabilities: u32,
}

pub(crate) struct SuperhumanDocsCell {
    pub(crate) column_id: String,
    pub(crate) value: Value,
}

pub(crate) struct SuperhumanDocsRow {
    pub(crate) id: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) deleted: bool,
    pub(crate) cells: Vec<SuperhumanDocsCell>,
}

pub(crate) struct SuperhumanDocsPage<T> {
    pub(crate) items: Vec<T>,
    pub(crate) next_page_token: String,
}

pub(crate) struct SuperhumanDocsRowsResponse {
    pub(crate) rows: Vec<SuperhumanDocsRow>,
    pub(crate) next_page_token: String,
    pub(crate) next_sync_token: String,
}

pub(crate) struct SuperhumanDocsRowsRequest {
    pub(crate) page_token: String,
    pub(crate) query: String,
    pub(crate) sort_by: String,
    pub(crate) sync_token: String,
    pub(crate) limit: u64,
}

pub(crate) fn client_config(
    config: RustExtClientConfig,
) -> Result<&'static SuperhumanDocsClientConfig, String> {
    ref_from_raw(
        config.handle.cast::<SuperhumanDocsClientConfig>(),
        "client config",
    )
}

pub(crate) fn table_from_handle(
    handle: *const std::ffi::c_void,
) -> Result<&'static SuperhumanDocsTable, String> {
    ref_from_raw(handle.cast::<SuperhumanDocsTable>(), "table")
}

pub(crate) fn column_from_handle(
    handle: *const std::ffi::c_void,
) -> Result<&'static SuperhumanDocsColumn, String> {
    ref_from_raw(handle.cast::<SuperhumanDocsColumn>(), "column")
}

pub(crate) fn row_from_handle(
    handle: *const std::ffi::c_void,
) -> Result<&'static SuperhumanDocsRow, String> {
    ref_from_raw(handle.cast::<SuperhumanDocsRow>(), "row")
}
