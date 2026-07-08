#pragma once

#include "rust_bridge_extension.h"
#include "duckdb/common/common.hpp"
#include "duckdb/common/types/data_chunk.hpp"
#include "duckdb/common/types/value.hpp"
#include "duckdb/planner/expression.hpp"

namespace duckdb {

inline LogicalType RustBridgeDuckDBLogicalType(int32_t rust_bridge_type) {
	switch (static_cast<RustExtLogicalType>(rust_bridge_type)) {
	case RUST_EXT_LOGICAL_BOOLEAN:
		return LogicalType::BOOLEAN;
	case RUST_EXT_LOGICAL_DOUBLE:
		return LogicalType::DOUBLE;
	case RUST_EXT_LOGICAL_TIMESTAMP_TZ:
		return LogicalType::TIMESTAMP_TZ;
	case RUST_EXT_LOGICAL_VARCHAR:
	default:
		return LogicalType::VARCHAR;
	}
}

struct RustBridgeColumnInfo {
	const RustExtColumn *column = nullptr;
	LogicalType duckdb_type = LogicalType::VARCHAR;

	const RustExtColumn &Raw() const {
		return *column;
	}
};

struct RustBridgeTableInfo {
	const RustExtCatalogTable *table = nullptr;
	vector<RustBridgeColumnInfo> columns;

	const RustExtCatalogTable &Raw() const {
		return *table;
	}

	bool Supports(uint32_t capability) const {
		return (Raw().capabilities & capability) != 0;
	}
};

struct RustBridgeScanRequest {
	string filter;
	string order;
	idx_t limit = 500;
};

class RustBridgeInputValueBuffer {
public:
	RustExtInputValue Convert(const Value &value) {
		RustExtInputValue result {};
		if (value.IsNull()) {
			result.value_type = RUST_EXT_INPUT_NULL;
			return result;
		}

		switch (value.type().id()) {
		case LogicalTypeId::BOOLEAN:
			result.value_type = RUST_EXT_INPUT_BOOL;
			result.bool_value = BooleanValue::Get(value);
			return result;
		case LogicalTypeId::TINYINT:
		case LogicalTypeId::SMALLINT:
		case LogicalTypeId::INTEGER:
		case LogicalTypeId::BIGINT:
			result.value_type = RUST_EXT_INPUT_INT;
			result.int_value = value.GetValue<int64_t>();
			return result;
		case LogicalTypeId::UTINYINT:
		case LogicalTypeId::USMALLINT:
		case LogicalTypeId::UINTEGER:
		case LogicalTypeId::UBIGINT:
			result.value_type = RUST_EXT_INPUT_UINT;
			result.uint_value = value.GetValue<uint64_t>();
			return result;
		case LogicalTypeId::FLOAT:
		case LogicalTypeId::DOUBLE:
			result.value_type = RUST_EXT_INPUT_DOUBLE;
			result.double_value = value.GetValue<double>();
			return result;
		default:
			strings.push_back(value.ToString());
			result.value_type = RUST_EXT_INPUT_STRING;
			result.string_value = BorrowString(strings.back());
			return result;
		}
	}

	void Reserve(idx_t count) {
		strings.reserve(count);
	}

private:
	static RustExtString BorrowString(const string &value) {
		return RustExtString {const_cast<char *>(value.c_str()), value.size()};
	}

	vector<string> strings;
};

class RustBridgeCatalogResponse {
public:
	RustBridgeCatalogResponse() = default;
	explicit RustBridgeCatalogResponse(RustExtCatalog catalog_p) : catalog(catalog_p) {
	}
	~RustBridgeCatalogResponse() {
		Reset();
	}

	RustBridgeCatalogResponse(const RustBridgeCatalogResponse &) = delete;
	RustBridgeCatalogResponse &operator=(const RustBridgeCatalogResponse &) = delete;

	RustBridgeCatalogResponse(RustBridgeCatalogResponse &&other) noexcept : catalog(other.catalog) {
		other.catalog = {};
	}

	RustBridgeCatalogResponse &operator=(RustBridgeCatalogResponse &&other) noexcept {
		if (this != &other) {
			Reset();
			catalog = other.catalog;
			other.catalog = {};
		}
		return *this;
	}

	const RustExtCatalog &Raw() const {
		return catalog;
	}

	idx_t TableCount() const {
		return catalog.table_count;
	}

private:
	void Reset() {
		rust_ext_free_catalog(catalog);
		catalog = {};
	}

	RustExtCatalog catalog {};
};

class RustBridgeScanBatch {
public:
	RustBridgeScanBatch() = default;
	explicit RustBridgeScanBatch(RustExtScanBatch batch_p) : batch(batch_p) {
	}
	~RustBridgeScanBatch() {
		Reset();
	}

	RustBridgeScanBatch(const RustBridgeScanBatch &) = delete;
	RustBridgeScanBatch &operator=(const RustBridgeScanBatch &) = delete;

	RustBridgeScanBatch(RustBridgeScanBatch &&other) noexcept : batch(other.batch) {
		other.batch = {};
	}

	RustBridgeScanBatch &operator=(RustBridgeScanBatch &&other) noexcept {
		if (this != &other) {
			Reset();
			batch = other.batch;
			other.batch = {};
		}
		return *this;
	}

	const RustExtScanBatch &Raw() const {
		return batch;
	}

	idx_t RowCount() const {
		return batch.row_count;
	}

	bool Empty() const {
		return batch.row_count == 0;
	}

	bool Finished() const {
		return batch.finished;
	}

private:
	void Reset() {
		rust_ext_free_scan_batch(batch);
		batch = {};
	}

	RustExtScanBatch batch {};
};

class RustBridgeScanHandle {
public:
	RustBridgeScanHandle() = default;
	explicit RustBridgeScanHandle(void *handle_p) : handle(handle_p) {
	}
	~RustBridgeScanHandle() {
		Reset();
	}

	RustBridgeScanHandle(const RustBridgeScanHandle &) = delete;
	RustBridgeScanHandle &operator=(const RustBridgeScanHandle &) = delete;

	RustBridgeScanHandle(RustBridgeScanHandle &&other) noexcept : handle(other.handle) {
		other.handle = nullptr;
	}

	RustBridgeScanHandle &operator=(RustBridgeScanHandle &&other) noexcept {
		if (this != &other) {
			Reset();
			handle = other.handle;
			other.handle = nullptr;
		}
		return *this;
	}

	void *Raw() const {
		return handle;
	}

private:
	void Reset() {
		if (handle) {
			rust_ext_scan_close(handle);
			handle = nullptr;
		}
	}

	void *handle = nullptr;
};

class RustBridgeAttachConfig {
public:
	RustBridgeAttachConfig() = default;
	explicit RustBridgeAttachConfig(RustExtAttachConfig config_p) : config(config_p) {
	}
	~RustBridgeAttachConfig() {
		Reset();
	}

	RustBridgeAttachConfig(const RustBridgeAttachConfig &) = delete;
	RustBridgeAttachConfig &operator=(const RustBridgeAttachConfig &) = delete;

	RustBridgeAttachConfig(RustBridgeAttachConfig &&other) noexcept : config(other.config) {
		other.config = {};
	}

	RustBridgeAttachConfig &operator=(RustBridgeAttachConfig &&other) noexcept {
		if (this != &other) {
			Reset();
			config = other.config;
			other.config = {};
		}
		return *this;
	}

	const RustExtAttachConfig &Raw() const {
		return config;
	}

	RustExtClientConfig ClientConfig() const {
		return RustExtClientConfig {config.resource, config.credential, config.endpoint, false};
	}

	bool IncludeSystemColumns() const {
		return config.include_system_columns;
	}

private:
	void Reset() {
		rust_ext_free_attach_config(config);
		config = {};
	}

	RustExtAttachConfig config {};
};

class RustBridgeClient {
public:
	explicit RustBridgeClient(RustExtClientConfig config);

	RustBridgeCatalogResponse ListTables(bool include_system_columns);
	RustBridgeScanHandle OpenScan(RustExtString table_id, const RustBridgeScanRequest &request);
	RustBridgeScanBatch NextScanBatch(RustBridgeScanHandle &handle);

	idx_t InsertRows(const RustBridgeTableInfo &table, DataChunk &chunk);
	idx_t UpdateRows(const RustBridgeTableInfo &table, DataChunk &chunk, const vector<PhysicalIndex> &columns,
	                 const vector<unique_ptr<Expression>> &expressions);
	idx_t DeleteRows(const RustBridgeTableInfo &table, DataChunk &chunk, idx_t row_id_index);

private:
	RustExtClientConfig config;
};

} // namespace duckdb
