#pragma once

#include "rust_bridge_extension.h"
#include "duckdb/common/common.hpp"

namespace duckdb {

inline RustExtString BorrowRustBridgeString(const string &value) {
	return RustExtString {const_cast<char *>(value.c_str()), value.size()};
}

inline string RustBridgeString(const RustExtString &value) {
	if (!value.ptr || value.len == 0) {
		return string();
	}
	return string(value.ptr, value.len);
}

inline string TakeRustBridgeString(RustExtString value) {
	auto result = RustBridgeString(value);
	rust_ext_free_string(value);
	return result;
}

inline string RustBridgeErrorMessage(const RustExtError &error) {
	return RustBridgeString(error.message);
}

inline string TakeRustBridgeErrorMessage(RustExtError error) {
	auto result = RustBridgeErrorMessage(error);
	rust_ext_free_error(error);
	return result;
}

inline void SetRustBridgeErrorMessage(RustExtError *error, const string &message) {
	RustExtError alloc_error;
	if (!rust_ext_alloc_string(message.c_str(), message.size(), &error->message, &alloc_error)) {
		rust_ext_free_error(alloc_error);
	}
}

} // namespace duckdb
