#pragma once

#include "duckdb/execution/physical_operator.hpp"
#include "duckdb/planner/operator/logical_update.hpp"
#include "storage/coda_table.hpp"

namespace duckdb {

class CodaInsert : public PhysicalOperator {
public:
  CodaInsert(PhysicalPlan &physical_plan, LogicalOperator &op,
             CodaTableCatalogEntry &table);

  optional_ptr<CodaTableCatalogEntry> table;

  unique_ptr<GlobalSinkState>
  GetGlobalSinkState(ClientContext &context) const override;
  unique_ptr<LocalSinkState>
  GetLocalSinkState(ExecutionContext &context) const override;
  SinkResultType Sink(ExecutionContext &context, DataChunk &chunk,
                      OperatorSinkInput &input) const override;
  SinkCombineResultType Combine(ExecutionContext &context,
                                OperatorSinkCombineInput &input) const override;
  SourceResultType GetDataInternal(ExecutionContext &context, DataChunk &chunk,
                                   OperatorSourceInput &input) const override;

  bool IsSink() const override { return true; }
  bool IsSource() const override { return true; }
  bool ParallelSink() const override { return false; }
  string GetName() const override;
};

class CodaUpdate : public PhysicalOperator {
public:
  CodaUpdate(PhysicalPlan &physical_plan, LogicalUpdate &op,
             CodaTableCatalogEntry &table);

  optional_ptr<CodaTableCatalogEntry> table;
  vector<PhysicalIndex> columns;
  vector<unique_ptr<Expression>> expressions;
  bool return_chunk;

  unique_ptr<GlobalSinkState>
  GetGlobalSinkState(ClientContext &context) const override;
  unique_ptr<LocalSinkState>
  GetLocalSinkState(ExecutionContext &context) const override;
  SinkResultType Sink(ExecutionContext &context, DataChunk &chunk,
                      OperatorSinkInput &input) const override;
  SinkCombineResultType Combine(ExecutionContext &context,
                                OperatorSinkCombineInput &input) const override;
  SourceResultType GetDataInternal(ExecutionContext &context, DataChunk &chunk,
                                   OperatorSourceInput &input) const override;

  bool IsSink() const override { return true; }
  bool IsSource() const override { return true; }
  bool ParallelSink() const override { return false; }
  string GetName() const override;
};

class CodaDelete : public PhysicalOperator {
public:
  CodaDelete(PhysicalPlan &physical_plan, LogicalDelete &op,
             CodaTableCatalogEntry &table, idx_t row_id_index);

  optional_ptr<CodaTableCatalogEntry> table;
  idx_t row_id_index;
  bool return_chunk;

  unique_ptr<GlobalSinkState>
  GetGlobalSinkState(ClientContext &context) const override;
  unique_ptr<LocalSinkState>
  GetLocalSinkState(ExecutionContext &context) const override;
  SinkResultType Sink(ExecutionContext &context, DataChunk &chunk,
                      OperatorSinkInput &input) const override;
  SinkCombineResultType Combine(ExecutionContext &context,
                                OperatorSinkCombineInput &input) const override;
  SourceResultType GetDataInternal(ExecutionContext &context, DataChunk &chunk,
                                   OperatorSourceInput &input) const override;

  bool IsSink() const override { return true; }
  bool IsSource() const override { return true; }
  bool ParallelSink() const override { return false; }
  string GetName() const override;
};

} // namespace duckdb
