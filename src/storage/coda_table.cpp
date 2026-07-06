#include "storage/coda_table.hpp"

#include "coda_scan.hpp"
#include "duckdb/common/string_util.hpp"
#include "duckdb/parser/parsed_data/create_table_info.hpp"
#include "duckdb/storage/table_storage_info.hpp"
#include "storage/coda_catalog.hpp"

namespace duckdb {

static string UniqueColumnName(const string &name, unordered_set<string> &seen) {
	auto result = name.empty() ? "column" : name;
	auto candidate = result;
	idx_t suffix = 1;
	while (seen.find(StringUtil::Lower(candidate)) != seen.end()) {
		candidate = result + "_" + to_string(++suffix);
	}
	seen.insert(StringUtil::Lower(candidate));
	return candidate;
}

CreateTableInfo CodaCreateTableInfo(const CodaTableInfo &table, const string &schema_name) {
	CreateTableInfo info(INVALID_CATALOG, schema_name, table.name);
	unordered_set<string> seen;
	for (auto &column : table.columns) {
		info.columns.AddColumn(ColumnDefinition(UniqueColumnName(column.name, seen), column.duckdb_type));
	}
	return info;
}

CodaTableCatalogEntry::CodaTableCatalogEntry(Catalog &catalog, SchemaCatalogEntry &schema, CreateTableInfo &info,
                                             CodaTableInfo table_p)
    : TableCatalogEntry(catalog, schema, info), table(std::move(table_p)) {
}

unique_ptr<BaseStatistics> CodaTableCatalogEntry::GetStatistics(ClientContext &, column_t) {
	return nullptr;
}

TableFunction CodaTableCatalogEntry::GetScanFunction(ClientContext &, unique_ptr<FunctionData> &bind_data) {
	auto &coda_catalog = catalog.Cast<CodaCatalog>();
	bind_data =
	    make_uniq<CodaScanBindData>(*this, coda_catalog.DocId(), coda_catalog.Token(), coda_catalog.APIBase(), table);
	return CodaScanFunction::GetFunction();
}

TableStorageInfo CodaTableCatalogEntry::GetStorageInfo(ClientContext &) {
	return TableStorageInfo();
}

virtual_column_map_t CodaTableCatalogEntry::GetVirtualColumns() const {
	virtual_column_map_t result;
	result.insert(make_pair(COLUMN_IDENTIFIER_ROW_ID, TableColumn("rowid", LogicalType::VARCHAR)));
	return result;
}

vector<column_t> CodaTableCatalogEntry::GetRowIdColumns() const {
	return {COLUMN_IDENTIFIER_ROW_ID};
}

} // namespace duckdb
