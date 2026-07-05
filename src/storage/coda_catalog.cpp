#include "storage/coda_catalog.hpp"

#include "duckdb/common/exception.hpp"
#include "duckdb/parser/parsed_data/create_schema_info.hpp"
#include "duckdb/parser/parsed_data/drop_info.hpp"
#include "duckdb/planner/expression/bound_reference_expression.hpp"
#include "duckdb/planner/operator/logical_create_table.hpp"
#include "duckdb/planner/operator/logical_delete.hpp"
#include "duckdb/planner/operator/logical_insert.hpp"
#include "duckdb/planner/operator/logical_update.hpp"
#include "duckdb/storage/database_size.hpp"
#include "storage/coda_dml.hpp"

namespace duckdb {

CodaCatalog::CodaCatalog(AttachedDatabase &db, ClientContext &context,
                         string doc_id_p, string token_p, string api_base_p,
                         bool include_row_metadata_p)
    : Catalog(db), doc_id(std::move(doc_id_p)), token(std::move(token_p)),
      api_base(std::move(api_base_p)),
      include_row_metadata(include_row_metadata_p) {
  LoadCatalog(context);
}

CodaCatalog::~CodaCatalog() {}

void CodaCatalog::LoadCatalog(ClientContext &context) {
  auto client = Client(context);
  tables = client.ListTables();
  if (include_row_metadata) {
    for (auto &table : tables) {
      CodaColumnInfo created_at;
      created_at.id = "createdAt";
      created_at.name = "createdAt";
      created_at.row_metadata = true;
      created_at.calculated = true;
      created_at.duckdb_type = LogicalType::TIMESTAMP_TZ;
      table.columns.push_back(std::move(created_at));

      CodaColumnInfo updated_at;
      updated_at.id = "updatedAt";
      updated_at.name = "updatedAt";
      updated_at.row_metadata = true;
      updated_at.calculated = true;
      updated_at.duckdb_type = LogicalType::TIMESTAMP_TZ;
      table.columns.push_back(std::move(updated_at));
    }
  }

  CreateSchemaInfo schema_info;
  schema_info.SetQualifiedName(
      QualifiedName({Identifier::DefaultSchema()}, Identifier()));
  main_schema =
      make_uniq<CodaSchemaCatalogEntry>(context, *this, schema_info, tables);
}

CodaClient CodaCatalog::Client(ClientContext &context) const {
  return CodaClient(context, doc_id, token, api_base);
}

void CodaCatalog::Initialize(bool) {}

optional_ptr<SchemaCatalogEntry>
CodaCatalog::LookupSchema(CatalogTransaction,
                          const EntryLookupInfo &schema_lookup,
                          OnEntryNotFound if_not_found) {
  if (StringUtil::CIEquals(schema_lookup.GetEntryName(), DEFAULT_SCHEMA)) {
    return main_schema.get();
  }
  if (if_not_found == OnEntryNotFound::THROW_EXCEPTION) {
    throw BinderException("Schema with name \"%s\" not found",
                          schema_lookup.GetEntryName());
  }
  return nullptr;
}

void CodaCatalog::ScanSchemas(
    ClientContext &, std::function<void(SchemaCatalogEntry &)> callback) {
  callback(*main_schema);
}

optional_ptr<CatalogEntry> CodaCatalog::CreateSchema(CatalogTransaction,
                                                     CreateSchemaInfo &) {
  throw NotImplementedException("Coda DDL is not supported: CREATE SCHEMA");
}

void CodaCatalog::DropSchema(ClientContext &, DropInfo &) {
  throw NotImplementedException("Coda DDL is not supported: DROP SCHEMA");
}

PhysicalOperator &CodaCatalog::PlanInsert(ClientContext &,
                                          PhysicalPlanGenerator &planner,
                                          LogicalInsert &op,
                                          optional_ptr<PhysicalOperator> plan) {
  if (op.return_chunk) {
    throw NotImplementedException("RETURNING is not supported for Coda INSERT");
  }
  D_ASSERT(plan);
  if (!op.column_index_map.empty()) {
    plan = planner.ResolveDefaultsProjection(op, *plan);
  }
  auto &insert =
      planner.Make<CodaInsert>(op, op.table.Cast<CodaTableCatalogEntry>());
  insert.children.push_back(*plan);
  return insert;
}

PhysicalOperator &CodaCatalog::PlanCreateTableAs(ClientContext &,
                                                 PhysicalPlanGenerator &,
                                                 LogicalCreateTable &,
                                                 PhysicalOperator &) {
  throw NotImplementedException("Coda DDL is not supported: CREATE TABLE AS");
}

PhysicalOperator &CodaCatalog::PlanUpdate(ClientContext &,
                                          PhysicalPlanGenerator &planner,
                                          LogicalUpdate &op,
                                          PhysicalOperator &plan) {
  if (op.return_chunk) {
    throw NotImplementedException("RETURNING is not supported for Coda UPDATE");
  }
  auto &update =
      planner.Make<CodaUpdate>(op, op.table.Cast<CodaTableCatalogEntry>());
  update.children.push_back(plan);
  return update;
}

PhysicalOperator &CodaCatalog::PlanDelete(ClientContext &,
                                          PhysicalPlanGenerator &planner,
                                          LogicalDelete &op,
                                          PhysicalOperator &plan) {
  if (op.return_chunk) {
    throw NotImplementedException("RETURNING is not supported for Coda DELETE");
  }
  auto &bound_ref = op.expressions[0]->Cast<BoundReferenceExpression>();
  auto &del = planner.Make<CodaDelete>(
      op, op.table.Cast<CodaTableCatalogEntry>(), bound_ref.Index());
  del.children.push_back(plan);
  return del;
}

unique_ptr<LogicalOperator>
CodaCatalog::BindCreateIndex(Binder &, CreateStatement &, TableCatalogEntry &,
                             unique_ptr<LogicalOperator>) {
  throw NotImplementedException("Coda DDL is not supported: CREATE INDEX");
}

DatabaseSize CodaCatalog::GetDatabaseSize(ClientContext &) {
  throw NotImplementedException("Coda database size is not available");
}

bool CodaCatalog::InMemory() { return false; }

string CodaCatalog::GetDBPath() { return doc_id; }

} // namespace duckdb
