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
	RUST_EXT_INPUT_NULL = 0,
	RUST_EXT_INPUT_BOOL = 1,
	RUST_EXT_INPUT_INT = 2,
	RUST_EXT_INPUT_UINT = 3,
	RUST_EXT_INPUT_DOUBLE = 4,
	RUST_EXT_INPUT_STRING = 5,
	RUST_EXT_INPUT_JSON = 6
} RustExtInputValueType;

typedef enum {
	RUST_EXT_COLUMN_GENERATED = 1 << 0,
	RUST_EXT_COLUMN_SYSTEM = 1 << 1,
	RUST_EXT_COLUMN_EDITABLE = 1 << 2,
	RUST_EXT_COLUMN_FILTER_EQUALITY = 1 << 3,
	RUST_EXT_COLUMN_SORT_ASC = 1 << 4
} RustExtColumnCapability;

typedef enum {
	RUST_EXT_TABLE_VIEW = 1 << 0,
	RUST_EXT_TABLE_INSERT = 1 << 1,
	RUST_EXT_TABLE_UPDATE = 1 << 2,
	RUST_EXT_TABLE_DELETE = 1 << 3,
	RUST_EXT_TABLE_ROW_ID = 1 << 4
} RustExtTableCapability;

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
	RustExtString name;
	RustExtInputValue value;
} RustExtNamedValue;

typedef struct {
	RustExtString name;
	RustExtString logical_type;
} RustExtSecretParameter;

typedef struct {
	RustExtString secret_type;
	RustExtString provider;
	RustExtString extension;
	const RustExtSecretParameter *parameters;
	size_t parameter_count;
} RustExtSecretRegistration;

typedef struct {
	RustExtString secret_type;
	RustExtString provider;
	RustExtString name;
	const RustExtString *scope;
	size_t scope_count;
	const RustExtNamedValue *options;
	size_t option_count;
} RustExtSecretCreateInput;

typedef struct {
	RustExtString *scope;
	size_t scope_count;
	RustExtNamedValue *entries;
	size_t entry_count;
	RustExtString *redact_keys;
	size_t redact_key_count;
} RustExtSecretCreateResult;

typedef struct {
	void *handle;
	uint32_t capabilities;
} RustExtWriteColumn;

typedef struct {
	void *handle;
	RustExtString name;
	RustExtString logical_type;
	RustExtString value_type_alias;
	uint32_t capabilities;
} RustExtColumn;

typedef struct {
	void *handle;
	RustExtString row_id;
} RustExtScanRow;

typedef struct {
	RustExtScanRow *rows;
	size_t row_count;
	bool finished;
} RustExtScanBatch;

typedef struct {
	bool is_null;
	RustExtString value;
} RustExtArrayValue;

typedef struct {
	bool is_null;
	bool value_owned;
	RustExtString value;
	RustExtArrayValue *array_values;
	size_t array_count;
} RustExtScanValue;

typedef struct {
	void *handle;
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
	void *handle;
} RustExtClientConfig;

typedef struct {
	void *handle;
	RustExtString database_name;
} RustExtAttachConfig;

typedef struct {
	RustExtString filter;
	RustExtString order;
	uint64_t limit;
} RustExtScanRequest;

typedef struct {
	bool (*set_description)(void *loader, const char *description, RustExtError *err);
	bool (*register_secret)(void *loader, RustExtSecretRegistration registration, RustExtError *err);
	bool (*register_storage_extension)(void *loader, const char *extension_name, RustExtError *err);
} RustExtDuckDbHost;

typedef struct {
	bool (*get_option)(void *userdata, const char *name, RustExtString *out, RustExtError *err);
	bool (*lookup_secret)(void *userdata, RustExtString scope, const char *secret_type, const char *secret_key,
	                      RustExtString *out, RustExtError *err);
} RustExtAttachHost;

bool rust_ext_create_secret(RustExtSecretCreateInput input, RustExtSecretCreateResult *out, RustExtError *err);
void rust_ext_free_secret(RustExtSecretCreateResult secret);
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
bool rust_ext_build_equality_query(void *column, RustExtInputValue value, RustExtString *out_query,
                                   RustExtString *out_description, RustExtError *err);
bool rust_ext_scan_value(void *column, void *row, RustExtScanValue *out);
void rust_ext_free_scan_value(RustExtScanValue value);
bool rust_ext_scan_sort_by(void *column, RustExtString *out);
bool rust_ext_scan_can_filter_equality(void *column);
const char *rust_ext_scan_function_name(void);
const char *rust_ext_scan_query_label(void);
const char *rust_ext_scan_sort_label(void);
const char *rust_ext_scan_limit_label(void);
const char *rust_ext_scan_column_index_out_of_range_message(void);
bool rust_ext_scan_open(RustExtClientConfig config, void *table, RustExtScanRequest request, void **out,
                        RustExtError *err);
bool rust_ext_scan_next(void *scan, RustExtScanBatch *out, RustExtError *err);
void rust_ext_scan_close(void *scan);

bool rust_ext_client_load_catalog(RustExtClientConfig config, RustExtCatalog *out, RustExtError *err);
bool rust_ext_client_insert_rows(RustExtClientConfig config, void *table, const RustExtWriteColumn *columns,
                                 size_t column_count, const RustExtInputValue *values, size_t row_count,
                                 size_t value_column_count, size_t *affected_count, RustExtError *err);
bool rust_ext_client_update_rows(RustExtClientConfig config, void *table, const RustExtString *row_ids,
                                 size_t row_count, const RustExtWriteColumn *columns, size_t column_count,
                                 const RustExtInputValue *values, size_t *affected_count, RustExtError *err);
bool rust_ext_client_delete_rows(RustExtClientConfig config, void *table, const RustExtString *row_ids, size_t count,
                                 size_t *affected_count, RustExtError *err);
bool rust_ext_alloc_string(const char *ptr, size_t len, RustExtString *out, RustExtError *err);

void rust_ext_free_string(RustExtString value);
void rust_ext_free_error(RustExtError err);
void rust_ext_free_scan_batch(RustExtScanBatch batch);
void rust_ext_free_catalog(RustExtCatalog catalog);
void rust_ext_free_attach_config(RustExtAttachConfig config);

#ifdef __cplusplus
}
#endif
