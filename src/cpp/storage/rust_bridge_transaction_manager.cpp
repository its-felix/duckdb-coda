#include "storage/rust_bridge_transaction_manager.hpp"

#include "rust_bridge_extension.h"
#include "duckdb/common/exception.hpp"
#include "duckdb/main/client_context.hpp"

namespace duckdb {

RustBridgeTransaction::RustBridgeTransaction(TransactionManager &manager, ClientContext &context)
    : Transaction(manager, context) {
}

RustBridgeTransaction::~RustBridgeTransaction() {
}

void RustBridgeTransaction::Start() {
}

void RustBridgeTransaction::Commit() {
}

void RustBridgeTransaction::Rollback() {
	if (has_writes && !rust_ext_supports_transaction_rollback()) {
		throw NotImplementedException("%s", rust_ext_transaction_rollback_not_supported_message());
	}
}

void RustBridgeTransaction::MarkWrite() {
	has_writes = true;
}

RustBridgeTransactionManager::RustBridgeTransactionManager(AttachedDatabase &db) : TransactionManager(db) {
}

Transaction &RustBridgeTransactionManager::StartTransaction(ClientContext &context) {
	if (!rust_ext_supports_explicit_transactions() && !context.transaction.IsAutoCommit()) {
		throw NotImplementedException("%s", rust_ext_explicit_transaction_not_supported_message());
	}
	auto transaction = make_uniq<RustBridgeTransaction>(*this, context);
	transaction->Start();
	auto &result = *transaction;
	lock_guard<mutex> lock(transaction_lock);
	transactions[result] = std::move(transaction);
	return result;
}

ErrorData RustBridgeTransactionManager::CommitTransaction(ClientContext &, Transaction &transaction) {
	auto &rust_bridge_transaction = transaction.Cast<RustBridgeTransaction>();
	rust_bridge_transaction.Commit();
	lock_guard<mutex> lock(transaction_lock);
	transactions.erase(transaction);
	return ErrorData();
}

void RustBridgeTransactionManager::RollbackTransaction(Transaction &transaction) {
	auto &rust_bridge_transaction = transaction.Cast<RustBridgeTransaction>();
	try {
		rust_bridge_transaction.Rollback();
	} catch (std::exception &ex) {
		lock_guard<mutex> lock(transaction_lock);
		transactions.erase(transaction);
		throw;
	}
	lock_guard<mutex> lock(transaction_lock);
	transactions.erase(transaction);
}

void RustBridgeTransactionManager::Checkpoint(ClientContext &, bool) {
}

} // namespace duckdb
