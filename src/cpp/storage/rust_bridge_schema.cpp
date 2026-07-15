#include "storage/rust_bridge_schema.hpp"

#include "rust_bridge_string.hpp"
#include "duckdb/common/exception.hpp"
#include "duckdb/parser/parsed_data/alter_info.hpp"
#include "duckdb/parser/parsed_data/create_info.hpp"
#include "duckdb/parser/parsed_data/create_table_info.hpp"
#include "duckdb/parser/parsed_data/drop_info.hpp"
#include "storage/rust_bridge_catalog.hpp"

namespace duckdb {

static RustBridgeTableInfo BorrowRustBridgeTableInfo(const RustExtCatalogTable &raw_table) {
	RustBridgeTableInfo table;
	table.table = &raw_table;
	table.columns.reserve(raw_table.column_count);
	for (idx_t col_idx = 0; col_idx < raw_table.column_count; col_idx++) {
		auto &raw_column = raw_table.columns[col_idx];
		table.columns.push_back(
		    RustBridgeColumnInfo {&raw_column, RustBridgeDuckDBLogicalType(raw_column.logical_type)});
	}
	return table;
}

static NotImplementedException DDLNotSupported(uint8_t operation) {
	return NotImplementedException("%s", rust_ext_ddl_not_supported_message(operation));
}

static NotImplementedException DropEntryNotSupported(const string &name) {
	RustExtString message;
	RustExtError error;
	if (!rust_ext_ddl_drop_entry_not_supported_message(name.c_str(), name.size(), &message, &error)) {
		return NotImplementedException("%s", TakeRustBridgeErrorMessage(error));
	}
	return NotImplementedException("%s", TakeRustBridgeString(message));
}

RustBridgeSchemaCatalogEntry::RustBridgeSchemaCatalogEntry(ClientContext &, RustBridgeCatalog &catalog,
                                                           CreateSchemaInfo &info,
                                                           const RustBridgeCatalogResponse &catalog_info)
    : SchemaCatalogEntry(catalog, info) {
	for (idx_t table_idx = 0; table_idx < catalog_info.TableCount(); table_idx++) {
		auto table = BorrowRustBridgeTableInfo(catalog_info.Raw().tables[table_idx]);
		auto create_info = RustBridgeCreateTableInfo(table, name.GetIdentifierName());
		auto entry = make_uniq<RustBridgeTableCatalogEntry>(catalog, *this, create_info, table);
		entries[RustBridgeString(table.Raw().name)] = std::move(entry);
	}
}

RustBridgeSchemaCatalogEntry::~RustBridgeSchemaCatalogEntry() {
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::LookupEntry(CatalogTransaction,
                                                                     const EntryLookupInfo &lookup_info) {
	auto entry = entries.find(lookup_info.GetEntryName());
	if (entry == entries.end()) {
		return nullptr;
	}
	return entry->second.get();
}

void RustBridgeSchemaCatalogEntry::Scan(ClientContext &, CatalogType type,
                                        const std::function<void(CatalogEntry &)> &callback) {
	Scan(type, callback);
}

void RustBridgeSchemaCatalogEntry::Scan(CatalogType type, const std::function<void(CatalogEntry &)> &callback) {
	if (type != CatalogType::TABLE_ENTRY && type != CatalogType::INVALID) {
		return;
	}
	for (auto &entry : entries) {
		callback(*entry.second);
	}
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreateIndex(CatalogTransaction, CreateIndexInfo &,
                                                                     TableCatalogEntry &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_INDEX);
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreateFunction(CatalogTransaction, CreateFunctionInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_FUNCTION);
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreateTable(CatalogTransaction, BoundCreateTableInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_TABLE);
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreateView(CatalogTransaction, CreateViewInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_VIEW);
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreateSequence(CatalogTransaction, CreateSequenceInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_SEQUENCE);
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreateTableFunction(CatalogTransaction,
                                                                             CreateTableFunctionInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_TABLE_FUNCTION);
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreateCopyFunction(CatalogTransaction,
                                                                            CreateCopyFunctionInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_COPY_FUNCTION);
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreatePragmaFunction(CatalogTransaction,
                                                                              CreatePragmaFunctionInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_PRAGMA_FUNCTION);
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreateCollation(CatalogTransaction, CreateCollationInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_COLLATION);
}

optional_ptr<CatalogEntry> RustBridgeSchemaCatalogEntry::CreateType(CatalogTransaction, CreateTypeInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_CREATE_TYPE);
}

void RustBridgeSchemaCatalogEntry::DropEntry(ClientContext &, DropInfo &info) {
	throw DropEntryNotSupported(info.GetQualifiedName().Name().GetIdentifierName());
}

void RustBridgeSchemaCatalogEntry::Alter(CatalogTransaction, AlterInfo &) {
	throw DDLNotSupported(RUST_EXT_DDL_ALTER);
}

} // namespace duckdb
