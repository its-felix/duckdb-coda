#pragma once

#include "duckdb/catalog/catalog_entry/schema_catalog_entry.hpp"
#include "duckdb/parser/parsed_data/create_schema_info.hpp"
#include "storage/coda_table.hpp"

namespace duckdb {

class CodaCatalog;

class CodaSchemaCatalogEntry : public SchemaCatalogEntry {
public:
  CodaSchemaCatalogEntry(ClientContext &context, CodaCatalog &catalog,
                         CreateSchemaInfo &info,
                         const vector<CodaTableInfo> &tables);
  ~CodaSchemaCatalogEntry() override;

  optional_ptr<CatalogEntry>
  LookupEntry(CatalogTransaction transaction,
              const EntryLookupInfo &lookup_info) override;
  void Scan(ClientContext &context, CatalogType type,
            const std::function<void(CatalogEntry &)> &callback) override;
  void Scan(CatalogType type,
            const std::function<void(CatalogEntry &)> &callback) override;

  optional_ptr<CatalogEntry> CreateIndex(CatalogTransaction transaction,
                                         CreateIndexInfo &info,
                                         TableCatalogEntry &table) override;
  optional_ptr<CatalogEntry> CreateFunction(CatalogTransaction transaction,
                                            CreateFunctionInfo &info) override;
  optional_ptr<CatalogEntry> CreateTable(CatalogTransaction transaction,
                                         BoundCreateTableInfo &info) override;
  optional_ptr<CatalogEntry> CreateView(CatalogTransaction transaction,
                                        CreateViewInfo &info) override;
  optional_ptr<CatalogEntry> CreateSequence(CatalogTransaction transaction,
                                            CreateSequenceInfo &info) override;
  optional_ptr<CatalogEntry>
  CreateTableFunction(CatalogTransaction transaction,
                      CreateTableFunctionInfo &info) override;
  optional_ptr<CatalogEntry>
  CreateCopyFunction(CatalogTransaction transaction,
                     CreateCopyFunctionInfo &info) override;
  optional_ptr<CatalogEntry>
  CreatePragmaFunction(CatalogTransaction transaction,
                       CreatePragmaFunctionInfo &info) override;
  optional_ptr<CatalogEntry>
  CreateCollation(CatalogTransaction transaction,
                  CreateCollationInfo &info) override;
  optional_ptr<CatalogEntry> CreateType(CatalogTransaction transaction,
                                        CreateTypeInfo &info) override;

  void DropEntry(ClientContext &context, DropInfo &info) override;
  void Alter(CatalogTransaction transaction, AlterInfo &info) override;

private:
  case_insensitive_map_t<unique_ptr<CatalogEntry>> entries;
};

} // namespace duckdb
