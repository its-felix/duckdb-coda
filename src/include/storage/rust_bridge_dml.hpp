#pragma once

#include "duckdb/execution/physical_operator.hpp"
#include "duckdb/planner/operator/logical_update.hpp"
#include "storage/rust_bridge_table.hpp"

namespace duckdb {

class RustBridgeDML : public PhysicalOperator {
public:
	RustBridgeDML(PhysicalPlan &physical_plan, LogicalOperator &op, idx_t estimated_cardinality);

	unique_ptr<GlobalSinkState> GetGlobalSinkState(ClientContext &context) const override;
	SinkCombineResultType Combine(ExecutionContext &context, OperatorSinkCombineInput &input) const override;
	SourceResultType GetDataInternal(ExecutionContext &context, DataChunk &chunk,
	                                 OperatorSourceInput &input) const override;

	bool IsSink() const override {
		return true;
	}
	bool IsSource() const override {
		return true;
	}
	bool ParallelSink() const override {
		return false;
	}
};

class RustBridgeInsert : public RustBridgeDML {
public:
	RustBridgeInsert(PhysicalPlan &physical_plan, LogicalOperator &op, RustBridgeTableCatalogEntry &table);

	optional_ptr<RustBridgeTableCatalogEntry> table;

	SinkResultType Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const override;
	string GetName() const override;
};

class RustBridgeUpdate : public RustBridgeDML {
public:
	RustBridgeUpdate(PhysicalPlan &physical_plan, LogicalUpdate &op, RustBridgeTableCatalogEntry &table);

	optional_ptr<RustBridgeTableCatalogEntry> table;
	vector<PhysicalIndex> columns;
	vector<unique_ptr<Expression>> expressions;

	SinkResultType Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const override;
	string GetName() const override;
};

class RustBridgeDelete : public RustBridgeDML {
public:
	RustBridgeDelete(PhysicalPlan &physical_plan, LogicalDelete &op, RustBridgeTableCatalogEntry &table,
	                 idx_t row_id_index);

	optional_ptr<RustBridgeTableCatalogEntry> table;
	idx_t row_id_index;

	SinkResultType Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const override;
	string GetName() const override;
};

} // namespace duckdb
