#pragma once

#include "coda_client.hpp"
#include "duckdb/catalog/catalog.hpp"
#include "storage/coda_schema.hpp"

namespace duckdb {

class CodaCatalog : public Catalog {
public:
  CodaCatalog(AttachedDatabase &db, ClientContext &context, string doc_id,
              string token, string api_base);
  ~CodaCatalog() override;

  string GetCatalogType() override { return "coda"; }

  void Initialize(bool load_builtin) override;
  optional_ptr<SchemaCatalogEntry>
  LookupSchema(CatalogTransaction transaction,
               const EntryLookupInfo &schema_lookup,
               OnEntryNotFound if_not_found) override;
  void ScanSchemas(ClientContext &context,
                   std::function<void(SchemaCatalogEntry &)> callback) override;
  optional_ptr<CatalogEntry> CreateSchema(CatalogTransaction transaction,
                                          CreateSchemaInfo &info) override;
  void DropSchema(ClientContext &context, DropInfo &info) override;

  PhysicalOperator &PlanInsert(ClientContext &context,
                               PhysicalPlanGenerator &planner,
                               LogicalInsert &op,
                               optional_ptr<PhysicalOperator> plan) override;
  PhysicalOperator &PlanCreateTableAs(ClientContext &context,
                                      PhysicalPlanGenerator &planner,
                                      LogicalCreateTable &op,
                                      PhysicalOperator &plan) override;
  PhysicalOperator &PlanUpdate(ClientContext &context,
                               PhysicalPlanGenerator &planner,
                               LogicalUpdate &op,
                               PhysicalOperator &plan) override;
  PhysicalOperator &PlanDelete(ClientContext &context,
                               PhysicalPlanGenerator &planner,
                               LogicalDelete &op,
                               PhysicalOperator &plan) override;
  unique_ptr<LogicalOperator>
  BindCreateIndex(Binder &binder, CreateStatement &stmt,
                  TableCatalogEntry &table,
                  unique_ptr<LogicalOperator> plan) override;

  DatabaseSize GetDatabaseSize(ClientContext &context) override;
  bool InMemory() override;
  string GetDBPath() override;

  CodaClient Client(ClientContext &context) const;
  const vector<CodaTableInfo> &Tables() const { return tables; }
  const string &DocId() const { return doc_id; }
  const string &Token() const { return token; }
  const string &APIBase() const { return api_base; }

private:
  void LoadCatalog(ClientContext &context);

private:
  string doc_id;
  string token;
  string api_base;
  vector<CodaTableInfo> tables;
  unique_ptr<CodaSchemaCatalogEntry> main_schema;
};

} // namespace duckdb
