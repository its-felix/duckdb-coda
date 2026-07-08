#include "rust_bridge_client.hpp"

#include "rust_bridge_extension.h"
#include "rust_bridge_string.hpp"
#include "duckdb/common/exception.hpp"
#include "duckdb/planner/expression/bound_reference_expression.hpp"

namespace duckdb {

static vector<RustExtWriteColumn> EditableColumns(const RustBridgeTableInfo &table) {
	vector<RustExtWriteColumn> result;
	result.reserve(table.columns.size());
	for (auto &column : table.columns) {
		auto &raw_column = column.Raw();
		RustExtWriteColumn rust_bridge_column {};
		rust_bridge_column.id = raw_column.id;
		rust_bridge_column.capabilities = raw_column.capabilities;
		result.push_back(rust_bridge_column);
	}
	return result;
}

RustBridgeClient::RustBridgeClient(RustExtClientConfig config_p) : config(config_p) {
}

RustBridgeCatalogResponse RustBridgeClient::ListTables(bool include_system_columns) {
	RustExtCatalog catalog {};
	RustExtError error;
	auto request_config = config;
	request_config.include_system_columns = include_system_columns;
	if (!rust_ext_client_load_catalog(request_config, &catalog, &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
	return RustBridgeCatalogResponse(catalog);
}

RustBridgeScanHandle RustBridgeClient::OpenScan(RustExtString table_id, const RustBridgeScanRequest &request) {
	void *handle = nullptr;
	RustExtError error;
	RustExtScanRequest rust_bridge_request {
	    BorrowRustBridgeString(request.filter), BorrowRustBridgeString(request.order), request.limit};
	if (!rust_ext_scan_open(config, table_id, rust_bridge_request, &handle, &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
	return RustBridgeScanHandle(handle);
}

RustBridgeScanBatch RustBridgeClient::NextScanBatch(RustBridgeScanHandle &handle) {
	RustExtScanBatch batch {};
	RustExtError error;
	if (!rust_ext_scan_next(handle.Raw(), &batch, &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
	return RustBridgeScanBatch(batch);
}

idx_t RustBridgeClient::InsertRows(const RustBridgeTableInfo &table, DataChunk &chunk) {
	auto columns = EditableColumns(table);
	RustBridgeInputValueBuffer value_buffer;
	value_buffer.Reserve(chunk.size() * table.columns.size());
	vector<RustExtInputValue> values;
	values.reserve(chunk.size() * table.columns.size());
	for (idx_t row_idx = 0; row_idx < chunk.size(); row_idx++) {
		for (idx_t col_idx = 0; col_idx < table.columns.size(); col_idx++) {
			values.push_back(value_buffer.Convert(chunk.GetValue(col_idx, row_idx)));
		}
	}

	RustExtError error;
	size_t affected_count = 0;
	if (!rust_ext_client_insert_rows(config, table.Raw().id, columns.data(), columns.size(), values.data(),
	                                 chunk.size(), table.columns.size(), table.Raw().capabilities, &affected_count,
	                                 &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
	return affected_count;
}

idx_t RustBridgeClient::UpdateRows(const RustBridgeTableInfo &table, DataChunk &chunk,
                                   const vector<PhysicalIndex> &columns,
                                   const vector<unique_ptr<Expression>> &expressions) {
	auto row_id_index = chunk.ColumnCount() - 1;
	vector<string> row_ids;
	vector<RustExtString> rust_bridge_row_ids;
	row_ids.reserve(chunk.size());
	rust_bridge_row_ids.reserve(chunk.size());

	vector<RustExtWriteColumn> update_columns;
	update_columns.reserve(expressions.size());
	for (idx_t expr_idx = 0; expr_idx < expressions.size(); expr_idx++) {
		auto col_idx = columns[expr_idx].index;
		RustExtWriteColumn column {};
		if (col_idx >= table.columns.size()) {
			column.capabilities = RUST_EXT_COLUMN_GENERATED;
		} else {
			auto &raw_column = table.columns[col_idx].Raw();
			column.id = raw_column.id;
			column.capabilities = raw_column.capabilities;
		}
		update_columns.push_back(column);
	}

	RustBridgeInputValueBuffer value_buffer;
	value_buffer.Reserve(chunk.size() * expressions.size());
	vector<RustExtInputValue> values;
	values.reserve(chunk.size() * expressions.size());
	for (idx_t row_idx = 0; row_idx < chunk.size(); row_idx++) {
		row_ids.push_back(chunk.GetValue(row_id_index, row_idx).ToString());
		rust_bridge_row_ids.push_back(BorrowRustBridgeString(row_ids.back()));
		for (idx_t expr_idx = 0; expr_idx < expressions.size(); expr_idx++) {
			auto col_idx = columns[expr_idx].index;
			RustExtInputValue rust_bridge_value {};
			rust_bridge_value.value_type = RUST_EXT_INPUT_NULL;
			if (col_idx < table.columns.size()) {
				Value value;
				if (expressions[expr_idx]->GetExpressionType() == ExpressionType::BOUND_REF) {
					auto &binding = expressions[expr_idx]->Cast<BoundReferenceExpression>();
					value = chunk.GetValue(binding.index, row_idx);
				} else if (expressions[expr_idx]->GetExpressionType() == ExpressionType::VALUE_DEFAULT) {
					value = Value(table.columns[col_idx].duckdb_type);
				} else {
					throw NotImplementedException("%s", rust_ext_unsupported_update_expression_message());
				}
				rust_bridge_value = value_buffer.Convert(value);
			}
			values.push_back(rust_bridge_value);
		}
	}

	RustExtError error;
	size_t affected_count = 0;
	if (!rust_ext_client_update_rows(config, table.Raw().id, rust_bridge_row_ids.data(), rust_bridge_row_ids.size(),
	                                 update_columns.data(), update_columns.size(), values.data(),
	                                 table.Raw().capabilities,
	                                 &affected_count, &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
	return affected_count;
}

idx_t RustBridgeClient::DeleteRows(const RustBridgeTableInfo &table, DataChunk &chunk, idx_t row_id_index) {
	vector<string> row_ids;
	vector<RustExtString> rust_bridge_row_ids;
	row_ids.reserve(chunk.size());
	rust_bridge_row_ids.reserve(chunk.size());
	for (idx_t row_idx = 0; row_idx < chunk.size(); row_idx++) {
		row_ids.push_back(chunk.GetValue(row_id_index, row_idx).ToString());
		rust_bridge_row_ids.push_back(BorrowRustBridgeString(row_ids.back()));
	}

	RustExtError error;
	size_t affected_count = 0;
	if (!rust_ext_client_delete_rows(config, table.Raw().id, rust_bridge_row_ids.data(), rust_bridge_row_ids.size(),
	                                 &affected_count, &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
	return affected_count;
}

} // namespace duckdb
