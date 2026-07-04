#include "storage/coda_transaction_manager.hpp"

namespace duckdb {

CodaTransaction::CodaTransaction(TransactionManager &manager,
                                 ClientContext &context)
    : Transaction(manager, context) {}

CodaTransaction::~CodaTransaction() {}

void CodaTransaction::Start() {}

void CodaTransaction::Commit() {}

void CodaTransaction::Rollback() {}

CodaTransactionManager::CodaTransactionManager(AttachedDatabase &db)
    : TransactionManager(db) {}

Transaction &CodaTransactionManager::StartTransaction(ClientContext &context) {
  auto transaction = make_uniq<CodaTransaction>(*this, context);
  transaction->Start();
  auto &result = *transaction;
  lock_guard<mutex> lock(transaction_lock);
  transactions[result] = std::move(transaction);
  return result;
}

ErrorData CodaTransactionManager::CommitTransaction(ClientContext &,
                                                    Transaction &transaction) {
  auto &coda_transaction = transaction.Cast<CodaTransaction>();
  coda_transaction.Commit();
  lock_guard<mutex> lock(transaction_lock);
  transactions.erase(transaction);
  return ErrorData();
}

void CodaTransactionManager::RollbackTransaction(Transaction &transaction) {
  auto &coda_transaction = transaction.Cast<CodaTransaction>();
  coda_transaction.Rollback();
  lock_guard<mutex> lock(transaction_lock);
  transactions.erase(transaction);
}

void CodaTransactionManager::Checkpoint(ClientContext &, bool) {}

} // namespace duckdb
