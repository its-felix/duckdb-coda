#include "coda_scan.hpp"

#include "duckdb/common/exception.hpp"
#include "duckdb/common/operator/cast_operators.hpp"
#include "duckdb/common/string_util.hpp"
#include "duckdb/common/types/data_chunk.hpp"
#include "duckdb/common/types/value.hpp"
#include "duckdb/common/vector_size.hpp"
#include "duckdb/function/table_function.hpp"

namespace duckdb {

class CodaScanGlobalState : public GlobalTableFunctionState {
public:
  CodaScanGlobalState(ClientContext &context, const CodaScanBindData &bind_data,
                      vector<ColumnIndex> column_indexes_p)
      : client(context, bind_data.doc_id, bind_data.token, bind_data.api_base),
        table(bind_data.table), column_indexes(std::move(column_indexes_p)) {}

  idx_t MaxThreads() const override { return 1; }

  CodaClient client;
  CodaTableInfo table;
  vector<ColumnIndex> column_indexes;
  string next_page_token;
  vector<CodaRow> rows;
  idx_t row_offset = 0;
  bool finished = false;
};

static unique_ptr<GlobalTableFunctionState>
CodaScanInitGlobal(ClientContext &context, TableFunctionInitInput &input) {
  auto &bind_data = input.bind_data->Cast<CodaScanBindData>();
  return make_uniq<CodaScanGlobalState>(context, bind_data,
                                        input.column_indexes);
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
      state.rows = state.client.ListRows(state.table.id, state.next_page_token,
                                         state.next_page_token);
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
      auto entry = row.values.find(column.id);
      if (entry == row.values.end()) {
        output.data[out_col].SetValue(out_row, Value(column.duckdb_type));
      } else {
        output.data[out_col].SetValue(
            out_row, CellToValue(entry->second, column.duckdb_type));
      }
    }
    out_row++;
    if (state.row_offset >= state.rows.size() &&
        state.next_page_token.empty()) {
      state.finished = true;
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

TableFunction CodaScanFunction::GetFunction() {
  TableFunction function("coda_scan", {}, CodaScan, nullptr,
                         CodaScanInitGlobal);
  function.get_virtual_columns = CodaScanGetVirtualColumns;
  function.get_row_id_columns = CodaScanGetRowIdColumns;
  function.get_bind_info = CodaScanGetBindInfo;
  function.projection_pushdown = true;
  return function;
}

} // namespace duckdb
