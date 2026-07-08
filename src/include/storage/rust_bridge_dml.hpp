#pragma once

#include "duckdb/execution/physical_operator.hpp"
#include "duckdb/planner/operator/logical_update.hpp"
#include "storage/rust_bridge_table.hpp"

namespace duckdb {

class RustBridgeInsert : public PhysicalOperator {
public:
	RustBridgeInsert(PhysicalPlan &physical_plan, LogicalOperator &op, RustBridgeTableCatalogEntry &table);

	optional_ptr<RustBridgeTableCatalogEntry> table;

	unique_ptr<GlobalSinkState> GetGlobalSinkState(ClientContext &context) const override;
	SinkResultType Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const override;
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
	string GetName() const override;
};

class RustBridgeUpdate : public PhysicalOperator {
public:
	RustBridgeUpdate(PhysicalPlan &physical_plan, LogicalUpdate &op, RustBridgeTableCatalogEntry &table);

	optional_ptr<RustBridgeTableCatalogEntry> table;
	vector<PhysicalIndex> columns;
	vector<unique_ptr<Expression>> expressions;

	unique_ptr<GlobalSinkState> GetGlobalSinkState(ClientContext &context) const override;
	SinkResultType Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const override;
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
	string GetName() const override;
};

class RustBridgeDelete : public PhysicalOperator {
public:
	RustBridgeDelete(PhysicalPlan &physical_plan, LogicalDelete &op, RustBridgeTableCatalogEntry &table,
	                 idx_t row_id_index);

	optional_ptr<RustBridgeTableCatalogEntry> table;
	idx_t row_id_index;

	unique_ptr<GlobalSinkState> GetGlobalSinkState(ClientContext &context) const override;
	SinkResultType Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const override;
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
	string GetName() const override;
};

} // namespace duckdb
