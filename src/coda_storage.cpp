#include "coda_storage.hpp"

#include "duckdb/common/string_util.hpp"
#include "duckdb/main/secret/secret_manager.hpp"
#include "duckdb/storage/storage_extension.hpp"
#include "storage/coda_catalog.hpp"
#include "storage/coda_transaction_manager.hpp"

namespace duckdb {

static string GetAttachOption(AttachOptions &attach_options,
                              const string &name) {
  auto entry = attach_options.options.find(name);
  if (entry == attach_options.options.end() || entry->second.IsNull()) {
    return string();
  }
  return entry->second.ToString();
}

static string ResolveToken(ClientContext &context, const string &doc_id,
                           AttachOptions &attach_options) {
  auto token = GetAttachOption(attach_options, "token");
  if (!token.empty()) {
    return token;
  }

  auto &secret_manager = SecretManager::Get(context);
  auto transaction = CatalogTransaction::GetSystemCatalogTransaction(context);
  auto match =
      secret_manager.LookupSecret(transaction, "coda:" + doc_id, "coda");
  if (!match.HasMatch()) {
    match = secret_manager.LookupSecret(transaction, "coda:", "coda");
  }
  if (match.HasMatch()) {
    auto &kv =
        dynamic_cast<const KeyValueSecret &>(*match.secret_entry->secret);
    return kv.TryGetValue("token", true).ToString();
  }
  throw InvalidInputException(
      "Coda token not provided. Pass TOKEN in ATTACH or create a Coda secret.");
}

static unique_ptr<Catalog> CodaAttach(optional_ptr<StorageExtensionInfo>,
                                      ClientContext &context,
                                      AttachedDatabase &db, const string &,
                                      AttachInfo &info,
                                      AttachOptions &attach_options) {
  auto doc_id = info.path;
  if (StringUtil::StartsWith(doc_id, "coda:")) {
    doc_id = doc_id.substr(5);
  }
  auto token = ResolveToken(context, doc_id, attach_options);
  auto api_base = GetAttachOption(attach_options, "api_base");
  if (api_base.empty()) {
    api_base = "https://coda.io/apis/v1";
  }
  return make_uniq<CodaCatalog>(db, context, doc_id, token, api_base);
}

static unique_ptr<TransactionManager>
CodaCreateTransactionManager(optional_ptr<StorageExtensionInfo>,
                             AttachedDatabase &db, Catalog &) {
  return make_uniq<CodaTransactionManager>(db);
}

CodaStorageExtension::CodaStorageExtension() {
  attach = CodaAttach;
  create_transaction_manager = CodaCreateTransactionManager;
}

} // namespace duckdb
