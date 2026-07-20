use std::ffi::c_void;

use crate::ffi::*;
use crate::model::{SuperhumanDocsColumn, SuperhumanDocsRow, SuperhumanDocsTable};

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
