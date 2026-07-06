#pragma once

#include "duckdb/common/reference_map.hpp"
#include "duckdb/transaction/transaction_manager.hpp"
#include "storage/coda_transaction.hpp"

namespace duckdb {

class CodaTransactionManager : public TransactionManager {
public:
	explicit CodaTransactionManager(AttachedDatabase &db);

	Transaction &StartTransaction(ClientContext &context) override;
	ErrorData CommitTransaction(ClientContext &context, Transaction &transaction) override;
	void RollbackTransaction(Transaction &transaction) override;
	void Checkpoint(ClientContext &context, bool force = false) override;

private:
	mutex transaction_lock;
	reference_map_t<Transaction, unique_ptr<CodaTransaction>> transactions;
};

} // namespace duckdb
