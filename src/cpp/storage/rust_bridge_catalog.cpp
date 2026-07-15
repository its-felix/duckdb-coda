#include "storage/rust_bridge_catalog.hpp"

#include "rust_bridge_string.hpp"
#include "duckdb/common/exception.hpp"
#include "duckdb/common/string_util.hpp"
#include "duckdb/execution/physical_plan_generator.hpp"
#include "duckdb/main/client_context.hpp"
#include "duckdb/parser/parsed_data/create_schema_info.hpp"
#include "duckdb/parser/parsed_data/drop_info.hpp"
#include "duckdb/planner/expression/bound_reference_expression.hpp"
#include "duckdb/planner/operator/logical_create_table.hpp"
#include "duckdb/planner/operator/logical_delete.hpp"
#include "duckdb/planner/operator/logical_insert.hpp"
#include "duckdb/planner/operator/logical_update.hpp"
#include "duckdb/storage/database_size.hpp"
#include "storage/rust_bridge_dml.hpp"

namespace duckdb {

static void RejectExplicitTransaction(ClientContext &context) {
	if (rust_ext_supports_explicit_transactions()) {
		return;
	}
	auto query = StringUtil::Lower(context.GetCurrentQuery());
	if (!context.transaction.IsAutoCommit() || query.find("begin") != string::npos ||
	    query.find("rollback") != string::npos || query.find("commit") != string::npos) {
		throw NotImplementedException("%s", rust_ext_explicit_transaction_not_supported_message());
	}
}

RustBridgeCatalog::RustBridgeCatalog(AttachedDatabase &db, ClientContext &context,
                                     RustBridgeAttachConfig attach_config_p)
    : Catalog(db), attach_config(std::move(attach_config_p)) {
	LoadCatalog(context);
}

RustBridgeCatalog::~RustBridgeCatalog() {
}

void RustBridgeCatalog::LoadCatalog(ClientContext &context) {
	auto client = Client();
	catalog_info = client.ListTables(attach_config.IncludeSystemColumns());

	CreateSchemaInfo schema_info;
	main_schema = make_uniq<RustBridgeSchemaCatalogEntry>(context, *this, schema_info, catalog_info);
}

RustBridgeClient RustBridgeCatalog::Client() const {
	return RustBridgeClient(attach_config.ClientConfig());
}

void RustBridgeCatalog::Initialize(bool) {
}

optional_ptr<SchemaCatalogEntry> RustBridgeCatalog::LookupSchema(CatalogTransaction,
                                                                 const EntryLookupInfo &schema_lookup,
                                                                 OnEntryNotFound if_not_found) {
	if (StringUtil::CIEquals(schema_lookup.GetEntryName(), DEFAULT_SCHEMA)) {
		return main_schema.get();
	}
	if (if_not_found == OnEntryNotFound::THROW_EXCEPTION) {
		throw BinderException("Schema with name \"%s\" not found", schema_lookup.GetEntryName());
	}
	return nullptr;
}

void RustBridgeCatalog::ScanSchemas(ClientContext &, std::function<void(SchemaCatalogEntry &)> callback) {
	callback(*main_schema);
}

optional_ptr<CatalogEntry> RustBridgeCatalog::CreateSchema(CatalogTransaction, CreateSchemaInfo &) {
	throw NotImplementedException("%s", rust_ext_ddl_not_supported_message(RUST_EXT_DDL_CREATE_SCHEMA));
}

void RustBridgeCatalog::DropSchema(ClientContext &, DropInfo &) {
	throw NotImplementedException("%s", rust_ext_ddl_not_supported_message(RUST_EXT_DDL_DROP_SCHEMA));
}

PhysicalOperator &RustBridgeCatalog::PlanInsert(ClientContext &context, PhysicalPlanGenerator &planner,
                                                LogicalInsert &op, optional_ptr<PhysicalOperator> plan) {
	RejectExplicitTransaction(context);
	if (op.return_chunk) {
		throw NotImplementedException("%s", rust_ext_returning_not_supported_message(RUST_EXT_DML_INSERT));
	}
	if (!op.table.Cast<RustBridgeTableCatalogEntry>().TableInfo().Supports(RUST_EXT_TABLE_INSERT)) {
		throw NotImplementedException("%s", rust_ext_dml_not_supported_message(RUST_EXT_DML_INSERT));
	}
	D_ASSERT(plan);
	if (!op.column_index_map.empty()) {
		plan = planner.ResolveDefaultsProjection(op, *plan);
	}
	auto &insert = planner.Make<RustBridgeInsert>(op, op.table.Cast<RustBridgeTableCatalogEntry>());
	insert.children.push_back(*plan);
	return insert;
}

PhysicalOperator &RustBridgeCatalog::PlanCreateTableAs(ClientContext &, PhysicalPlanGenerator &, LogicalCreateTable &,
                                                       PhysicalOperator &) {
	throw NotImplementedException("%s", rust_ext_ddl_not_supported_message(RUST_EXT_DDL_CREATE_TABLE_AS));
}

PhysicalOperator &RustBridgeCatalog::PlanUpdate(ClientContext &context, PhysicalPlanGenerator &planner,
                                                LogicalUpdate &op, PhysicalOperator &plan) {
	RejectExplicitTransaction(context);
	if (op.return_chunk) {
		throw NotImplementedException("%s", rust_ext_returning_not_supported_message(RUST_EXT_DML_UPDATE));
	}
	if (!op.table.Cast<RustBridgeTableCatalogEntry>().TableInfo().Supports(RUST_EXT_TABLE_UPDATE)) {
		throw NotImplementedException("%s", rust_ext_dml_not_supported_message(RUST_EXT_DML_UPDATE));
	}
	auto &update = planner.Make<RustBridgeUpdate>(op, op.table.Cast<RustBridgeTableCatalogEntry>());
	update.children.push_back(plan);
	return update;
}

PhysicalOperator &RustBridgeCatalog::PlanDelete(ClientContext &context, PhysicalPlanGenerator &planner,
                                                LogicalDelete &op, PhysicalOperator &plan) {
	RejectExplicitTransaction(context);
	if (op.return_chunk) {
		throw NotImplementedException("%s", rust_ext_returning_not_supported_message(RUST_EXT_DML_DELETE));
	}
	if (!op.table.Cast<RustBridgeTableCatalogEntry>().TableInfo().Supports(RUST_EXT_TABLE_DELETE)) {
		throw NotImplementedException("%s", rust_ext_dml_not_supported_message(RUST_EXT_DML_DELETE));
	}
	auto &bound_ref = op.expressions[0]->Cast<BoundReferenceExpression>();
	auto &del = planner.Make<RustBridgeDelete>(op, op.table.Cast<RustBridgeTableCatalogEntry>(), bound_ref.Index());
	del.children.push_back(plan);
	return del;
}

unique_ptr<LogicalOperator> RustBridgeCatalog::BindCreateIndex(Binder &, CreateStatement &, TableCatalogEntry &,
                                                               unique_ptr<LogicalOperator>) {
	throw NotImplementedException("%s", rust_ext_ddl_not_supported_message(RUST_EXT_DDL_CREATE_INDEX));
}

DatabaseSize RustBridgeCatalog::GetDatabaseSize(ClientContext &) {
	throw NotImplementedException("%s", rust_ext_database_size_not_available_message());
}

bool RustBridgeCatalog::InMemory() {
	return false;
}

string RustBridgeCatalog::GetDBPath() {
	return RustBridgeString(attach_config.Raw().resource);
}

} // namespace duckdb
