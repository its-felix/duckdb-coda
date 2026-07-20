#pragma once

#include "rust_bridge_extension.h"
#include "duckdb/common/common.hpp"

namespace duckdb {

class RustBridgeCatalogResponse {
public:
	RustBridgeCatalogResponse() = default;
	explicit RustBridgeCatalogResponse(RustExtCatalog catalog_p) : catalog(catalog_p) {
	}
	~RustBridgeCatalogResponse() {
		Reset();
	}

	RustBridgeCatalogResponse(const RustBridgeCatalogResponse &) = delete;
	RustBridgeCatalogResponse &operator=(const RustBridgeCatalogResponse &) = delete;

	RustBridgeCatalogResponse(RustBridgeCatalogResponse &&other) noexcept : catalog(other.catalog) {
		other.catalog = {};
	}

	RustBridgeCatalogResponse &operator=(RustBridgeCatalogResponse &&other) noexcept {
		if (this != &other) {
			Reset();
			catalog = other.catalog;
			other.catalog = {};
		}
		return *this;
	}

	const RustExtCatalog &Raw() const {
		return catalog;
	}

	idx_t TableCount() const {
		return catalog.table_count;
	}

private:
	void Reset() {
		rust_ext_free_catalog(catalog);
		catalog = {};
	}

	RustExtCatalog catalog {};
};

class RustBridgeScanBatch {
public:
	RustBridgeScanBatch() = default;
	explicit RustBridgeScanBatch(RustExtScanBatch batch_p) : batch(batch_p) {
	}
	~RustBridgeScanBatch() {
		Reset();
	}

	RustBridgeScanBatch(const RustBridgeScanBatch &) = delete;
	RustBridgeScanBatch &operator=(const RustBridgeScanBatch &) = delete;

	RustBridgeScanBatch(RustBridgeScanBatch &&other) noexcept : batch(other.batch) {
		other.batch = {};
	}

	RustBridgeScanBatch &operator=(RustBridgeScanBatch &&other) noexcept {
		if (this != &other) {
			Reset();
			batch = other.batch;
			other.batch = {};
		}
		return *this;
	}

	const RustExtScanBatch &Raw() const {
		return batch;
	}

	idx_t RowCount() const {
		return batch.row_count;
	}

	bool Empty() const {
		return batch.row_count == 0;
	}

	bool Finished() const {
		return batch.finished;
	}

private:
	void Reset() {
		rust_ext_free_scan_batch(batch);
		batch = {};
	}

	RustExtScanBatch batch {};
};

class RustBridgeScanHandle {
public:
	RustBridgeScanHandle() = default;
	explicit RustBridgeScanHandle(void *handle_p) : handle(handle_p) {
	}
	~RustBridgeScanHandle() {
		Reset();
	}

	RustBridgeScanHandle(const RustBridgeScanHandle &) = delete;
	RustBridgeScanHandle &operator=(const RustBridgeScanHandle &) = delete;

	RustBridgeScanHandle(RustBridgeScanHandle &&other) noexcept : handle(other.handle) {
		other.handle = nullptr;
	}

	RustBridgeScanHandle &operator=(RustBridgeScanHandle &&other) noexcept {
		if (this != &other) {
			Reset();
			handle = other.handle;
			other.handle = nullptr;
		}
		return *this;
	}

	void *Raw() const {
		return handle;
	}

private:
	void Reset() {
		if (handle) {
			rust_ext_scan_close(handle);
			handle = nullptr;
		}
	}

	void *handle = nullptr;
};

class RustBridgeAttachConfig {
public:
	RustBridgeAttachConfig() = default;
	explicit RustBridgeAttachConfig(RustExtAttachConfig config_p) : config(config_p) {
	}
	~RustBridgeAttachConfig() {
		Reset();
	}

	RustBridgeAttachConfig(const RustBridgeAttachConfig &) = delete;
	RustBridgeAttachConfig &operator=(const RustBridgeAttachConfig &) = delete;

	RustBridgeAttachConfig(RustBridgeAttachConfig &&other) noexcept : config(other.config) {
		other.config = {};
	}

	RustBridgeAttachConfig &operator=(RustBridgeAttachConfig &&other) noexcept {
		if (this != &other) {
			Reset();
			config = other.config;
			other.config = {};
		}
		return *this;
	}

	const RustExtAttachConfig &Raw() const {
		return config;
	}

	RustExtClientConfig ClientConfig() const {
		return RustExtClientConfig {config.handle};
	}

	RustExtString DatabaseName() const {
		return config.database_name;
	}

private:
	void Reset() {
		rust_ext_free_attach_config(config);
		config = {};
	}

	RustExtAttachConfig config {};
};

} // namespace duckdb
