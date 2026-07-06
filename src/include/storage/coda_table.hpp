#pragma once

#include "coda_client.hpp"
#include "duckdb/catalog/catalog_entry/table_catalog_entry.hpp"

namespace duckdb {

class CodaTableCatalogEntry : public TableCatalogEntry {
public:
	CodaTableCatalogEntry(Catalog &catalog, SchemaCatalogEntry &schema, CreateTableInfo &info, CodaTableInfo table);

	unique_ptr<BaseStatistics> GetStatistics(ClientContext &context, column_t column_id) override;
	TableFunction GetScanFunction(ClientContext &context, unique_ptr<FunctionData> &bind_data) override;
	TableStorageInfo GetStorageInfo(ClientContext &context) override;
	virtual_column_map_t GetVirtualColumns() const override;
	vector<column_t> GetRowIdColumns() const override;

	const CodaTableInfo &TableInfo() const {
		return table;
	}

private:
	CodaTableInfo table;
};

CreateTableInfo CodaCreateTableInfo(const CodaTableInfo &table, const string &schema_name);

} // namespace duckdb
