pub(crate) const EXTENSION_NAME: &[u8] = b"superhuman_docs\0";
pub(crate) const EXTENSION_DESCRIPTION: &[u8] =
    b"DuckDB extension for reading and writing Superhuman Docs documents\0";
pub(crate) const SECRET_TYPE: &[u8] = b"superhuman_docs\0";
pub(crate) const SECRET_PROVIDER: &[u8] = b"config\0";
pub(crate) const SECRET_SCOPE_PREFIX: &str = "superhuman_docs:";
pub(crate) const ATTACH_RESOURCE_PREFIXES: [&str; 4] = [
    "coda:",
    "superhuman:",
    "superhuman-docs:",
    SECRET_SCOPE_PREFIX,
];
pub(crate) const TOKEN_OPTION: &[u8] = b"token\0";
pub(crate) const TOKEN_ENV_OPTION: &[u8] = b"token_env\0";
pub(crate) const API_BASE_OPTION: &[u8] = b"api_base\0";
pub(crate) const INCLUDE_ROW_METADATA_OPTION: &[u8] = b"include_row_metadata\0";
pub(crate) const WAIT_FOR_MUTATIONS_OPTION: &[u8] = b"wait_for_mutations\0";
pub(crate) const MUTATION_TIMEOUT_SECONDS_OPTION: &[u8] = b"mutation_timeout_seconds\0";
pub(crate) const ALLOW_MUTATION_WARNINGS_OPTION: &[u8] = b"allow_mutation_warnings\0";
