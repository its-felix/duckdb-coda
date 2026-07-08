use crate::ffi::*;

pub(crate) fn scan_value(column: RustExtColumn, row: RustExtRow) -> RustExtScanValue {
    let column_id = column.id.as_str();
    if column.capabilities & RUST_EXT_COLUMN_SYSTEM != 0 {
        let value = if column_id.eq_ignore_ascii_case("createdAt") {
            row.created_at
        } else if column_id.eq_ignore_ascii_case("updatedAt") {
            row.updated_at
        } else {
            RustExtString::default()
        };
        if value.as_str().is_empty() {
            return RustExtScanValue::default();
        }
        return RustExtScanValue {
            is_null: false,
            value_type: 3,
            value,
            ..Default::default()
        };
    }
    if row.cells.is_null() {
        return RustExtScanValue::default();
    }
    let cells = slice_from_raw_parts(row.cells, row.cell_count);
    for cell in cells {
        if !cell.column_id.as_str().eq_ignore_ascii_case(column_id) {
            continue;
        }
        if cell.value_type == 0 || cell.value_type == 1 {
            return RustExtScanValue::default();
        }
        let value_text = cell.value.as_str();
        let double_value = if column.logical_type == 2 {
            value_text.parse::<f64>().ok()
        } else {
            None
        };
        return RustExtScanValue {
            is_null: false,
            value_type: cell.value_type,
            bool_value: cell.value_type == 2 && value_text == "true",
            has_double_value: double_value.is_some(),
            double_value: double_value.unwrap_or_default(),
            value: cell.value,
        };
    }
    RustExtScanValue::default()
}
