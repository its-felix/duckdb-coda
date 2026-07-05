#include "coda_scan.hpp"

#include "duckdb/common/exception.hpp"
#include "duckdb/common/operator/cast_operators.hpp"
#include "duckdb/common/string_util.hpp"
#include "duckdb/common/types/data_chunk.hpp"
#include "duckdb/common/types/value.hpp"
#include "duckdb/common/vector_size.hpp"
#include "duckdb/function/table_function.hpp"
#include "duckdb/planner/expression/bound_columnref_expression.hpp"
#include "duckdb/planner/expression/bound_comparison_expression.hpp"
#include "duckdb/planner/expression/bound_constant_expression.hpp"
#include "duckdb/planner/expression/bound_function_expression.hpp"
#include "duckdb/planner/operator/logical_get.hpp"
#include "duckdb/storage/table/row_group_reorderer.hpp"

namespace duckdb {

class CodaScanGlobalState : public GlobalTableFunctionState {
public:
  CodaScanGlobalState(ClientContext &context, const CodaScanBindData &bind_data,
                      TableFunctionInitInput &input)
      : client(context, bind_data.doc_id, bind_data.token, bind_data.api_base),
        table(bind_data.table) {
    if (!input.projection_ids.empty()) {
      column_indexes.reserve(input.projection_ids.size());
      for (auto projection_id : input.projection_ids) {
        column_indexes.push_back(input.column_indexes[projection_id]);
      }
    } else {
      column_indexes = input.column_indexes;
    }
    request.query = bind_data.pushed_query;
    request.sort_by = bind_data.pushed_sort_by;
    request.limit = bind_data.pushed_limit == 0
                        ? 500
                        : MinValue<idx_t>(bind_data.pushed_limit, 500);
    remaining_rows = bind_data.pushed_limit;
  }

  idx_t MaxThreads() const override { return 1; }

  CodaClient client;
  CodaTableInfo table;
  vector<ColumnIndex> column_indexes;
  CodaListRowsRequest request;
  string next_page_token;
  string next_sync_token;
  vector<CodaRow> rows;
  idx_t row_offset = 0;
  idx_t remaining_rows = 0;
  bool sync_check_done = false;
  bool finished = false;
};

static unique_ptr<GlobalTableFunctionState>
CodaScanInitGlobal(ClientContext &context, TableFunctionInitInput &input) {
  auto &bind_data = input.bind_data->Cast<CodaScanBindData>();
  return make_uniq<CodaScanGlobalState>(context, bind_data, input);
}

static string CodaQueryLiteral(const Value &value) {
  if (value.IsNull()) {
    return string();
  }
  switch (value.type().id()) {
  case LogicalTypeId::BOOLEAN:
    return BooleanValue::Get(value) ? "true" : "false";
  case LogicalTypeId::TINYINT:
  case LogicalTypeId::SMALLINT:
  case LogicalTypeId::INTEGER:
  case LogicalTypeId::BIGINT:
  case LogicalTypeId::UTINYINT:
  case LogicalTypeId::USMALLINT:
  case LogicalTypeId::UINTEGER:
  case LogicalTypeId::UBIGINT:
  case LogicalTypeId::FLOAT:
  case LogicalTypeId::DOUBLE:
    return value.ToString();
  default: {
    JSONWriter writer;
    writer.SetRoot(writer.CreateString(value.ToString()));
    return writer.ToString();
  }
  }
}

static bool TryExtractCodaEqualityFilter(const LogicalGet &get,
                                         const CodaTableInfo &table,
                                         const Expression &expr, string &query,
                                         string &description) {
  if (expr.GetExpressionType() != ExpressionType::COMPARE_EQUAL ||
      expr.GetExpressionClass() != ExpressionClass::BOUND_FUNCTION) {
    return false;
  }

  auto &comparison = expr.Cast<BoundFunctionExpression>();
  auto left = &BoundComparisonExpression::Left(comparison);
  auto right = &BoundComparisonExpression::Right(comparison);
  if (left->GetExpressionClass() != ExpressionClass::BOUND_COLUMN_REF ||
      right->GetExpressionClass() != ExpressionClass::BOUND_CONSTANT) {
    std::swap(left, right);
  }
  if (left->GetExpressionClass() != ExpressionClass::BOUND_COLUMN_REF ||
      right->GetExpressionClass() != ExpressionClass::BOUND_CONSTANT) {
    return false;
  }

  auto &column_ref = left->Cast<BoundColumnRefExpression>();
  if (column_ref.Binding().table_index != get.table_index) {
    return false;
  }
  auto column_index = get.GetColumnIndex(column_ref.Binding());
  if (column_index.IsVirtualColumn()) {
    return false;
  }
  auto col_idx = column_index.GetPrimaryIndex();
  if (col_idx >= table.columns.size()) {
    return false;
  }
  auto &column = table.columns[col_idx];
  if (column.row_metadata || column.is_array) {
    return false;
  }
  auto &constant = right->Cast<BoundConstantExpression>().GetValue();
  auto literal = CodaQueryLiteral(constant);
  if (literal.empty()) {
    return false;
  }

  query = column.id + ":" + literal;
  description = column.name + " = " + constant.ToString();
  return true;
}

static void
CodaScanPushdownComplexFilter(ClientContext &, LogicalGet &get,
                              FunctionData *bind_data_p,
                              vector<unique_ptr<Expression>> &filters) {
  auto &bind_data = bind_data_p->Cast<CodaScanBindData>();
  if (!bind_data.pushed_query.empty()) {
    return;
  }

  for (idx_t filter_idx = 0; filter_idx < filters.size(); filter_idx++) {
    string query;
    string description;
    if (!TryExtractCodaEqualityFilter(
            get, bind_data.table, *filters[filter_idx], query, description)) {
      continue;
    }
    bind_data.pushed_query = std::move(query);
    bind_data.pushed_query_description = std::move(description);
    filters.erase_at(filter_idx);
    return;
  }
}

static Value CellToValue(const CodaCellValue &cell,
                         const LogicalType &target_type) {
  if (cell.type == JSONValueType::INVALID ||
      cell.type == JSONValueType::JSON_NULL) {
    return Value(target_type);
  }
  if (target_type.id() == LogicalTypeId::VARCHAR) {
    return Value(cell.value);
  }
  try {
    switch (target_type.id()) {
    case LogicalTypeId::BOOLEAN:
      if (cell.type == JSONValueType::BOOLEAN) {
        return Value::BOOLEAN(StringUtil::Lower(cell.value) == "true");
      }
      return Value(cell.value).DefaultCastAs(target_type);
    case LogicalTypeId::DOUBLE:
      return Value(cell.value).DefaultCastAs(target_type);
    default:
      return Value(cell.value).DefaultCastAs(target_type);
    }
  } catch (...) {
    return Value(target_type);
  }
}

static Value MetadataToValue(const string &value,
                             const LogicalType &target_type) {
  if (value.empty()) {
    return Value(target_type);
  }
  try {
    return Value(value).DefaultCastAs(target_type);
  } catch (...) {
    return Value(target_type);
  }
}

static void CodaScan(ClientContext &, TableFunctionInput &input,
                     DataChunk &output) {
  auto &state = input.global_state->Cast<CodaScanGlobalState>();
  if (state.finished) {
    return;
  }

  idx_t out_row = 0;
  while (out_row < STANDARD_VECTOR_SIZE) {
    if (state.row_offset >= state.rows.size()) {
      if (state.finished) {
        break;
      }
      state.request.page_token = state.next_page_token;
      if (state.next_page_token.empty() && !state.next_sync_token.empty() &&
          !state.sync_check_done) {
        state.request.sync_token = state.next_sync_token;
        state.sync_check_done = true;
      } else {
        state.request.sync_token.clear();
      }
      auto response = state.client.ListRows(state.table.id, state.request);
      state.rows = std::move(response.rows);
      state.next_page_token = std::move(response.next_page_token);
      state.next_sync_token = std::move(response.next_sync_token);
      state.row_offset = 0;
      if (state.rows.empty()) {
        if (state.next_page_token.empty()) {
          state.finished = true;
          break;
        }
        continue;
      }
    }

    auto &row = state.rows[state.row_offset++];
    if (row.deleted) {
      continue;
    }
    for (idx_t out_col = 0; out_col < state.column_indexes.size(); out_col++) {
      auto column_index = state.column_indexes[out_col];
      if (column_index.IsVirtualColumn()) {
        if (column_index.GetPrimaryIndex() == COLUMN_IDENTIFIER_ROW_ID) {
          output.data[out_col].SetValue(out_row, Value(row.id));
        } else {
          output.data[out_col].SetValue(out_row,
                                        Value(output.data[out_col].GetType()));
        }
        continue;
      }
      auto col_idx = column_index.GetPrimaryIndex();
      if (col_idx >= state.table.columns.size()) {
        throw InternalException("Coda scan column index out of range");
      }
      auto &column = state.table.columns[col_idx];
      if (column.row_metadata) {
        if (StringUtil::CIEquals(column.id, "createdAt")) {
          output.data[out_col].SetValue(
              out_row, MetadataToValue(row.created_at, column.duckdb_type));
        } else if (StringUtil::CIEquals(column.id, "updatedAt")) {
          output.data[out_col].SetValue(
              out_row, MetadataToValue(row.updated_at, column.duckdb_type));
        } else {
          output.data[out_col].SetValue(out_row, Value(column.duckdb_type));
        }
        continue;
      }
      auto entry = row.values.find(column.id);
      if (entry == row.values.end()) {
        output.data[out_col].SetValue(out_row, Value(column.duckdb_type));
      } else {
        output.data[out_col].SetValue(
            out_row, CellToValue(entry->second, column.duckdb_type));
      }
    }
    out_row++;
    if (state.remaining_rows > 0 && --state.remaining_rows == 0) {
      state.finished = true;
      break;
    }
    if (state.row_offset >= state.rows.size() &&
        state.next_page_token.empty()) {
      state.finished = state.next_sync_token.empty() || state.sync_check_done;
    }
  }
  output.SetChildCardinality(out_row);
}

static virtual_column_map_t
CodaScanGetVirtualColumns(ClientContext &, optional_ptr<FunctionData>) {
  virtual_column_map_t result;
  result.insert(make_pair(COLUMN_IDENTIFIER_ROW_ID,
                          TableColumn("rowid", LogicalType::VARCHAR)));
  return result;
}

static vector<column_t> CodaScanGetRowIdColumns(ClientContext &,
                                                optional_ptr<FunctionData>) {
  return {COLUMN_IDENTIFIER_ROW_ID};
}

static BindInfo
CodaScanGetBindInfo(const optional_ptr<FunctionData> bind_data) {
  auto &coda_bind = bind_data->Cast<CodaScanBindData>();
  return BindInfo(coda_bind.table_entry);
}

static void CodaScanSetScanOrder(unique_ptr<RowGroupOrderOptions> order_options,
                                 optional_ptr<FunctionData> bind_data_p) {
  auto &bind_data = bind_data_p->Cast<CodaScanBindData>();
  if (!order_options || !order_options->row_limit.IsValid()) {
    return;
  }
  if (order_options->order_type == OrderType::DESCENDING) {
    return;
  }

  auto col_idx = order_options->column_idx.GetPrimaryIndex();
  if (col_idx >= bind_data.table.columns.size()) {
    return;
  }
  auto &column = bind_data.table.columns[col_idx];
  if (!column.row_metadata) {
    return;
  }
  if (StringUtil::CIEquals(column.id, "createdAt")) {
    bind_data.pushed_sort_by = "createdAt";
  } else if (StringUtil::CIEquals(column.id, "updatedAt")) {
    bind_data.pushed_sort_by = "updatedAt";
  } else {
    return;
  }
  bind_data.pushed_limit = order_options->row_limit.GetIndex();
}

static InsertionOrderPreservingMap<string>
CodaScanToString(TableFunctionToStringInput &input) {
  auto &bind_data = input.bind_data->Cast<CodaScanBindData>();
  InsertionOrderPreservingMap<string> result;
  if (!bind_data.pushed_query.empty()) {
    result["Coda Query"] = bind_data.pushed_query_description;
  }
  if (!bind_data.pushed_sort_by.empty()) {
    result["Coda Sort"] = bind_data.pushed_sort_by;
  }
  if (bind_data.pushed_limit > 0) {
    result["Coda Limit"] = to_string(bind_data.pushed_limit);
  }
  return result;
}

static bool CodaScanSupportsPushdownType(const FunctionData &, idx_t) {
  return false;
}

TableFunction CodaScanFunction::GetFunction() {
  TableFunction function("coda_scan", {}, CodaScan, nullptr,
                         CodaScanInitGlobal);
  function.get_virtual_columns = CodaScanGetVirtualColumns;
  function.get_row_id_columns = CodaScanGetRowIdColumns;
  function.get_bind_info = CodaScanGetBindInfo;
  function.pushdown_complex_filter = CodaScanPushdownComplexFilter;
  function.set_scan_order = CodaScanSetScanOrder;
  function.to_string = CodaScanToString;
  function.projection_pushdown = true;
  function.filter_pushdown = true;
  function.supports_pushdown_type = CodaScanSupportsPushdownType;
  return function;
}

} // namespace duckdb
