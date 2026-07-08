#pragma once

#include "rust_bridge_client.hpp"
#include "duckdb/catalog/catalog.hpp"
#include "storage/rust_bridge_schema.hpp"

namespace duckdb {

class RustBridgeCatalog : public Catalog {
public:
	RustBridgeCatalog(AttachedDatabase &db, ClientContext &context, RustBridgeAttachConfig attach_config);
	~RustBridgeCatalog() override;

	string GetCatalogType() override {
		return rust_ext_extension_name();
	}

	void Initialize(bool load_builtin) override;
	optional_ptr<SchemaCatalogEntry> LookupSchema(CatalogTransaction transaction, const EntryLookupInfo &schema_lookup,
	                                              OnEntryNotFound if_not_found) override;
	void ScanSchemas(ClientContext &context, std::function<void(SchemaCatalogEntry &)> callback) override;
	optional_ptr<CatalogEntry> CreateSchema(CatalogTransaction transaction, CreateSchemaInfo &info) override;
	void DropSchema(ClientContext &context, DropInfo &info) override;

	PhysicalOperator &PlanInsert(ClientContext &context, PhysicalPlanGenerator &planner, LogicalInsert &op,
	                             optional_ptr<PhysicalOperator> plan) override;
	PhysicalOperator &PlanCreateTableAs(ClientContext &context, PhysicalPlanGenerator &planner, LogicalCreateTable &op,
	                                    PhysicalOperator &plan) override;
	PhysicalOperator &PlanUpdate(ClientContext &context, PhysicalPlanGenerator &planner, LogicalUpdate &op,
	                             PhysicalOperator &plan) override;
	PhysicalOperator &PlanDelete(ClientContext &context, PhysicalPlanGenerator &planner, LogicalDelete &op,
	                             PhysicalOperator &plan) override;
	unique_ptr<LogicalOperator> BindCreateIndex(Binder &binder, CreateStatement &stmt, TableCatalogEntry &table,
	                                            unique_ptr<LogicalOperator> plan) override;

	DatabaseSize GetDatabaseSize(ClientContext &context) override;
	bool InMemory() override;
	string GetDBPath() override;

	RustBridgeClient Client() const;
	const RustBridgeCatalogResponse &CatalogInfo() const {
		return catalog_info;
	}

private:
	void LoadCatalog(ClientContext &context);

private:
	RustBridgeAttachConfig attach_config;
	RustBridgeCatalogResponse catalog_info;
	unique_ptr<RustBridgeSchemaCatalogEntry> main_schema;
};

} // namespace duckdb
