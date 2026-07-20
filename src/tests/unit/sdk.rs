use super::*;

#[test]
fn token_validation_uses_whoami_status() {
    let _network_guard = NETWORK_UNIT_TEST_LOCK.lock().unwrap();
    let server = MockSuperhumanDocsServer::start();
    validate_token_at(&server.base_url(), "mock-token").unwrap();
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/whoami");
    assert!(
        requests[0]
            .headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case("Authorization: Bearer mock-token")),
        "expected bearer token in request headers: {}",
        requests[0].headers
    );
    drop(server);

    let server = MockSuperhumanDocsServer::start_with_whoami_status("401 Unauthorized");
    let error = validate_token_at(&server.base_url(), "bad-token").unwrap_err();
    assert_eq!(
        error,
        "Whoami returned HTTP 401, expected 200: not valid JSON"
    );
}

#[test]
fn token_environment_variable_is_read_eagerly() {
    let name = format!(
        "DUCKDB_SUPERHUMAN_DOCS_TOKEN_ENV_TEST_{}",
        std::process::id()
    );
    env::set_var(&name, "resolved-token");
    assert_eq!(read_environment_variable(&name).unwrap(), "resolved-token");
    env::remove_var(&name);
    assert!(read_environment_variable(&name).is_err());
}
