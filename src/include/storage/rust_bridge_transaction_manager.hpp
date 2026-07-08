#pragma once

#include "duckdb/common/reference_map.hpp"
#include "duckdb/transaction/transaction_manager.hpp"
#include "storage/rust_bridge_transaction.hpp"

namespace duckdb {

class RustBridgeTransactionManager : public TransactionManager {
public:
	explicit RustBridgeTransactionManager(AttachedDatabase &db);

	Transaction &StartTransaction(ClientContext &context) override;
	ErrorData CommitTransaction(ClientContext &context, Transaction &transaction) override;
	void RollbackTransaction(Transaction &transaction) override;
	void Checkpoint(ClientContext &context, bool force = false) override;

private:
	mutex transaction_lock;
	reference_map_t<Transaction, unique_ptr<RustBridgeTransaction>> transactions;
};

} // namespace duckdb
