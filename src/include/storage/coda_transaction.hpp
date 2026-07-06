#pragma once

#include "duckdb/transaction/transaction.hpp"

namespace duckdb {

class CodaCatalog;

class CodaTransaction : public Transaction {
public:
	CodaTransaction(TransactionManager &manager, ClientContext &context);
	~CodaTransaction() override;

	void Start();
	void Commit();
	void Rollback();
};

} // namespace duckdb
