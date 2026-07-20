use super::*;

pub(super) const REAL_WIDE_TYPES_TABLE: &str = "duckdb_coda_wide_types";
pub(super) const REAL_WIDE_TYPES_FIXTURE_TABLE: &str = "duckdb_coda_wide_types_fixture";
pub(super) const REAL_WIDE_TYPES_COLUMNS: &[(&str, &str)] = &[
    ("col_text", "VARCHAR"),
    ("col_number", "DECIMAL(38,20)"),
    ("col_percentage", "DECIMAL(38,20)"),
    (
        "col_currency",
        "STRUCT(currency VARCHAR, amount DECIMAL(38,20))",
    ),
    ("col_slider", "DECIMAL(38,20)"),
    ("col_progress", "DECIMAL(38,20)"),
    ("col_scale", "DECIMAL(38,20)"),
    ("col_date", "DATE"),
    ("col_time", "TIME"),
    ("col_datetime", "TIMESTAMP WITH TIME ZONE"),
    ("col_duration", "INTERVAL"),
    ("col_canvas", "JSON"),
    ("col_checkbox", "BOOLEAN"),
    ("col_people", "STRUCT(\"name\" VARCHAR, email VARCHAR)"),
    ("col_link", "STRUCT(\"name\" VARCHAR, url VARCHAR)"),
    ("col_select", "VARCHAR"),
    ("col_reaction", "JSON[]"),
    (
        "col_relation",
        "STRUCT(\"name\" VARCHAR, url VARCHAR, tableId VARCHAR, tableUrl VARCHAR, rowId VARCHAR)",
    ),
    (
        "col_reference",
        "STRUCT(\"name\" VARCHAR, url VARCHAR, tableId VARCHAR, tableUrl VARCHAR, rowId VARCHAR)",
    ),
    ("col_button", "JSON"),
    (
        "col_image",
        "STRUCT(\"name\" VARCHAR, url VARCHAR, height DOUBLE, width DOUBLE, status VARCHAR)[]",
    ),
    (
        "col_image_url",
        "STRUCT(\"name\" VARCHAR, url VARCHAR, height DOUBLE, width DOUBLE, status VARCHAR)",
    ),
    ("col_file", "JSON[]"),
    (
        "col_virtual_createdby",
        "STRUCT(\"name\" VARCHAR, email VARCHAR)",
    ),
    ("col_toggle", "BOOLEAN"),
    ("col_email", "VARCHAR"),
];

fn superhuman_docs_attach_sql(resource: &str, credential: &str, endpoint: &str) -> String {
    format!(
        "LOAD {}; ATTACH {} AS superhuman_docs_doc (TYPE superhuman_docs, TOKEN {}, API_BASE {}, WAIT_FOR_MUTATIONS true);",
        sql_literal(extension_path()),
        sql_literal(resource),
        sql_literal(credential),
        sql_literal(endpoint),
    )
}

pub(super) fn run_duckdb_real_wide_types_schema_case(
    resource: &str,
    credential: &str,
    endpoint: &str,
    table_name: &str,
) {
    let expected = REAL_WIDE_TYPES_COLUMNS
        .iter()
        .map(|(name, data_type)| {
            // The fixture enables multiple selections for its reference column.
            let data_type =
                if table_name == REAL_WIDE_TYPES_FIXTURE_TABLE && *name == "col_reference" {
                    format!("{data_type}[]")
                } else {
                    data_type.to_string()
                };
            format!("({}, {})", sql_literal(name), sql_literal(&data_type))
        })
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"{}
        WITH expected(column_name, data_type) AS (VALUES {expected}),
             actual AS (
                 SELECT column_name, data_type FROM information_schema.columns
                 WHERE table_catalog = 'superhuman_docs_doc' AND table_schema = 'main' AND table_name = {}
             )
        SELECT count(*) = {} AND (SELECT count(*) FROM actual) = {}
               AND bool_and(actual.data_type = expected.data_type) AS schema_ok
        FROM expected JOIN actual USING (column_name);"#,
        superhuman_docs_attach_sql(resource, credential, endpoint),
        sql_literal(table_name),
        REAL_WIDE_TYPES_COLUMNS.len(),
        REAL_WIDE_TYPES_COLUMNS.len(),
    );
    let output = run_duckdb(&sql);
    assert!(
        output.contains("schema_ok\ntrue"),
        "expected {table_name} to match the captured wide-type schema, got:\n{output}"
    );
}

struct WideRowCleanup {
    resource: String,
    credential: String,
    endpoint: String,
    row_name: String,
    active: bool,
}

impl WideRowCleanup {
    fn sql(&self) -> String {
        format!(
            "{} DELETE FROM superhuman_docs_doc.main.{} WHERE col_text = {};",
            superhuman_docs_attach_sql(&self.resource, &self.credential, &self.endpoint),
            sql_ident(REAL_WIDE_TYPES_TABLE),
            sql_literal(&self.row_name),
        )
    }

    fn delete(&mut self) {
        run_duckdb(&self.sql());
        self.active = false;
    }
}

impl Drop for WideRowCleanup {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let _ = Command::new("build/release/duckdb")
            .env("TZ", "UTC")
            .args(["-batch", "-csv", ":memory:", "-c", &self.sql()])
            .output();
    }
}

pub(super) fn run_duckdb_real_wide_types_dml_case(
    resource: &str,
    credential: &str,
    endpoint: &str,
) {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let row_name = format!("duckdb-superhuman-docs-wide-{run_id}");
    let table = format!(
        "superhuman_docs_doc.main.{}",
        sql_ident(REAL_WIDE_TYPES_TABLE)
    );
    let mut cleanup = WideRowCleanup {
        resource: resource.to_string(),
        credential: credential.to_string(),
        endpoint: endpoint.to_string(),
        row_name: row_name.clone(),
        active: true,
    };
    let insert_sql = format!(
        "{} INSERT INTO {table} (\
             col_text, col_number, col_percentage, col_slider, col_progress, col_scale,\
             col_date, col_time, col_datetime, col_duration, col_canvas, col_checkbox, col_select,\
             col_toggle, col_email\
         ) VALUES (\
             {}, 123.5, 0.375, 11, 0.4, 4, DATE '2026-07-17', TIME '12:34:56',\
             TIMESTAMPTZ '2026-07-17 12:34:56+00', INTERVAL '1 hour 2 minutes 3 seconds', 'wide canvas',\
             true, 'Done', false, 'probe@example.com'\
         );",
        superhuman_docs_attach_sql(resource, credential, endpoint),
        sql_literal(&row_name),
    );
    run_duckdb(&insert_sql);

    let select_sql = format!(
        r#"{} SELECT col_text, col_number, col_percentage, col_slider, col_progress, col_scale,
             col_date, col_time, col_datetime, col_duration,
             col_canvas::VARCHAR LIKE '%wide canvas%' AS canvas_ok, col_checkbox, col_select,
             col_toggle, col_email
         FROM {table} WHERE col_text = {};"#,
        superhuman_docs_attach_sql(resource, credential, endpoint),
        sql_literal(&row_name),
    );
    let mut output = String::new();
    for _ in 0..20 {
        output = run_duckdb(&select_sql);
        if output.contains(&row_name) {
            break;
        }
        thread::sleep(Duration::from_secs(3));
    }
    cleanup.delete();

    for expected in [
        row_name.as_str(),
        "123.50000000000000000000,0.37500000000000000000,11.00000000000000000000,0.40000000000000000000,4.00000000000000000000",
        "2026-07-17,12:34:56,2026-07-17 12:34:56+00,01:02:03,true,true,Done,false,probe@example.com",
    ] {
        assert!(
            output.contains(expected),
            "expected mutable wide-type row output to contain '{expected}', got:\n{output}"
        );
    }
}

pub(super) fn run_duckdb_real_wide_types_fixture_select_case(
    resource: &str,
    credential: &str,
    endpoint: &str,
) {
    let table = format!(
        "superhuman_docs_doc.main.{}",
        sql_ident(REAL_WIDE_TYPES_FIXTURE_TABLE)
    );
    let sql = format!(
        r#"{}
         SELECT count(*) = 10 AS row_count_ok FROM {table};
         SELECT col_text, col_number, col_slider, col_progress, col_scale
         FROM {table} WHERE col_text IN ('Alice Johnson', 'James O''Brien') ORDER BY col_text;
         SELECT col_text, col_percentage, col_currency.currency, col_currency.amount, col_date,
                col_time, col_datetime, col_duration, col_checkbox, col_people.name AS person_name,
                col_link.url AS link_url, col_select, col_relation.rowId AS relation_row_id,
                col_image_url.url AS image_url, col_virtual_createdby.email AS created_by,
                col_toggle, col_email
         FROM {table} WHERE col_text IN ('Alice Johnson', 'James O''Brien') ORDER BY col_text;
         SELECT col_text, col_canvas::VARCHAR AS canvas_json,
                list_count(col_reaction) AS reaction_count,
                list_count(col_reference) AS reference_count,
                col_button::VARCHAR AS button_json,
                col_image IS NULL AS image_null, col_file IS NULL AS file_null
         FROM {table} WHERE col_text IN ('Alice Johnson', 'James O''Brien') ORDER BY col_text;"#,
        superhuman_docs_attach_sql(resource, credential, endpoint),
    );
    let output = run_duckdb(&sql);
    for expected in [
        "row_count_ok\ntrue",
        "Alice Johnson,42.00000000000000000000,3.00000000000000000000,0.10000000000000000000,3.00000000000000000000",
        "\"James O'Brien\",12.50000000000000000000,10.00000000000000000000,1.00000000000000000000,2.00000000000000000000",
        "Alice Johnson,0.25000000000000000000,EUR,1200.50000000000000000000,2024-01-15,09:00:00,2024-01-15 08:00:00+00,00:01:00,true,NULL,https://example.com/project-alpha,Not started,i-UcMngti-L2,https://picsum.photos/seed/row1/200/150,felix.testuser@outlook.com,false,alice@example.com",
        "\"James O'Brien\",0.55000000000000000000,EUR,44.99000000000000000000,2025-12-25,23:00:00,2025-12-25 22:00:00+00,00:00:07,false,Felix Testuser,https://example.com/final-report,Done,NULL,https://picsum.photos/seed/row10/200/150,felix.testuser@outlook.com,false,james@example.com",
        "Alice Johnson,\"\"\"**Project kickoff** meeting notes. Action items TBD.\"\"\",1,0,\"\"\"\"\"\",true,true",
        "\"James O'Brien\",\"\"\"Final report submitted. *Project closed.*\"\"\",1,0,\"\"\"\"\"\",true,true",
    ] {
        assert!(
            output.contains(expected),
            "expected immutable wide-type fixture output to contain '{expected}', got:\n{output}"
        );
    }
}
