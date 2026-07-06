#include "storage/coda_dml.hpp"

#include "duckdb/common/types/column/column_data_collection.hpp"
#include "duckdb/execution/expression_executor.hpp"
#include "duckdb/planner/expression/bound_reference_expression.hpp"
#include "duckdb/planner/operator/logical_delete.hpp"
#include "storage/coda_catalog.hpp"

namespace duckdb {

class CodaDMLGlobalState : public GlobalSinkState {
public:
	explicit CodaDMLGlobalState(ClientContext &context, const vector<LogicalType> &return_types)
	    : affected_count(0), return_collection(context, return_types) {
	}

	mutex lock;
	idx_t affected_count;
	ColumnDataCollection return_collection;
};

class CodaDMLLocalState : public LocalSinkState {
public:
	CodaDMLLocalState(ClientContext &context, const vector<LogicalType> &types) {
		buffer.Initialize(Allocator::Get(context), types);
	}

	DataChunk buffer;
};

static CodaClient GetClient(ClientContext &context, const CodaTableCatalogEntry &table) {
	auto &catalog = table.catalog.Cast<CodaCatalog>();
	return catalog.Client(context);
}

static void AddAffected(CodaDMLGlobalState &state, idx_t count) {
	lock_guard<mutex> lock(state.lock);
	state.affected_count += count;
}

CodaInsert::CodaInsert(PhysicalPlan &physical_plan, LogicalOperator &op, CodaTableCatalogEntry &table_p)
    : PhysicalOperator(physical_plan, PhysicalOperatorType::EXTENSION, op.types, 1), table(table_p) {
}

unique_ptr<GlobalSinkState> CodaInsert::GetGlobalSinkState(ClientContext &context) const {
	return make_uniq<CodaDMLGlobalState>(context, GetTypes());
}

unique_ptr<LocalSinkState> CodaInsert::GetLocalSinkState(ExecutionContext &context) const {
	return make_uniq<CodaDMLLocalState>(context.client, table->GetTypes());
}

SinkResultType CodaInsert::Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const {
	if (chunk.size() == 0) {
		return SinkResultType::NEED_MORE_INPUT;
	}
	auto client = GetClient(context.client, *table);
	auto count = client.InsertRows(table->TableInfo(), chunk);
	AddAffected(input.global_state.Cast<CodaDMLGlobalState>(), count);
	return SinkResultType::NEED_MORE_INPUT;
}

SinkCombineResultType CodaInsert::Combine(ExecutionContext &, OperatorSinkCombineInput &) const {
	return SinkCombineResultType::FINISHED;
}

SourceResultType CodaInsert::GetDataInternal(ExecutionContext &, DataChunk &chunk, OperatorSourceInput &) const {
	auto &state = sink_state->Cast<CodaDMLGlobalState>();
	chunk.data[0].SetValue(0, Value::BIGINT(NumericCast<int64_t>(state.affected_count)));
	chunk.SetCardinality(1);
	return SourceResultType::FINISHED;
}

string CodaInsert::GetName() const {
	return "CODA_INSERT";
}

CodaUpdate::CodaUpdate(PhysicalPlan &physical_plan, LogicalUpdate &op, CodaTableCatalogEntry &table_p)
    : PhysicalOperator(physical_plan, PhysicalOperatorType::EXTENSION, op.types, op.estimated_cardinality),
      table(table_p), columns(std::move(op.columns)), expressions(std::move(op.expressions)),
      return_chunk(op.return_chunk) {
}

unique_ptr<GlobalSinkState> CodaUpdate::GetGlobalSinkState(ClientContext &context) const {
	return make_uniq<CodaDMLGlobalState>(context, GetTypes());
}

unique_ptr<LocalSinkState> CodaUpdate::GetLocalSinkState(ExecutionContext &context) const {
	return make_uniq<CodaDMLLocalState>(context.client, table->GetTypes());
}

SinkResultType CodaUpdate::Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const {
	if (chunk.size() == 0) {
		return SinkResultType::NEED_MORE_INPUT;
	}
	auto client = GetClient(context.client, *table);
	auto count = client.UpdateRows(table->TableInfo(), chunk, columns, expressions);
	AddAffected(input.global_state.Cast<CodaDMLGlobalState>(), count);
	return SinkResultType::NEED_MORE_INPUT;
}

SinkCombineResultType CodaUpdate::Combine(ExecutionContext &, OperatorSinkCombineInput &) const {
	return SinkCombineResultType::FINISHED;
}

SourceResultType CodaUpdate::GetDataInternal(ExecutionContext &, DataChunk &chunk, OperatorSourceInput &) const {
	auto &state = sink_state->Cast<CodaDMLGlobalState>();
	chunk.data[0].SetValue(0, Value::BIGINT(NumericCast<int64_t>(state.affected_count)));
	chunk.SetCardinality(1);
	return SourceResultType::FINISHED;
}

string CodaUpdate::GetName() const {
	return "CODA_UPDATE";
}

CodaDelete::CodaDelete(PhysicalPlan &physical_plan, LogicalDelete &op, CodaTableCatalogEntry &table_p,
                       idx_t row_id_index_p)
    : PhysicalOperator(physical_plan, PhysicalOperatorType::EXTENSION, op.types, op.estimated_cardinality),
      table(table_p), row_id_index(row_id_index_p), return_chunk(op.return_chunk) {
}

unique_ptr<GlobalSinkState> CodaDelete::GetGlobalSinkState(ClientContext &context) const {
	return make_uniq<CodaDMLGlobalState>(context, GetTypes());
}

unique_ptr<LocalSinkState> CodaDelete::GetLocalSinkState(ExecutionContext &context) const {
	return make_uniq<CodaDMLLocalState>(context.client, table->GetTypes());
}

SinkResultType CodaDelete::Sink(ExecutionContext &context, DataChunk &chunk, OperatorSinkInput &input) const {
	if (chunk.size() == 0) {
		return SinkResultType::NEED_MORE_INPUT;
	}
	auto client = GetClient(context.client, *table);
	auto count = client.DeleteRows(table->TableInfo(), chunk, row_id_index);
	AddAffected(input.global_state.Cast<CodaDMLGlobalState>(), count);
	return SinkResultType::NEED_MORE_INPUT;
}

SinkCombineResultType CodaDelete::Combine(ExecutionContext &, OperatorSinkCombineInput &) const {
	return SinkCombineResultType::FINISHED;
}

SourceResultType CodaDelete::GetDataInternal(ExecutionContext &, DataChunk &chunk, OperatorSourceInput &) const {
	auto &state = sink_state->Cast<CodaDMLGlobalState>();
	chunk.data[0].SetValue(0, Value::BIGINT(NumericCast<int64_t>(state.affected_count)));
	chunk.SetCardinality(1);
	return SourceResultType::FINISHED;
}

string CodaDelete::GetName() const {
	return "CODA_DELETE";
}

} // namespace duckdb
