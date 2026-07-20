use super::*;

#[test]
fn scan_sort_by_returns_owned_string() {
    let column = Box::into_raw(Box::new(SuperhumanDocsColumn {
        id: "createdAt".to_string(),
        name: "createdAt".to_string(),
        format_type: "datetime".to_string(),
        duckdb_type: "TIMESTAMPTZ".to_string(),
        duckdb_type_alias: String::new(),
        is_array: false,
        capabilities: RUST_EXT_COLUMN_SORT_ASC,
    }));
    let mut sort_by = RustExtString::default();
    assert!(crate::exports::rust_ext_scan_sort_by(
        column.cast(),
        &mut sort_by
    ));
    assert_eq!(sort_by.as_str(), "createdAt");
    assert_ne!(
        sort_by.ptr,
        unsafe { &*column }.id.as_ptr().cast_mut().cast()
    );
    sort_by.free();
    drop(unsafe { Box::from_raw(column) });
}

#[test]
fn scan_unwraps_superhuman_docs_rich_text_fences() {
    let column = SuperhumanDocsColumn {
        id: "c1".to_string(),
        name: "Name".to_string(),
        format_type: "text".to_string(),
        duckdb_type: "VARCHAR".to_string(),
        duckdb_type_alias: String::new(),
        is_array: false,
        capabilities: 0,
    };
    let row = SuperhumanDocsRow {
        id: "r1".to_string(),
        created_at: String::new(),
        updated_at: String::new(),
        deleted: false,
        cells: vec![SuperhumanDocsCell {
            column_id: "c1".to_string(),
            value: Value::String("```Alpha```".to_string()),
        }],
    };
    let value = scan_value(&column, &row);
    assert_eq!(value.value.as_str(), "Alpha");
    crate::exports::rust_ext_free_scan_value(value);
}

#[test]
fn scan_normalizes_superhuman_docs_rich_percentage() {
    let column = SuperhumanDocsColumn {
        id: "c1".to_string(),
        name: "Percent".to_string(),
        format_type: "percent".to_string(),
        duckdb_type: "DECIMAL(38,20)".to_string(),
        duckdb_type_alias: String::new(),
        is_array: false,
        capabilities: 0,
    };
    for (raw, expected) in [
        (json!(0.125), "0.125"),
        (json!("25\u{a0}%"), "0.25"),
        (json!("80\u{202f}%"), "0.8"),
    ] {
        let row = SuperhumanDocsRow {
            id: "r1".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            deleted: false,
            cells: vec![SuperhumanDocsCell {
                column_id: "c1".to_string(),
                value: raw,
            }],
        };
        let value = scan_value(&column, &row);
        assert_eq!(value.value.as_str(), expected);
        crate::exports::rust_ext_free_scan_value(value);
    }
}
