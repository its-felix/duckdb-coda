pub(crate) const EXTENSION_NAME: &[u8] = b"coda\0";
pub(crate) const EXTENSION_DESCRIPTION: &[u8] =
    b"DuckDB extension for reading and writing Coda docs\0";
pub(crate) const SECRET_TYPE: &[u8] = b"coda\0";
pub(crate) const SECRET_PROVIDER: &[u8] = b"config\0";
pub(crate) const SECRET_SCOPE_PREFIX: &str = "coda:";
pub(crate) const SECRET_SCOPE_PREFIX_C: &[u8] = b"coda:\0";
pub(crate) const TOKEN_OPTION: &[u8] = b"token\0";
pub(crate) const TOKEN_ENV_OPTION: &[u8] = b"token_env\0";
pub(crate) const API_BASE_OPTION: &[u8] = b"api_base\0";
pub(crate) const INCLUDE_ROW_METADATA_OPTION: &[u8] = b"include_row_metadata\0";
