#include "rust_bridge_client.hpp"

#include "rust_bridge_extension.h"
#include "rust_bridge_string.hpp"
#include "duckdb/common/exception.hpp"

namespace duckdb {

RustBridgeClient::RustBridgeClient(RustExtClientConfig config_p) : config(config_p) {
}

RustBridgeCatalogResponse RustBridgeClient::ListTables() {
	RustExtCatalog catalog {};
	RustExtError error;
	if (!rust_ext_client_load_catalog(config, &catalog, &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
	return RustBridgeCatalogResponse(catalog);
}

RustBridgeScanHandle RustBridgeClient::OpenScan(const RustBridgeTableInfo &table,
                                                const RustBridgeScanRequest &request) {
	void *handle = nullptr;
	RustExtError error;
	RustExtScanRequest rust_bridge_request {BorrowRustBridgeString(request.filter),
	                                        BorrowRustBridgeString(request.order), request.limit};
	if (!rust_ext_scan_open(config, table.Raw().handle, rust_bridge_request, &handle, &error)) {
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

} // namespace duckdb
