#pragma once

#include "rust_bridge_extension.h"
#include "rust_bridge_string.hpp"
#include "duckdb/common/common.hpp"
#include "duckdb/common/types/value.hpp"

namespace duckdb {

class RustBridgeInputValueBuffer {
public:
	RustExtInputValue Convert(const Value &value) {
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
		default:
			strings.push_back(value.ToString());
			result.value_type = RUST_EXT_INPUT_STRING;
			result.string_value = BorrowRustBridgeString(strings.back());
			return result;
		}
	}

	void Reserve(idx_t count) {
		strings.reserve(count);
	}

private:
	vector<string> strings;
};

inline Value RustBridgeDuckDBValue(const RustExtInputValue &value) {
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
	case RUST_EXT_INPUT_STRING:
	default:
		return Value(RustBridgeString(value.string_value));
	}
}

} // namespace duckdb
