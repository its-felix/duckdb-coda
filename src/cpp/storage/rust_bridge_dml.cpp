#include "storage/rust_bridge_dml.hpp"

#include "rust_bridge_extension.h"
#include "duckdb/common/exception.hpp"
#include "duckdb/common/string_util.hpp"
#include "duckdb/main/client_context.hpp"
#include "duckdb/planner/operator/logical_delete.hpp"
#include "duckdb/transaction/transaction.hpp"
#include "storage/rust_bridge_catalog.hpp"
#include "storage/rust_bridge_transaction.hpp"

namespace duckdb {

class RustBridgeDMLGlobalState : public GlobalSinkState {
public:
	RustBridgeDMLGlobalState() : affected_count(0) {
	}

	mutex lock;
	idx_t affected_count;
};

static RustBridgeClient GetClient(const RustBridgeTableCatalogEntry &table) {
	auto &catalog = table.catalog.Cast<RustBridgeCatalog>();
	return catalog.Client();
}

static void AddAffected(RustBridgeDMLGlobalState &state, idx_t count) {
	lock_guard<mutex> lock(state.lock);
	state.affected_count += count;
}

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

static void MarkRemoteWrite(ClientContext &context, const RustBridgeTableCatalogEntry &table) {
	auto &transaction = Transaction::Get(context, table.catalog.GetAttached()).Cast<RustBridgeTransaction>();
	transaction.MarkWrite();
}

RustBridgeInsert::RustBridgeInsert(PhysicalPlan &physical_plan, LogicalOperator &op,
                                   RustBridgeTableCatalogEntry &table_p)
    : PhysicalOperator(physical_plan, PhysicalOperatorType::EXTENSION, op.types, 1), table(table_p) {
}

unique_ptr<GlobalSinkState> RustBridgeInsert::GetGlobalSinkState(ClientContext &context) const {
	return make_uniq<RustBridgeDMLGlobalState>();
}

SinkResultType RustBridgeInsert::Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const {
	if (chunk.size() == 0) {
		return SinkResultType::NEED_MORE_INPUT;
	}
	RejectExplicitTransaction(context.client);
	if (!table->TableInfo().Supports(RUST_EXT_TABLE_INSERT)) {
		throw NotImplementedException("%s", rust_ext_dml_not_supported_message(RUST_EXT_DML_INSERT));
	}
	auto client = GetClient(*table);
	auto count = client.InsertRows(table->TableInfo(), chunk);
	MarkRemoteWrite(context.client, *table);
	AddAffected(input.global_state.Cast<RustBridgeDMLGlobalState>(), count);
	return SinkResultType::NEED_MORE_INPUT;
}

SinkCombineResultType RustBridgeInsert::Combine(ExecutionContext &, OperatorSinkCombineInput &) const {
	return SinkCombineResultType::FINISHED;
}

SourceResultType RustBridgeInsert::GetDataInternal(ExecutionContext &, DataChunk &chunk, OperatorSourceInput &) const {
	auto &state = sink_state->Cast<RustBridgeDMLGlobalState>();
	chunk.data[0].SetValue(0, Value::BIGINT(NumericCast<int64_t>(state.affected_count)));
	chunk.SetCardinality(1);
	return SourceResultType::FINISHED;
}

string RustBridgeInsert::GetName() const {
	return rust_ext_insert_operator_name();
}

RustBridgeUpdate::RustBridgeUpdate(PhysicalPlan &physical_plan, LogicalUpdate &op, RustBridgeTableCatalogEntry &table_p)
    : PhysicalOperator(physical_plan, PhysicalOperatorType::EXTENSION, op.types, op.estimated_cardinality),
      table(table_p), columns(std::move(op.columns)), expressions(std::move(op.expressions)) {
}

unique_ptr<GlobalSinkState> RustBridgeUpdate::GetGlobalSinkState(ClientContext &context) const {
	return make_uniq<RustBridgeDMLGlobalState>();
}

SinkResultType RustBridgeUpdate::Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const {
	if (chunk.size() == 0) {
		return SinkResultType::NEED_MORE_INPUT;
	}
	RejectExplicitTransaction(context.client);
	if (!table->TableInfo().Supports(RUST_EXT_TABLE_UPDATE)) {
		throw NotImplementedException("%s", rust_ext_dml_not_supported_message(RUST_EXT_DML_UPDATE));
	}
	auto client = GetClient(*table);
	auto count = client.UpdateRows(table->TableInfo(), chunk, columns, expressions);
	MarkRemoteWrite(context.client, *table);
	AddAffected(input.global_state.Cast<RustBridgeDMLGlobalState>(), count);
	return SinkResultType::NEED_MORE_INPUT;
}

SinkCombineResultType RustBridgeUpdate::Combine(ExecutionContext &, OperatorSinkCombineInput &) const {
	return SinkCombineResultType::FINISHED;
}

SourceResultType RustBridgeUpdate::GetDataInternal(ExecutionContext &, DataChunk &chunk, OperatorSourceInput &) const {
	auto &state = sink_state->Cast<RustBridgeDMLGlobalState>();
	chunk.data[0].SetValue(0, Value::BIGINT(NumericCast<int64_t>(state.affected_count)));
	chunk.SetCardinality(1);
	return SourceResultType::FINISHED;
}

string RustBridgeUpdate::GetName() const {
	return rust_ext_update_operator_name();
}

RustBridgeDelete::RustBridgeDelete(PhysicalPlan &physical_plan, LogicalDelete &op, RustBridgeTableCatalogEntry &table_p,
                                   idx_t row_id_index_p)
    : PhysicalOperator(physical_plan, PhysicalOperatorType::EXTENSION, op.types, op.estimated_cardinality),
      table(table_p), row_id_index(row_id_index_p) {
}

unique_ptr<GlobalSinkState> RustBridgeDelete::GetGlobalSinkState(ClientContext &context) const {
	return make_uniq<RustBridgeDMLGlobalState>();
}

SinkResultType RustBridgeDelete::Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const {
	if (chunk.size() == 0) {
		return SinkResultType::NEED_MORE_INPUT;
	}
	RejectExplicitTransaction(context.client);
	if (!table->TableInfo().Supports(RUST_EXT_TABLE_DELETE)) {
		throw NotImplementedException("%s", rust_ext_dml_not_supported_message(RUST_EXT_DML_DELETE));
	}
	auto client = GetClient(*table);
	auto count = client.DeleteRows(table->TableInfo(), chunk, row_id_index);
	MarkRemoteWrite(context.client, *table);
	AddAffected(input.global_state.Cast<RustBridgeDMLGlobalState>(), count);
	return SinkResultType::NEED_MORE_INPUT;
}

SinkCombineResultType RustBridgeDelete::Combine(ExecutionContext &, OperatorSinkCombineInput &) const {
	return SinkCombineResultType::FINISHED;
}

SourceResultType RustBridgeDelete::GetDataInternal(ExecutionContext &, DataChunk &chunk, OperatorSourceInput &) const {
	auto &state = sink_state->Cast<RustBridgeDMLGlobalState>();
	chunk.data[0].SetValue(0, Value::BIGINT(NumericCast<int64_t>(state.affected_count)));
	chunk.SetCardinality(1);
	return SourceResultType::FINISHED;
}

string RustBridgeDelete::GetName() const {
	return rust_ext_delete_operator_name();
}

} // namespace duckdb
