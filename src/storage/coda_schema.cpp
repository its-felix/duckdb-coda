#include "storage/coda_schema.hpp"

#include "duckdb/common/exception.hpp"
#include "duckdb/parser/parsed_data/alter_info.hpp"
#include "duckdb/parser/parsed_data/create_info.hpp"
#include "duckdb/parser/parsed_data/create_table_info.hpp"
#include "duckdb/parser/parsed_data/drop_info.hpp"
#include "storage/coda_catalog.hpp"

namespace duckdb {

static NotImplementedException DDLNotSupported(const string &operation) {
  return NotImplementedException("Coda DDL is not supported: %s", operation);
}

CodaSchemaCatalogEntry::CodaSchemaCatalogEntry(
    ClientContext &, CodaCatalog &catalog, CreateSchemaInfo &info,
    const vector<CodaTableInfo> &tables)
    : SchemaCatalogEntry(catalog, info) {
  for (auto &table : tables) {
    auto create_info = CodaCreateTableInfo(table, name.GetIdentifierName());
    auto entry =
        make_uniq<CodaTableCatalogEntry>(catalog, *this, create_info, table);
    entries[table.name] = std::move(entry);
  }
}

CodaSchemaCatalogEntry::~CodaSchemaCatalogEntry() {}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::LookupEntry(CatalogTransaction,
                                    const EntryLookupInfo &lookup_info) {
  auto entry = entries.find(lookup_info.GetEntryName());
  if (entry == entries.end()) {
    return nullptr;
  }
  return entry->second.get();
}

void CodaSchemaCatalogEntry::Scan(
    ClientContext &, CatalogType type,
    const std::function<void(CatalogEntry &)> &callback) {
  Scan(type, callback);
}

void CodaSchemaCatalogEntry::Scan(
    CatalogType type, const std::function<void(CatalogEntry &)> &callback) {
  if (type != CatalogType::TABLE_ENTRY && type != CatalogType::INVALID) {
    return;
  }
  for (auto &entry : entries) {
    callback(*entry.second);
  }
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreateIndex(CatalogTransaction, CreateIndexInfo &,
                                    TableCatalogEntry &) {
  throw DDLNotSupported("CREATE INDEX");
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreateFunction(CatalogTransaction,
                                       CreateFunctionInfo &) {
  throw DDLNotSupported("CREATE FUNCTION");
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreateTable(CatalogTransaction,
                                    BoundCreateTableInfo &) {
  throw DDLNotSupported("CREATE TABLE");
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreateView(CatalogTransaction, CreateViewInfo &) {
  throw DDLNotSupported("CREATE VIEW");
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreateSequence(CatalogTransaction,
                                       CreateSequenceInfo &) {
  throw DDLNotSupported("CREATE SEQUENCE");
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreateTableFunction(CatalogTransaction,
                                            CreateTableFunctionInfo &) {
  throw DDLNotSupported("CREATE TABLE FUNCTION");
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreateCopyFunction(CatalogTransaction,
                                           CreateCopyFunctionInfo &) {
  throw DDLNotSupported("CREATE COPY FUNCTION");
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreatePragmaFunction(CatalogTransaction,
                                             CreatePragmaFunctionInfo &) {
  throw DDLNotSupported("CREATE PRAGMA FUNCTION");
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreateCollation(CatalogTransaction,
                                        CreateCollationInfo &) {
  throw DDLNotSupported("CREATE COLLATION");
}

optional_ptr<CatalogEntry>
CodaSchemaCatalogEntry::CreateType(CatalogTransaction, CreateTypeInfo &) {
  throw DDLNotSupported("CREATE TYPE");
}

void CodaSchemaCatalogEntry::DropEntry(ClientContext &, DropInfo &info) {
  throw DDLNotSupported("DROP " +
                        info.GetQualifiedName().Name().GetIdentifierName());
}

void CodaSchemaCatalogEntry::Alter(CatalogTransaction, AlterInfo &) {
  throw DDLNotSupported("ALTER");
}

} // namespace duckdb
