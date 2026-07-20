#pragma once

#include "rust_bridge_extension.h"
#include "duckdb/common/common.hpp"
#include "duckdb/common/types/value.hpp"

namespace duckdb {

class RustBridgeInputValueBuffer {
public:
	RustExtInputValue Convert(const Value &value);
	void Reserve(idx_t count);

private:
	vector<string> strings;
};

Value RustBridgeDuckDBValue(const RustExtInputValue &value);

} // namespace duckdb
