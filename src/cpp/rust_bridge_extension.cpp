#include "rust_bridge_duckdb_extension.hpp"
#include "rust_bridge_extension.h"
#include "duckdb/main/database.hpp"
#include "duckdb/main/extension/extension_loader.hpp"
#include "duckdb/main/secret/secret.hpp"
#include "duckdb/storage/storage_extension.hpp"
#include "rust_bridge_string.hpp"
#include "rust_bridge_storage.hpp"
#include "rust_bridge_value.hpp"

#include <exception>
namespace duckdb {

class RustBridgeSecretResult {
public:
	~RustBridgeSecretResult() {
		rust_ext_free_secret(value);
	}
	RustExtSecretCreateResult value {};
};

static unique_ptr<BaseSecret> CreateRustBridgeSecret(ClientContext &, CreateSecretInput &input) {
	vector<RustExtString> scope;
	scope.reserve(input.scope.size());
	for (auto &entry : input.scope) {
		scope.push_back(BorrowRustBridgeString(entry));
	}
	RustBridgeInputValueBuffer value_buffer;
	value_buffer.Reserve(input.options.size());
	vector<RustExtNamedValue> options;
	options.reserve(input.options.size());
	for (auto &entry : input.options) {
		options.push_back(RustExtNamedValue {BorrowRustBridgeString(entry.first), value_buffer.Convert(entry.second)});
	}
	RustExtSecretCreateInput rust_input {BorrowRustBridgeString(input.type),
	                                     BorrowRustBridgeString(input.provider),
	                                     BorrowRustBridgeString(input.name),
	                                     scope.data(),
	                                     scope.size(),
	                                     options.data(),
	                                     options.size()};
	RustBridgeSecretResult result;
	RustExtError error;
	if (!rust_ext_create_secret(rust_input, &result.value, &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
	vector<string> resolved_scope;
	resolved_scope.reserve(result.value.scope_count);
	for (idx_t idx = 0; idx < result.value.scope_count; idx++) {
		resolved_scope.push_back(RustBridgeString(result.value.scope[idx]));
	}
	auto secret = make_uniq<KeyValueSecret>(resolved_scope, input.type, input.provider, input.name);
	for (idx_t idx = 0; idx < result.value.entry_count; idx++) {
		auto &entry = result.value.entries[idx];
		secret->secret_map[RustBridgeString(entry.name)] = RustBridgeDuckDBValue(entry.value);
	}
	for (idx_t idx = 0; idx < result.value.redact_key_count; idx++) {
		secret->redact_keys.insert(RustBridgeString(result.value.redact_keys[idx]));
	}
	return std::move(secret);
}

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

static bool HostRegisterSecret(void *loader_ptr, RustExtSecretRegistration registration, RustExtError *err) {
	try {
		auto &loader = *reinterpret_cast<ExtensionLoader *>(loader_ptr);
		auto secret_type = RustBridgeString(registration.secret_type);
		auto provider = RustBridgeString(registration.provider);

		SecretType type;
		type.name = secret_type;
		type.deserializer = KeyValueSecret::Deserialize<KeyValueSecret>;
		type.default_provider = provider;
		type.extension = RustBridgeString(registration.extension);
		loader.RegisterSecretType(type);

		CreateSecretFunction config_fun = {secret_type, provider, CreateRustBridgeSecret};
		for (idx_t idx = 0; idx < registration.parameter_count; idx++) {
			auto &parameter = registration.parameters[idx];
			config_fun.named_parameters[RustBridgeString(parameter.name)] =
			    UnboundType::TryParseAndDefaultBind(RustBridgeString(parameter.logical_type));
		}
		loader.RegisterFunction(config_fun);
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
	    HostRegisterSecret,
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
