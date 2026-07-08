#include "rust_bridge_scan.hpp"

#include "rust_bridge_extension.h"
#include "rust_bridge_string.hpp"
#include "duckdb/common/exception.hpp"
#include "duckdb/common/operator/cast_operators.hpp"
#include "duckdb/common/types/data_chunk.hpp"
#include "duckdb/common/types/value.hpp"
#include "duckdb/common/vector_size.hpp"
#include "duckdb/function/table_function.hpp"
#include "duckdb/main/client_context.hpp"
#include "duckdb/planner/expression/bound_columnref_expression.hpp"
#include "duckdb/planner/expression/bound_comparison_expression.hpp"
#include "duckdb/planner/expression/bound_constant_expression.hpp"
#include "duckdb/planner/expression/bound_function_expression.hpp"
#include "duckdb/planner/operator/logical_get.hpp"
#include "duckdb/storage/table/row_group_reorderer.hpp"
#include "storage/rust_bridge_catalog.hpp"

namespace duckdb {

class RustBridgeScanGlobalState : public GlobalTableFunctionState {
public:
	RustBridgeScanGlobalState(ClientContext &, const RustBridgeScanBindData &bind_data, TableFunctionInitInput &input)
	    : client(bind_data.table_entry.catalog.Cast<RustBridgeCatalog>().Client()), table(bind_data.table) {
		if (!input.projection_ids.empty()) {
			column_indexes.reserve(input.projection_ids.size());
			for (auto projection_id : input.projection_ids) {
				column_indexes.push_back(input.column_indexes[projection_id]);
			}
		} else {
			column_indexes = input.column_indexes;
		}
		request.filter = bind_data.pushed_query;
		request.order = bind_data.pushed_sort_by;
		request.limit = bind_data.pushed_limit == 0 ? 500 : MinValue<idx_t>(bind_data.pushed_limit, 500);
		remaining_rows = bind_data.pushed_limit;
		scan = client.OpenScan(table.Raw().id, request);
	}

	idx_t MaxThreads() const override {
		return 1;
	}

	RustBridgeClient client;
	RustBridgeTableInfo table;
	vector<ColumnIndex> column_indexes;
	RustBridgeScanRequest request;
	RustBridgeScanHandle scan;
	RustBridgeScanBatch rows;
	idx_t row_offset = 0;
	idx_t remaining_rows = 0;
	bool finished = false;
};

static unique_ptr<GlobalTableFunctionState> RustBridgeScanInitGlobal(ClientContext &context,
                                                                     TableFunctionInitInput &input) {
	if (!rust_ext_supports_explicit_transactions() && !context.transaction.IsAutoCommit()) {
		throw NotImplementedException("%s", rust_ext_explicit_transaction_not_supported_message());
	}
	auto &bind_data = input.bind_data->Cast<RustBridgeScanBindData>();
	return make_uniq<RustBridgeScanGlobalState>(context, bind_data, input);
}

static RustExtColumn BorrowRustBridgeColumn(const RustBridgeColumnInfo &column) {
	return column.Raw();
}

static bool TryExtractRustBridgeEqualityFilter(const LogicalGet &get, const RustBridgeTableInfo &table,
                                               const Expression &expr, string &query, string &description) {
	if (expr.GetExpressionType() != ExpressionType::COMPARE_EQUAL ||
	    expr.GetExpressionClass() != ExpressionClass::BOUND_COMPARISON) {
		return false;
	}

	auto &comparison = expr.Cast<BoundComparisonExpression>();
	auto left = comparison.left.get();
	auto right = comparison.right.get();
	if (left->GetExpressionClass() != ExpressionClass::BOUND_COLUMN_REF ||
	    right->GetExpressionClass() != ExpressionClass::BOUND_CONSTANT) {
		std::swap(left, right);
	}
	if (left->GetExpressionClass() != ExpressionClass::BOUND_COLUMN_REF ||
	    right->GetExpressionClass() != ExpressionClass::BOUND_CONSTANT) {
		return false;
	}

	auto &column_ref = left->Cast<BoundColumnRefExpression>();
	if (column_ref.binding.table_index != get.table_index) {
		return false;
	}
	auto &column_ids = get.GetColumnIds();
	if (column_ref.binding.column_index >= column_ids.size()) {
		return false;
	}
	auto column_index = column_ids[column_ref.binding.column_index];
	if (column_index.IsVirtualColumn()) {
		return false;
	}
	auto col_idx = column_index.GetPrimaryIndex();
	if (col_idx >= table.columns.size()) {
		return false;
	}
	auto &column = table.columns[col_idx];
	auto rust_bridge_column = BorrowRustBridgeColumn(column);
	if (!rust_ext_scan_can_filter_equality(rust_bridge_column)) {
		return false;
	}
	auto &constant = right->Cast<BoundConstantExpression>().value;
	if (constant.IsNull()) {
		return false;
	}

	RustBridgeInputValueBuffer value_buffer;
	auto value = value_buffer.Convert(constant);
	RustExtString query_result;
	RustExtString description_result;
	RustExtError error;
	auto &raw_column = column.Raw();
	if (!rust_ext_build_equality_query(raw_column.id.ptr, raw_column.id.len, raw_column.name.ptr, raw_column.name.len,
	                                   value, &query_result, &description_result, &error)) {
		rust_ext_free_error(error);
		return false;
	}
	query = TakeRustBridgeString(query_result);
	description = TakeRustBridgeString(description_result);
	return true;
}

static void RustBridgeScanPushdownComplexFilter(ClientContext &, LogicalGet &get, FunctionData *bind_data_p,
                                                vector<unique_ptr<Expression>> &filters) {
	auto &bind_data = bind_data_p->Cast<RustBridgeScanBindData>();
	if (!bind_data.pushed_query.empty()) {
		return;
	}

	for (idx_t filter_idx = 0; filter_idx < filters.size(); filter_idx++) {
		string query;
		string description;
		if (!TryExtractRustBridgeEqualityFilter(get, bind_data.table, *filters[filter_idx], query, description)) {
			continue;
		}
		bind_data.pushed_query = std::move(query);
		bind_data.pushed_query_description = std::move(description);
		filters.erase_at(filter_idx);
		return;
	}
}

static Value ScanValueToValue(const RustExtScanValue &scan_value, const LogicalType &target_type) {
	if (scan_value.is_null) {
		return Value(target_type);
	}
	auto value = RustBridgeString(scan_value.value);
	if (target_type.id() == LogicalTypeId::VARCHAR) {
		return Value(value);
	}
	try {
		switch (target_type.id()) {
		case LogicalTypeId::BOOLEAN:
			if (scan_value.value_type == RUST_EXT_JSON_BOOLEAN) {
				return Value::BOOLEAN(scan_value.bool_value);
			}
			return Value(value).DefaultCastAs(target_type);
		case LogicalTypeId::DOUBLE:
			if (scan_value.has_double_value) {
				return Value::DOUBLE(scan_value.double_value);
			}
			return Value(value).DefaultCastAs(target_type);
		default:
			return Value(value).DefaultCastAs(target_type);
		}
	} catch (...) {
		return Value(target_type);
	}
}

static void RustBridgeScan(ClientContext &, TableFunctionInput &input, DataChunk &output) {
	auto &state = input.global_state->Cast<RustBridgeScanGlobalState>();
	if (state.finished && state.row_offset >= state.rows.RowCount()) {
		return;
	}

	idx_t out_row = 0;
	while (out_row < STANDARD_VECTOR_SIZE) {
		if (state.row_offset >= state.rows.RowCount()) {
			if (state.finished) {
				break;
			}
			auto response = state.client.NextScanBatch(state.scan);
			state.rows = std::move(response);
			state.finished = state.rows.Finished();
			state.row_offset = 0;
			if (state.rows.Empty()) {
				if (state.finished) {
					break;
				}
				continue;
			}
		}

		auto &row = state.rows.Raw().rows[state.row_offset++];
		for (idx_t out_col = 0; out_col < state.column_indexes.size(); out_col++) {
			auto column_index = state.column_indexes[out_col];
			if (column_index.IsVirtualColumn()) {
				if (column_index.GetPrimaryIndex() == COLUMN_IDENTIFIER_ROW_ID) {
					output.data[out_col].SetValue(out_row, Value(RustBridgeString(row.id)));
				} else {
					output.data[out_col].SetValue(out_row, Value(output.data[out_col].GetType()));
				}
				continue;
			}
			auto col_idx = column_index.GetPrimaryIndex();
			if (col_idx >= state.table.columns.size()) {
				throw InternalException("%s", rust_ext_scan_column_index_out_of_range_message());
			}
			auto &column = state.table.columns[col_idx];
			auto rust_bridge_column = BorrowRustBridgeColumn(column);
			RustExtScanValue scan_value;
			if (!rust_ext_scan_value(rust_bridge_column, row, &scan_value)) {
				output.data[out_col].SetValue(out_row, Value(column.duckdb_type));
			} else {
				output.data[out_col].SetValue(out_row, ScanValueToValue(scan_value, column.duckdb_type));
			}
		}
		out_row++;
		if (state.remaining_rows > 0 && --state.remaining_rows == 0) {
			state.finished = true;
			break;
		}
	}
	output.SetCardinality(out_row);
}

static virtual_column_map_t RustBridgeScanGetVirtualColumns(ClientContext &, optional_ptr<FunctionData> bind_data) {
	virtual_column_map_t result;
	if (!bind_data || bind_data->Cast<RustBridgeScanBindData>().table.Supports(RUST_EXT_TABLE_ROW_ID)) {
		result.insert(
		    make_pair(COLUMN_IDENTIFIER_ROW_ID, TableColumn(rust_ext_row_id_column_name(), LogicalType::VARCHAR)));
	}
	return result;
}

static vector<column_t> RustBridgeScanGetRowIdColumns(ClientContext &, optional_ptr<FunctionData> bind_data) {
	if (bind_data && !bind_data->Cast<RustBridgeScanBindData>().table.Supports(RUST_EXT_TABLE_ROW_ID)) {
		return {};
	}
	return {COLUMN_IDENTIFIER_ROW_ID};
}

static BindInfo RustBridgeScanGetBindInfo(const optional_ptr<FunctionData> bind_data) {
	auto &rust_bridge_bind = bind_data->Cast<RustBridgeScanBindData>();
	return BindInfo(rust_bridge_bind.table_entry);
}

static void RustBridgeScanSetScanOrder(unique_ptr<RowGroupOrderOptions> order_options,
                                       optional_ptr<FunctionData> bind_data_p) {
	auto &bind_data = bind_data_p->Cast<RustBridgeScanBindData>();
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
	auto rust_bridge_column = BorrowRustBridgeColumn(column);
	RustExtString sort_by;
	if (!rust_ext_scan_sort_by(rust_bridge_column, &sort_by)) {
		return;
	}
	bind_data.pushed_sort_by = TakeRustBridgeString(sort_by);
	bind_data.pushed_limit = order_options->row_limit.GetIndex();
}

static InsertionOrderPreservingMap<string> RustBridgeScanToString(TableFunctionToStringInput &input) {
	auto &bind_data = input.bind_data->Cast<RustBridgeScanBindData>();
	InsertionOrderPreservingMap<string> result;
	if (!bind_data.pushed_query.empty()) {
		result[rust_ext_scan_query_label()] = bind_data.pushed_query_description;
	}
	if (!bind_data.pushed_sort_by.empty()) {
		result[rust_ext_scan_sort_label()] = bind_data.pushed_sort_by;
	}
	if (bind_data.pushed_limit > 0) {
		result[rust_ext_scan_limit_label()] = to_string(bind_data.pushed_limit);
	}
	return result;
}

TableFunction RustBridgeScanFunction::GetFunction() {
	TableFunction function(rust_ext_scan_function_name(), {}, RustBridgeScan, nullptr, RustBridgeScanInitGlobal);
	function.get_virtual_columns = RustBridgeScanGetVirtualColumns;
	function.get_row_id_columns = RustBridgeScanGetRowIdColumns;
	function.get_bind_info = RustBridgeScanGetBindInfo;
	function.pushdown_complex_filter = RustBridgeScanPushdownComplexFilter;
	function.set_scan_order = RustBridgeScanSetScanOrder;
	function.to_string = RustBridgeScanToString;
	function.projection_pushdown = true;
	return function;
}

} // namespace duckdb
