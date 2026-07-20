#pragma once

#include "rust_bridge_extension.h"
#include "duckdb/common/types.hpp"

namespace duckdb {

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
	idx_t limit = 0;
};

} // namespace duckdb
