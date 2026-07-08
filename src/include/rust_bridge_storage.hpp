#pragma once

#include "duckdb/storage/storage_extension.hpp"

namespace duckdb {

class RustBridgeStorageExtension : public StorageExtension {
public:
	RustBridgeStorageExtension();
};

} // namespace duckdb
