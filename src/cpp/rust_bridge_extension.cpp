#include "rust_bridge_duckdb_extension.hpp"
#include "rust_bridge_extension.h"
#include "duckdb/main/database.hpp"
#include "duckdb/main/extension/extension_loader.hpp"
#include "duckdb/main/secret/secret.hpp"
#include "duckdb/storage/storage_extension.hpp"
#include "rust_bridge_string.hpp"
#include "rust_bridge_storage.hpp"

#include <exception>
#include <mutex>
#include <unordered_map>

namespace duckdb {

struct RustBridgeSecretConfig {
	string provider;
	string extension;
	string default_scope;
	string secret_key;
};

static mutex &SecretConfigLock() {
	static mutex lock;
	return lock;
}

static unordered_map<string, RustBridgeSecretConfig> &SecretConfigs() {
	static unordered_map<string, RustBridgeSecretConfig> configs;
	return configs;
}

static string SecretConfigMissingMessage(const string &secret_type) {
	RustExtString message;
	RustExtError error;
	if (!rust_ext_secret_config_missing_message(secret_type.c_str(), secret_type.size(), &message, &error)) {
		return TakeRustBridgeErrorMessage(error);
	}
	return TakeRustBridgeString(message);
}

static string UnknownSecretParameterMessage(const string &secret_type, const string &parameter_name) {
	RustExtString message;
	RustExtError error;
	if (!rust_ext_secret_unknown_parameter_message(secret_type.c_str(), secret_type.size(), parameter_name.c_str(),
	                                               parameter_name.size(), &message, &error)) {
		return TakeRustBridgeErrorMessage(error);
	}
	return TakeRustBridgeString(message);
}

static string CanonicalSecretParameterName(const string &secret_key, const string &parameter_name) {
	RustExtString canonical;
	RustExtError error;
	if (!rust_ext_secret_canonical_parameter_name(secret_key.c_str(), secret_key.size(), parameter_name.c_str(),
	                                              parameter_name.size(), &canonical, &error)) {
		throw InvalidInputException("%s", TakeRustBridgeErrorMessage(error));
	}
	return TakeRustBridgeString(canonical);
}

static unique_ptr<BaseSecret> CreateConfigSecret(ClientContext &, CreateSecretInput &input) {
	RustBridgeSecretConfig config;
	{
		lock_guard<mutex> lock(SecretConfigLock());
		auto entry = SecretConfigs().find(input.type);
		if (entry == SecretConfigs().end()) {
			throw InvalidInputException("%s", SecretConfigMissingMessage(input.type));
		}
		config = entry->second;
	}

	auto scope = input.scope;
	if (scope.empty()) {
		scope.emplace_back(config.default_scope);
	}
	auto secret = make_uniq<KeyValueSecret>(scope, input.type, input.provider, input.name);
	for (auto &named_param : input.options) {
		auto canonical_name = CanonicalSecretParameterName(config.secret_key, named_param.first);
		if (!canonical_name.empty()) {
			secret->secret_map[canonical_name] = named_param.second.ToString();
		} else {
			throw InvalidInputException("%s", UnknownSecretParameterMessage(input.type, named_param.first));
		}
	}
	secret->redact_keys = {config.secret_key};
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

static bool HostRegisterConfigSecret(void *loader_ptr, const char *secret_type, const char *provider,
                                     const char *extension, const char *default_scope, const char *secret_key,
                                     RustExtError *err) {
	try {
		auto &loader = *reinterpret_cast<ExtensionLoader *>(loader_ptr);
		{
			lock_guard<mutex> lock(SecretConfigLock());
			SecretConfigs()[secret_type] = RustBridgeSecretConfig {provider, extension, default_scope, secret_key};
		}

		SecretType type;
		type.name = secret_type;
		type.deserializer = KeyValueSecret::Deserialize<KeyValueSecret>;
		type.default_provider = provider;
		type.extension = extension;
		loader.RegisterSecretType(type);

		CreateSecretFunction config_fun = {secret_type, provider, CreateConfigSecret};
		config_fun.named_parameters[secret_key] = LogicalType::VARCHAR;
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
	    HostRegisterConfigSecret,
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
