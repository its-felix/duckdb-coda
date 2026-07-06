#define DUCKDB_EXTENSION_MAIN

#include "coda_extension.hpp"

#include "coda_storage.hpp"
#include "duckdb/common/string_util.hpp"
#include "duckdb/main/database.hpp"
#include "duckdb/main/extension/extension_loader.hpp"
#include "duckdb/main/secret/secret.hpp"
#include "duckdb/storage/storage_extension.hpp"

namespace duckdb {

static constexpr const char *CODA_SECRET_TYPE = "coda";

static unique_ptr<BaseSecret> CreateCodaSecretFromConfig(ClientContext &, CreateSecretInput &input) {
	auto scope = input.scope;
	if (scope.empty()) {
		scope.emplace_back("coda:");
	}
	auto secret = make_uniq<KeyValueSecret>(scope, input.type, input.provider, input.name);
	for (auto &named_param : input.options) {
		auto lower_name = StringUtil::Lower(named_param.first);
		if (lower_name == "token") {
			secret->secret_map["token"] = named_param.second.ToString();
		} else {
			throw InvalidInputException("Unknown named parameter for Coda secret: %s", lower_name);
		}
	}
	secret->redact_keys = {"token"};
	return std::move(secret);
}

static void RegisterCodaSecretType(ExtensionLoader &loader) {
	SecretType secret_type;
	secret_type.name = CODA_SECRET_TYPE;
	secret_type.deserializer = KeyValueSecret::Deserialize<KeyValueSecret>;
	secret_type.default_provider = "config";
	secret_type.extension = "coda";
	loader.RegisterSecretType(secret_type);

	CreateSecretFunction config_fun = {CODA_SECRET_TYPE, "config", CreateCodaSecretFromConfig};
	config_fun.named_parameters["token"] = LogicalType::VARCHAR;
	loader.RegisterFunction(config_fun);
}

static void LoadInternal(ExtensionLoader &loader) {
	loader.SetDescription("DuckDB extension for reading and writing Coda docs");
	RegisterCodaSecretType(loader);

	auto storage = duckdb::make_shared_ptr<CodaStorageExtension>();
	StorageExtension::Register(loader.GetDatabaseInstance().config, "coda", storage);
}

void CodaExtension::Load(ExtensionLoader &loader) {
	LoadInternal(loader);
}

std::string CodaExtension::Name() {
	return "coda";
}

} // namespace duckdb

extern "C" {

DUCKDB_CPP_EXTENSION_ENTRY(coda, loader) {
	duckdb::LoadInternal(loader);
}
}

#ifndef DUCKDB_EXTENSION_MAIN
#error DUCKDB_EXTENSION_MAIN not defined
#endif
