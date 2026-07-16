#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
	char *ptr;
	size_t len;
} RustExtString;

typedef struct {
	RustExtString message;
} RustExtError;

typedef enum {
	RUST_EXT_LOGICAL_VARCHAR = 0,
	RUST_EXT_LOGICAL_BOOLEAN = 1,
	RUST_EXT_LOGICAL_DOUBLE = 2,
	RUST_EXT_LOGICAL_TIMESTAMP_TZ = 3
} RustExtLogicalType;

typedef enum {
	RUST_EXT_JSON_INVALID = 0,
	RUST_EXT_JSON_NULL = 1,
	RUST_EXT_JSON_BOOLEAN = 2,
	RUST_EXT_JSON_STRING = 3,
	RUST_EXT_JSON_OTHER = 4
} RustExtJSONValueType;

typedef enum {
	RUST_EXT_INPUT_NULL = 0,
	RUST_EXT_INPUT_BOOL = 1,
	RUST_EXT_INPUT_INT = 2,
	RUST_EXT_INPUT_UINT = 3,
	RUST_EXT_INPUT_DOUBLE = 4,
	RUST_EXT_INPUT_STRING = 5
} RustExtInputValueType;

typedef enum {
	RUST_EXT_COLUMN_GENERATED = 1 << 0,
	RUST_EXT_COLUMN_SYSTEM = 1 << 1,
	RUST_EXT_COLUMN_EDITABLE = 1 << 2,
	RUST_EXT_COLUMN_FILTER_EQUALITY = 1 << 3,
	RUST_EXT_COLUMN_SORT_ASC = 1 << 4,
	RUST_EXT_COLUMN_ARRAY = 1 << 5
} RustExtColumnCapability;

typedef enum {
	RUST_EXT_TABLE_VIEW = 1 << 0,
	RUST_EXT_TABLE_INSERT = 1 << 1,
	RUST_EXT_TABLE_UPDATE = 1 << 2,
	RUST_EXT_TABLE_DELETE = 1 << 3,
	RUST_EXT_TABLE_ROW_ID = 1 << 4
} RustExtTableCapability;

typedef enum {
	RUST_EXT_HTTP_GET = 0,
	RUST_EXT_HTTP_POST = 1,
	RUST_EXT_HTTP_PUT = 2,
	RUST_EXT_HTTP_DELETE = 3
} RustExtHttpMethod;

typedef enum { RUST_EXT_DML_INSERT = 0, RUST_EXT_DML_UPDATE = 1, RUST_EXT_DML_DELETE = 2 } RustExtDmlOperation;

typedef enum {
	RUST_EXT_DDL_CREATE_SCHEMA = 0,
	RUST_EXT_DDL_DROP_SCHEMA = 1,
	RUST_EXT_DDL_CREATE_TABLE_AS = 2,
	RUST_EXT_DDL_CREATE_INDEX = 3,
	RUST_EXT_DDL_CREATE_FUNCTION = 4,
	RUST_EXT_DDL_CREATE_TABLE = 5,
	RUST_EXT_DDL_CREATE_VIEW = 6,
	RUST_EXT_DDL_CREATE_SEQUENCE = 7,
	RUST_EXT_DDL_CREATE_TABLE_FUNCTION = 8,
	RUST_EXT_DDL_CREATE_COPY_FUNCTION = 9,
	RUST_EXT_DDL_CREATE_PRAGMA_FUNCTION = 10,
	RUST_EXT_DDL_CREATE_COLLATION = 11,
	RUST_EXT_DDL_CREATE_TYPE = 12,
	RUST_EXT_DDL_ALTER = 13
} RustExtDdlOperation;

typedef struct {
	uint8_t value_type;
	bool bool_value;
	int64_t int_value;
	uint64_t uint_value;
	double double_value;
	RustExtString string_value;
} RustExtInputValue;

typedef struct {
	RustExtString id;
	uint32_t capabilities;
} RustExtWriteColumn;

typedef struct {
	RustExtString id;
	RustExtString name;
	uint32_t capabilities;
} RustExtTable;

typedef struct {
	RustExtTable *items;
	size_t count;
	RustExtString next_page_token;
} RustExtTableList;

typedef struct {
	RustExtString id;
	RustExtString name;
	RustExtString type_name;
	uint32_t capabilities;
	int32_t logical_type;
} RustExtColumn;

typedef struct {
	RustExtColumn *items;
	size_t count;
	RustExtString next_page_token;
} RustExtColumnList;

typedef struct {
	RustExtString column_id;
	uint8_t value_type;
	RustExtString value;
} RustExtCell;

typedef struct {
	RustExtString id;
	RustExtString created_at;
	RustExtString updated_at;
	bool deleted;
	RustExtCell *cells;
	size_t cell_count;
} RustExtRow;

typedef struct {
	RustExtRow *rows;
	size_t row_count;
	bool finished;
} RustExtScanBatch;

typedef struct {
	bool is_null;
	uint8_t value_type;
	bool bool_value;
	bool has_double_value;
	double double_value;
	RustExtString value;
} RustExtScanValue;

typedef struct {
	RustExtString id;
	RustExtString name;
	uint32_t capabilities;
	RustExtColumn *columns;
	size_t column_count;
} RustExtCatalogTable;

typedef struct {
	RustExtCatalogTable *tables;
	size_t table_count;
} RustExtCatalog;

typedef struct {
	RustExtString resource;
	RustExtString credential;
	RustExtString endpoint;
	bool include_system_columns;
} RustExtClientConfig;

typedef struct {
	RustExtString resource;
	RustExtString credential;
	RustExtString endpoint;
	RustExtString primary_secret_scope;
	RustExtString fallback_secret_scope;
	bool include_system_columns;
} RustExtAttachConfig;

typedef struct {
	RustExtString filter;
	RustExtString order;
	uint64_t limit;
} RustExtScanRequest;

typedef struct {
	bool (*set_description)(void *loader, const char *description, RustExtError *err);
	bool (*register_config_secret)(void *loader, const char *secret_type, const char *provider, const char *extension,
	                               const char *default_scope, const char *secret_key, const char *secret_env_key,
	                               RustExtError *err);
	bool (*register_storage_extension)(void *loader, const char *extension_name, RustExtError *err);
} RustExtDuckDbHost;

typedef struct {
	bool (*get_option)(void *userdata, const char *name, RustExtString *out, RustExtError *err);
	bool (*lookup_secret)(void *userdata, RustExtString scope, const char *secret_type, const char *secret_key,
	                      RustExtString *out, RustExtError *err);
} RustExtAttachHost;

bool rust_ext_secret_config_missing_message(const char *secret_type_ptr, size_t secret_type_len, RustExtString *out,
                                            RustExtError *err);
bool rust_ext_secret_unknown_parameter_message(const char *secret_type_ptr, size_t secret_type_len,
                                               const char *parameter_ptr, size_t parameter_len, RustExtString *out,
                                               RustExtError *err);
bool rust_ext_secret_canonical_parameter_name(const char *secret_key_ptr, size_t secret_key_len,
                                              const char *parameter_ptr, size_t parameter_len, RustExtString *out,
                                              RustExtError *err);
bool rust_ext_validate_secret_token(const char *token_ptr, size_t token_len, RustExtError *err);
bool rust_ext_read_environment_variable(const char *name_ptr, size_t name_len, RustExtString *out, RustExtError *err);
const char *rust_ext_extension_name(void);
const char *rust_ext_unsupported_update_expression_message(void);
const char *rust_ext_row_id_column_name(void);
const char *rust_ext_insert_operator_name(void);
const char *rust_ext_update_operator_name(void);
const char *rust_ext_delete_operator_name(void);
const char *rust_ext_dml_not_supported_message(uint8_t operation);
const char *rust_ext_ddl_not_supported_message(uint8_t operation);
bool rust_ext_ddl_drop_entry_not_supported_message(const char *name_ptr, size_t name_len, RustExtString *out,
                                                   RustExtError *err);
const char *rust_ext_database_size_not_available_message(void);
const char *rust_ext_returning_not_supported_message(uint8_t operation);
const char *rust_ext_explicit_transaction_not_supported_message(void);
const char *rust_ext_transaction_rollback_not_supported_message(void);
bool rust_ext_supports_explicit_transactions(void);
bool rust_ext_supports_transaction_rollback(void);
bool rust_ext_extension_load(const RustExtDuckDbHost *host, void *loader, RustExtError *err);
bool rust_ext_resolve_attach(RustExtString path, const RustExtAttachHost *host, void *userdata,
                             RustExtAttachConfig *out, RustExtError *err);
bool rust_ext_build_equality_query(const char *column_id_ptr, size_t column_id_len, const char *column_name_ptr,
                                   size_t column_name_len, RustExtInputValue value, RustExtString *out_query,
                                   RustExtString *out_description, RustExtError *err);
bool rust_ext_scan_value(RustExtColumn column, RustExtRow row, RustExtScanValue *out);
bool rust_ext_scan_sort_by(RustExtColumn column, RustExtString *out);
bool rust_ext_scan_can_filter_equality(RustExtColumn column);
const char *rust_ext_scan_function_name(void);
const char *rust_ext_scan_query_label(void);
const char *rust_ext_scan_sort_label(void);
const char *rust_ext_scan_limit_label(void);
const char *rust_ext_scan_column_index_out_of_range_message(void);
bool rust_ext_scan_open(RustExtClientConfig config, RustExtString table_id, RustExtScanRequest request,
                        void **out, RustExtError *err);
bool rust_ext_scan_next(void *scan, RustExtScanBatch *out, RustExtError *err);
void rust_ext_scan_close(void *scan);

bool rust_ext_client_load_catalog(RustExtClientConfig config, RustExtCatalog *out, RustExtError *err);
bool rust_ext_client_insert_rows(RustExtClientConfig config, RustExtString table_id,
                                 const RustExtWriteColumn *columns, size_t column_count,
                                 const RustExtInputValue *values, size_t row_count, size_t value_column_count,
                                 uint32_t table_capabilities, size_t *affected_count, RustExtError *err);
bool rust_ext_client_update_rows(RustExtClientConfig config, RustExtString table_id, const RustExtString *row_ids,
                                 size_t row_count, const RustExtWriteColumn *columns, size_t column_count,
                                 const RustExtInputValue *values, uint32_t table_capabilities, size_t *affected_count,
                                 RustExtError *err);
bool rust_ext_client_delete_rows(RustExtClientConfig config, RustExtString table_id, const RustExtString *row_ids,
                                 size_t count, size_t *affected_count, RustExtError *err);
bool rust_ext_alloc_string(const char *ptr, size_t len, RustExtString *out, RustExtError *err);

void rust_ext_free_string(RustExtString value);
void rust_ext_free_error(RustExtError err);
void rust_ext_free_scan_batch(RustExtScanBatch batch);
void rust_ext_free_catalog(RustExtCatalog catalog);
void rust_ext_free_attach_config(RustExtAttachConfig config);

#ifdef __cplusplus
}
#endif
