#pragma once

#include "duckdb/storage/storage_extension.hpp"

namespace duckdb {

class CodaStorageExtension : public StorageExtension {
public:
  CodaStorageExtension();
};

} // namespace duckdb
