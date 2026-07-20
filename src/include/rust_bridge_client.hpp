#pragma once

#include "rust_bridge_client_resources.hpp"
#include "rust_bridge_types.hpp"
#include "duckdb/common/types.hpp"

namespace duckdb {

class DataChunk;
class Expression;

class RustBridgeClient {
public:
	explicit RustBridgeClient(RustExtClientConfig config);

	RustBridgeCatalogResponse ListTables();
	RustBridgeScanHandle OpenScan(const RustBridgeTableInfo &table, const RustBridgeScanRequest &request);
	RustBridgeScanBatch NextScanBatch(RustBridgeScanHandle &handle);

	idx_t InsertRows(const RustBridgeTableInfo &table, DataChunk &chunk);
	idx_t UpdateRows(const RustBridgeTableInfo &table, DataChunk &chunk, const vector<PhysicalIndex> &columns,
	                 const vector<unique_ptr<Expression>> &expressions);
	idx_t DeleteRows(const RustBridgeTableInfo &table, DataChunk &chunk, idx_t row_id_index);

private:
	RustExtClientConfig config;
};

} // namespace duckdb
