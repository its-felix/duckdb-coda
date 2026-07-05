#pragma once

#include "coda_client.hpp"
#include "duckdb/function/table_function.hpp"

namespace duckdb {

class TableCatalogEntry;

struct CodaScanBindData : FunctionData {
  CodaScanBindData(TableCatalogEntry &table_entry_p, string doc_id_p,
                   string token_p, string api_base_p, CodaTableInfo table_p)
      : table_entry(table_entry_p), doc_id(std::move(doc_id_p)),
        token(std::move(token_p)), api_base(std::move(api_base_p)),
        table(std::move(table_p)) {}

  unique_ptr<FunctionData> Copy() const override {
    auto copy = make_uniq<CodaScanBindData>(table_entry, doc_id, token,
                                            api_base, table);
    copy->pushed_query = pushed_query;
    copy->pushed_query_description = pushed_query_description;
    copy->pushed_sort_by = pushed_sort_by;
    copy->pushed_limit = pushed_limit;
    return std::move(copy);
  }

  bool Equals(const FunctionData &other_p) const override {
    auto &other = other_p.Cast<CodaScanBindData>();
    return &table_entry == &other.table_entry && doc_id == other.doc_id &&
           api_base == other.api_base && table.id == other.table.id;
  }

  TableCatalogEntry &table_entry;
  string doc_id;
  string token;
  string api_base;
  CodaTableInfo table;
  string pushed_query;
  string pushed_query_description;
  string pushed_sort_by;
  idx_t pushed_limit = 0;
};

class CodaScanFunction {
public:
  static TableFunction GetFunction();
};

} // namespace duckdb
