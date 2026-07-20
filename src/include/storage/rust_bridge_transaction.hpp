#pragma once

#include "duckdb/transaction/transaction.hpp"

namespace duckdb {

void RustBridgeRejectExplicitTransaction(ClientContext &context);

class RustBridgeTransaction : public Transaction {
public:
	RustBridgeTransaction(TransactionManager &manager, ClientContext &context);
	~RustBridgeTransaction() override;

	void Start();
	void Commit();
	void Rollback();
	void MarkWrite();

private:
	bool has_writes = false;
};

} // namespace duckdb
