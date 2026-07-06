#pragma once

#include "duckdb/common/common.hpp"
#include "duckdb/common/http_util.hpp"
#include "duckdb/common/types/value.hpp"
#include "duckdb/main/client_context.hpp"
#include "duckdb/planner/expression.hpp"
#include "yyjson.hpp"

namespace duckdb {

class CodaJSONDocument;

enum class CodaJSONValueType { INVALID, JSON_NULL, BOOLEAN, STRING, OTHER };

struct CodaColumnInfo {
	string id;
	string name;
	string format_type;
	bool is_array = false;
	bool calculated = false;
	bool row_metadata = false;
	LogicalType duckdb_type = LogicalType::VARCHAR;
};

struct CodaTableInfo {
	string id;
	string name;
	string table_type;
	bool is_view = false;
	vector<CodaColumnInfo> columns;
};

struct CodaCellValue {
	CodaJSONValueType type = CodaJSONValueType::INVALID;
	string value;
};

struct CodaRow {
	string id;
	string created_at;
	string updated_at;
	bool deleted = false;
	case_insensitive_map_t<CodaCellValue> values;
};

struct CodaListRowsRequest {
	string page_token;
	string query;
	string sort_by;
	string sync_token;
	idx_t limit = 500;
};

struct CodaListRowsResponse {
	vector<CodaRow> rows;
	string next_page_token;
	string next_sync_token;
};

class CodaClient {
public:
	CodaClient(ClientContext &context, string doc_id, string token, string api_base = "https://coda.io/apis/v1");

	vector<CodaTableInfo> ListTables();
	vector<CodaColumnInfo> ListColumns(const string &table_id);
	CodaListRowsResponse ListRows(const string &table_id, const CodaListRowsRequest &request);

	idx_t InsertRows(const CodaTableInfo &table, DataChunk &chunk);
	idx_t UpdateRows(const CodaTableInfo &table, DataChunk &chunk, const vector<PhysicalIndex> &columns,
	                 const vector<unique_ptr<Expression>> &expressions);
	idx_t DeleteRows(const CodaTableInfo &table, DataChunk &chunk, idx_t row_id_index);

	const string &DocId() const {
		return doc_id;
	}

private:
	unique_ptr<CodaJSONDocument> GetJSON(const string &path_and_query);
	unique_ptr<CodaJSONDocument> PostJSON(const string &path_and_query, const string &body);
	unique_ptr<CodaJSONDocument> PutJSON(const string &path_and_query, const string &body);
	unique_ptr<CodaJSONDocument> DeleteJSON(const string &path_and_query, const string &body);

	string BuildURL(const string &path_and_query) const;
	HTTPHeaders BuildHeaders() const;
	unique_ptr<HTTPParams> BuildParams(const string &url);
	unique_ptr<CodaJSONDocument> ParseResponse(const string &method, const string &url,
	                                           unique_ptr<HTTPResponse> response);
	duckdb_yyjson::yyjson_mut_val *ValueToJSON(duckdb_yyjson::yyjson_mut_doc *doc, const Value &value);
	string RowEditJSON(const CodaTableInfo &table, DataChunk &chunk, idx_t row);

private:
	ClientContext &context;
	string doc_id;
	string token;
	string api_base;
};

} // namespace duckdb
