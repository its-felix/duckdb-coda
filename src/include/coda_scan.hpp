#pragma once

#include "coda_client.hpp"
#include "duckdb/function/table_function.hpp"

namespace duckdb {

struct CodaScanBindData : FunctionData {
  CodaScanBindData(string doc_id_p, string token_p, CodaTableInfo table_p)
      : doc_id(std::move(doc_id_p)), token(std::move(token_p)),
        table(std::move(table_p)) {}

  unique_ptr<FunctionData> Copy() const override {
    return make_uniq<CodaScanBindData>(doc_id, token, table);
  }

  bool Equals(const FunctionData &other_p) const override {
    auto &other = other_p.Cast<CodaScanBindData>();
    return doc_id == other.doc_id && table.id == other.table.id;
  }

  string doc_id;
  string token;
  CodaTableInfo table;
};

class CodaScanFunction {
public:
  static TableFunction GetFunction();
};

} // namespace duckdb
