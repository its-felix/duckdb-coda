#include "rust_bridge_duckdb_extension.hpp"
#include "rust_bridge_extension.h"
#include "duckdb/main/database.hpp"
#include "duckdb/main/extension/extension_loader.hpp"
#include "duckdb/storage/storage_extension.hpp"
#include "rust_bridge_secret.hpp"
#include "rust_bridge_string.hpp"
#include "rust_bridge_storage.hpp"

#include <exception>
namespace duckdb {

static bool HostSetDescription(void *loader_ptr, const char *description, RustExtError *err) {
	try {
		auto &loader = *reinterpret_cast<ExtensionLoader *>(loader_ptr);
		loader.SetDescription(description);
		return true;
	} catch (std::exception &ex) {
		SetRustBridgeErrorMessage(err, ex.what());
		return false;
	}
}

static bool HostRegisterStorageExtension(void *loader_ptr, const char *extension_name, RustExtError *err) {
	try {
		auto &loader = *reinterpret_cast<ExtensionLoader *>(loader_ptr);
		auto storage = duckdb::make_shared_ptr<RustBridgeStorageExtension>();
		StorageExtension::Register(loader.GetDatabaseInstance().config, extension_name, storage);
		return true;
	} catch (std::exception &ex) {
		SetRustBridgeErrorMessage(err, ex.what());
		return false;
	}
}

static void LoadFromRustBridge(ExtensionLoader &loader) {
	RustExtDuckDbHost host {
	    HostSetDescription,
	    RustBridgeRegisterSecret,
	    HostRegisterStorageExtension,
	};
	RustExtError error;
	if (!rust_ext_extension_load(&host, &loader, &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
}

void RUST_BRIDGE_EXTENSION_CLASS::Load(ExtensionLoader &loader) {
	LoadFromRustBridge(loader);
}

std::string RUST_BRIDGE_EXTENSION_CLASS::Name() {
	return rust_ext_extension_name();
}

} // namespace duckdb

#define RUST_BRIDGE_ENTRY_POINT_INNER(NAME)                                                                            \
	extern "C" DUCKDB_EXTENSION_API void NAME##_duckdb_cpp_init(void *loader_ptr)
#define RUST_BRIDGE_ENTRY_POINT(NAME) RUST_BRIDGE_ENTRY_POINT_INNER(NAME)

RUST_BRIDGE_ENTRY_POINT(RUST_BRIDGE_EXTENSION_ENTRY) {
	auto &loader = *reinterpret_cast<duckdb::ExtensionLoader *>(loader_ptr);
	duckdb::LoadFromRustBridge(loader);
}
