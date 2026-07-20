use super::*;

mod api;
mod duckdb_cases;
mod wide;

use api::*;
use duckdb_cases::*;
use wide::*;

#[test]
#[ignore]
fn real_superhuman_docs_api_smoke() {
    let credential = required_env("SUPERHUMAN_DOCS_TEST_API_TOKEN");
    let endpoint = env::var("SUPERHUMAN_DOCS_TEST_API_BASE")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    let resource = required_env("SUPERHUMAN_DOCS_TEST_DOC_ID");

    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let page_name = format!("duckdb-superhuman-docs-test-{run_id}");
    let table_name = format!("duckdb_superhuman_docs_test_{run_id}");
    let page = create_test_page(&endpoint, &credential, &resource, &page_name, &table_name)
        .expect("failed to create Superhuman Docs test page");
    let page_id = page
        .get("id")
        .and_then(Value::as_str)
        .expect("create page response did not include id")
        .to_string();
    let cleanup = PageCleanup {
        endpoint: endpoint.clone(),
        credential: credential.clone(),
        resource: resource.clone(),
        page_id: page_id.clone(),
    };

    let discovered_table =
        wait_for_page_table(&endpoint, &credential, &resource, &page_id, &table_name)
            .expect("timed out waiting for generated Superhuman Docs table");
    let table_id = discovered_table
        .get("id")
        .and_then(Value::as_str)
        .expect("generated table did not include id");
    assert_required_columns(&endpoint, &credential, &resource, table_id)
        .expect("generated table is missing expected columns");
    let actual_table_name = discovered_table
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&table_name);

    run_duckdb_success_case(&resource, &credential, &endpoint, actual_table_name);
    run_duckdb_metadata_case(&resource, &credential, &endpoint, actual_table_name);
    drop(cleanup);
}

#[test]
#[ignore]
fn real_superhuman_docs_api_wide_types() {
    let credential = required_env("SUPERHUMAN_DOCS_TEST_API_TOKEN");
    let endpoint = env::var("SUPERHUMAN_DOCS_TEST_API_BASE")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    let resource = required_env("SUPERHUMAN_DOCS_TEST_DOC_ID");

    run_duckdb_real_wide_types_schema_case(
        &resource,
        &credential,
        &endpoint,
        REAL_WIDE_TYPES_TABLE,
    );
    run_duckdb_real_wide_types_dml_case(&resource, &credential, &endpoint);
}

#[test]
#[ignore]
fn real_superhuman_docs_api_wide_types_fixture_select_only() {
    let credential = required_env("SUPERHUMAN_DOCS_TEST_API_TOKEN");
    let endpoint = env::var("SUPERHUMAN_DOCS_TEST_API_BASE")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    let resource = required_env("SUPERHUMAN_DOCS_TEST_DOC_ID");

    run_duckdb_real_wide_types_schema_case(
        &resource,
        &credential,
        &endpoint,
        REAL_WIDE_TYPES_FIXTURE_TABLE,
    );
    run_duckdb_real_wide_types_fixture_select_case(&resource, &credential, &endpoint);
}
