#pragma once

#include "rust_bridge_client.hpp"
#include "duckdb/function/table_function.hpp"

namespace duckdb {

class TableCatalogEntry;

struct RustBridgeScanBindData : FunctionData {
	RustBridgeScanBindData(TableCatalogEntry &table_entry_p, RustBridgeTableInfo table_p)
	    : table_entry(table_entry_p), table(std::move(table_p)) {
	}

	unique_ptr<FunctionData> Copy() const override {
		auto copy = make_uniq<RustBridgeScanBindData>(table_entry, table);
		copy->pushed_query = pushed_query;
		copy->pushed_query_description = pushed_query_description;
		copy->pushed_sort_by = pushed_sort_by;
		copy->pushed_limit = pushed_limit;
		return std::move(copy);
	}

	bool Equals(const FunctionData &other_p) const override {
		auto &other = other_p.Cast<RustBridgeScanBindData>();
		auto table_id = table.Raw().id;
		auto other_table_id = other.table.Raw().id;
		return &table_entry == &other.table_entry && table_id.ptr == other_table_id.ptr &&
		       table_id.len == other_table_id.len;
	}

	TableCatalogEntry &table_entry;
	RustBridgeTableInfo table;
	string pushed_query;
	string pushed_query_description;
	string pushed_sort_by;
	idx_t pushed_limit = 0;
};

class RustBridgeScanFunction {
public:
	static TableFunction GetFunction();
};

} // namespace duckdb
