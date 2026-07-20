#pragma once

#include "rust_bridge_types.hpp"
#include "duckdb/catalog/catalog_entry/table_catalog_entry.hpp"

namespace duckdb {

class RustBridgeTableCatalogEntry : public TableCatalogEntry {
public:
	RustBridgeTableCatalogEntry(Catalog &catalog, SchemaCatalogEntry &schema, CreateTableInfo &info,
	                            RustBridgeTableInfo table);

	unique_ptr<BaseStatistics> GetStatistics(ClientContext &context, column_t column_id) override;
	TableFunction GetScanFunction(ClientContext &context, unique_ptr<FunctionData> &bind_data) override;
	TableStorageInfo GetStorageInfo(ClientContext &context) override;
	virtual_column_map_t GetVirtualColumns() const override;
	vector<column_t> GetRowIdColumns() const override;

	const RustBridgeTableInfo &TableInfo() const {
		return table;
	}

private:
	RustBridgeTableInfo table;
};

CreateTableInfo RustBridgeCreateTableInfo(const RustBridgeTableInfo &table, const string &schema_name);

} // namespace duckdb
