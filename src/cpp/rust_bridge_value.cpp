#include "rust_bridge_value.hpp"

#include "rust_bridge_string.hpp"
#include "duckdb/common/exception.hpp"
#include "yyjson.hpp"

#include <cstdlib>

namespace duckdb {

using namespace duckdb_yyjson; // NOLINT

static yyjson_mut_val *RustBridgeJsonValue(yyjson_mut_doc *doc, const Value &value) {
	if (value.IsNull()) {
		return yyjson_mut_null(doc);
	}

	switch (value.type().id()) {
	case LogicalTypeId::BOOLEAN:
		return yyjson_mut_bool(doc, BooleanValue::Get(value));
	case LogicalTypeId::TINYINT:
	case LogicalTypeId::SMALLINT:
	case LogicalTypeId::INTEGER:
	case LogicalTypeId::BIGINT:
		return yyjson_mut_sint(doc, value.GetValue<int64_t>());
	case LogicalTypeId::UTINYINT:
	case LogicalTypeId::USMALLINT:
	case LogicalTypeId::UINTEGER:
	case LogicalTypeId::UBIGINT:
		return yyjson_mut_uint(doc, value.GetValue<uint64_t>());
	case LogicalTypeId::FLOAT:
	case LogicalTypeId::DOUBLE:
		return yyjson_mut_real(doc, value.GetValue<double>());
	case LogicalTypeId::DECIMAL:
	case LogicalTypeId::HUGEINT:
	case LogicalTypeId::UHUGEINT: {
		auto number = value.ToString();
		return yyjson_mut_rawncpy(doc, number.c_str(), number.size());
	}
	case LogicalTypeId::LIST: {
		auto result = yyjson_mut_arr(doc);
		for (auto &child : ListValue::GetChildren(value)) {
			yyjson_mut_arr_append(result, RustBridgeJsonValue(doc, child));
		}
		return result;
	}
	case LogicalTypeId::ARRAY: {
		auto result = yyjson_mut_arr(doc);
		for (auto &child : ArrayValue::GetChildren(value)) {
			yyjson_mut_arr_append(result, RustBridgeJsonValue(doc, child));
		}
		return result;
	}
	case LogicalTypeId::STRUCT: {
		auto result = yyjson_mut_obj(doc);
		auto &fields = StructType::GetChildTypes(value.type());
		auto &children = StructValue::GetChildren(value);
		for (idx_t index = 0; index < children.size(); index++) {
			auto &name = fields[index].first;
			auto key = yyjson_mut_strncpy(doc, name.c_str(), name.size());
			yyjson_mut_obj_add(result, key, RustBridgeJsonValue(doc, children[index]));
		}
		return result;
	}
	default: {
		auto text = value.ToString();
		return yyjson_mut_strncpy(doc, text.c_str(), text.size());
	}
	}
}

static string RustBridgeJson(const Value &value) {
	auto doc = yyjson_mut_doc_new(nullptr);
	yyjson_mut_doc_set_root(doc, RustBridgeJsonValue(doc, value));
	yyjson_write_err error;
	size_t length;
	auto json = yyjson_mut_write_opts(doc, YYJSON_WRITE_ALLOW_INVALID_UNICODE, nullptr, &length, &error);
	if (!json) {
		yyjson_mut_doc_free(doc);
		throw SerializationException("Failed to serialize DuckDB value as JSON: %s", error.msg);
	}
	string result(json, length);
	free(json);
	yyjson_mut_doc_free(doc);
	return result;
}

RustExtInputValue RustBridgeInputValueBuffer::Convert(const Value &value) {
	RustExtInputValue result {};
	if (value.IsNull()) {
		result.value_type = RUST_EXT_INPUT_NULL;
		return result;
	}

	switch (value.type().id()) {
	case LogicalTypeId::BOOLEAN:
		result.value_type = RUST_EXT_INPUT_BOOL;
		result.bool_value = BooleanValue::Get(value);
		return result;
	case LogicalTypeId::TINYINT:
	case LogicalTypeId::SMALLINT:
	case LogicalTypeId::INTEGER:
	case LogicalTypeId::BIGINT:
		result.value_type = RUST_EXT_INPUT_INT;
		result.int_value = value.GetValue<int64_t>();
		return result;
	case LogicalTypeId::UTINYINT:
	case LogicalTypeId::USMALLINT:
	case LogicalTypeId::UINTEGER:
	case LogicalTypeId::UBIGINT:
		result.value_type = RUST_EXT_INPUT_UINT;
		result.uint_value = value.GetValue<uint64_t>();
		return result;
	case LogicalTypeId::FLOAT:
	case LogicalTypeId::DOUBLE:
		result.value_type = RUST_EXT_INPUT_DOUBLE;
		result.double_value = value.GetValue<double>();
		return result;
	case LogicalTypeId::DECIMAL:
	case LogicalTypeId::HUGEINT:
	case LogicalTypeId::UHUGEINT:
		// Preserve the exact decimal representation while marking it as JSON so
		// Rust emits a JSON number instead of a quoted string.
		strings.push_back(value.ToString());
		result.value_type = RUST_EXT_INPUT_JSON;
		result.string_value = BorrowRustBridgeString(strings.back());
		return result;
	case LogicalTypeId::LIST:
	case LogicalTypeId::ARRAY:
	case LogicalTypeId::STRUCT:
		strings.push_back(RustBridgeJson(value));
		result.value_type = RUST_EXT_INPUT_JSON;
		result.string_value = BorrowRustBridgeString(strings.back());
		return result;
	default:
		strings.push_back(value.ToString());
		result.value_type = RUST_EXT_INPUT_STRING;
		result.string_value = BorrowRustBridgeString(strings.back());
		return result;
	}
}

void RustBridgeInputValueBuffer::Reserve(idx_t count) {
	strings.reserve(count);
}

Value RustBridgeDuckDBValue(const RustExtInputValue &value) {
	switch (value.value_type) {
	case RUST_EXT_INPUT_NULL:
		return Value();
	case RUST_EXT_INPUT_BOOL:
		return Value::BOOLEAN(value.bool_value);
	case RUST_EXT_INPUT_INT:
		return Value::BIGINT(value.int_value);
	case RUST_EXT_INPUT_UINT:
		return Value::UBIGINT(value.uint_value);
	case RUST_EXT_INPUT_DOUBLE:
		return Value::DOUBLE(value.double_value);
	case RUST_EXT_INPUT_JSON:
	case RUST_EXT_INPUT_STRING:
	default:
		return Value(RustBridgeString(value.string_value));
	}
}

} // namespace duckdb

