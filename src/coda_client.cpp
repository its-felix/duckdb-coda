#include "coda_client.hpp"

#include "duckdb/common/exception.hpp"
#include "duckdb/common/string_util.hpp"
#include "duckdb/planner/expression/bound_reference_expression.hpp"

#include <curl/curl.h>

namespace duckdb {

using namespace duckdb_yyjson; // NOLINT

static constexpr yyjson_read_flag CODA_JSON_READ_FLAGS =
    YYJSON_READ_ALLOW_INF_AND_NAN | YYJSON_READ_ALLOW_TRAILING_COMMAS | YYJSON_READ_BIGNUM_AS_RAW;
static constexpr yyjson_write_flag CODA_JSON_WRITE_FLAGS = YYJSON_WRITE_ALLOW_INF_AND_NAN;

class CodaJSONDocument {
public:
	static unique_ptr<CodaJSONDocument> Parse(string body) {
		return make_uniq<CodaJSONDocument>(std::move(body));
	}

	explicit CodaJSONDocument(string body_p) : body(std::move(body_p)) {
		yyjson_read_err error;
		doc = yyjson_read_opts(body.data(), body.size(), CODA_JSON_READ_FLAGS, nullptr, &error);
		if (error.code != YYJSON_READ_SUCCESS) {
			auto input = body.size() > 50 ? body.substr(0, 47) + "..." : body;
			input = StringUtil::Replace(input, "\r", "\\r");
			throw InvalidInputException("Failed to parse JSON in Coda API response at byte %lld: %s. "
			                            "Input: \"%s\"",
			                            error.pos, error.msg, input);
		}
	}

	~CodaJSONDocument() {
		if (doc) {
			yyjson_doc_free(doc);
		}
	}

	yyjson_val *GetRoot() const {
		return yyjson_doc_get_root(doc);
	}

private:
	string body;
	yyjson_doc *doc = nullptr;
};

static LogicalType CodaLogicalType(const string &format_type, bool is_array) {
	if (is_array) {
		return LogicalType::VARCHAR;
	}
	auto lower = StringUtil::Lower(format_type);
	if (lower == "checkbox") {
		return LogicalType::BOOLEAN;
	}
	if (lower == "number" || lower == "currency" || lower == "percent" || lower == "duration" || lower == "slider" ||
	    lower == "scale") {
		return LogicalType::DOUBLE;
	}
	return LogicalType::VARCHAR;
}

static yyjson_val *GetMember(yyjson_val *value, const string &key) {
	return yyjson_obj_getn(value, key.c_str(), key.size());
}

static CodaJSONValueType GetJSONType(yyjson_val *value) {
	if (!value) {
		return CodaJSONValueType::INVALID;
	}
	if (yyjson_is_null(value)) {
		return CodaJSONValueType::JSON_NULL;
	}
	if (yyjson_is_bool(value)) {
		return CodaJSONValueType::BOOLEAN;
	}
	if (yyjson_is_str(value)) {
		return CodaJSONValueType::STRING;
	}
	return CodaJSONValueType::OTHER;
}

static string JSONString(yyjson_val *value) {
	if (!value) {
		return string();
	}
	if (yyjson_is_str(value)) {
		return string(yyjson_get_str(value), yyjson_get_len(value));
	}
	size_t len;
	auto data = yyjson_val_write_opts(value, CODA_JSON_WRITE_FLAGS, nullptr, &len, nullptr);
	if (!data) {
		return string();
	}
	string result(data, len);
	free(data);
	return result;
}

static string JSONStringMember(yyjson_val *value, const string &key) {
	auto member = GetMember(value, key);
	return yyjson_is_str(member) ? JSONString(member) : string();
}

static yyjson_val *RequireArrayMember(yyjson_val *value, const string &key, const string &context) {
	auto member = GetMember(value, key);
	if (!yyjson_is_arr(member)) {
		throw InvalidInputException("Coda API response for %s is missing array member '%s'", context, key);
	}
	return member;
}

static string RequireStringMember(yyjson_val *value, const string &key, const string &context) {
	auto member = GetMember(value, key);
	if (!yyjson_is_str(member)) {
		throw InvalidInputException("Coda API response for %s is missing string member '%s'", context, key);
	}
	return JSONString(member);
}

static bool OptionalBooleanMember(yyjson_val *value, const string &key) {
	auto member = GetMember(value, key);
	return yyjson_is_bool(member) && yyjson_get_bool(member);
}

static size_t CurlWriteCallback(char *data, size_t size, size_t nmemb, void *userdata) {
	auto byte_count = size * nmemb;
	auto &body = *reinterpret_cast<string *>(userdata);
	body.append(data, byte_count);
	return byte_count;
}

static unique_ptr<curl_slist, void (*)(curl_slist *)> CurlHeaders(const HTTPHeaders &headers) {
	curl_slist *list = nullptr;
	for (auto &entry : headers) {
		auto header = entry.first + ": " + entry.second;
		list = curl_slist_append(list, header.c_str());
	}
	return unique_ptr<curl_slist, void (*)(curl_slist *)>(list, [](curl_slist *headers) {
		if (headers) {
			curl_slist_free_all(headers);
		}
	});
}

CodaClient::CodaClient(ClientContext &context_p, string doc_id_p, string token_p, string api_base_p)
    : context(context_p), doc_id(std::move(doc_id_p)), token(std::move(token_p)), api_base(std::move(api_base_p)) {
	if (doc_id.empty()) {
		throw InvalidInputException("Coda doc id cannot be empty");
	}
	if (token.empty()) {
		throw InvalidInputException("Coda token cannot be empty");
	}
	if (StringUtil::EndsWith(api_base, "/")) {
		api_base = api_base.substr(0, api_base.size() - 1);
	}
}

string CodaClient::BuildURL(const string &path_and_query) const {
	return api_base + path_and_query;
}

HTTPHeaders CodaClient::BuildHeaders() const {
	HTTPHeaders headers(*context.db);
	headers.Insert("Authorization", "Bearer " + token);
	headers.Insert("Content-Type", "application/json");
	headers.Insert("Accept", "application/json");
	return headers;
}

unique_ptr<HTTPParams> CodaClient::BuildParams(const string &url) {
	auto &http_util = HTTPUtil::Get(*context.db);
	return http_util.InitializeParameters(context, url);
}

unique_ptr<CodaJSONDocument> CodaClient::ParseResponse(const string &method, const string &url,
                                                       unique_ptr<HTTPResponse> response) {
	if (!response) {
		throw IOException("Coda API %s request to '%s' returned no response", method, url);
	}
	if (!response->Success()) {
		throw IOException("Coda API %s request to '%s' failed: %s", method, url, response->GetError());
	}
	return CodaJSONDocument::Parse(std::move(response->body));
}

unique_ptr<CodaJSONDocument> CodaClient::GetJSON(const string &path_and_query) {
	auto url = BuildURL(path_and_query);
	auto params = BuildParams(url);
	GetRequestInfo request(url, BuildHeaders(), *params, nullptr, nullptr);
	auto response = HTTPUtil::Get(*context.db).Request(request);
	return ParseResponse("GET", url, std::move(response));
}

unique_ptr<CodaJSONDocument> CodaClient::PostJSON(const string &path_and_query, const string &body) {
	auto url = BuildURL(path_and_query);
	auto params = BuildParams(url);
	PostRequestInfo request(url, BuildHeaders(), *params, const_data_ptr_cast(body.c_str()), body.size());
	auto response = HTTPUtil::Get(*context.db).Request(request);
	return ParseResponse("POST", url, std::move(response));
}

unique_ptr<CodaJSONDocument> CodaClient::PutJSON(const string &path_and_query, const string &body) {
	auto url = BuildURL(path_and_query);
	auto params = BuildParams(url);
	string content_type = "application/json";
	PutRequestInfo request(url, BuildHeaders(), *params, const_data_ptr_cast(body.c_str()), body.size(), content_type);
	auto response = HTTPUtil::Get(*context.db).Request(request);
	return ParseResponse("PUT", url, std::move(response));
}

unique_ptr<CodaJSONDocument> CodaClient::DeleteJSON(const string &path_and_query, const string &body) {
	auto url = BuildURL(path_and_query);
	auto params = BuildParams(url);

	auto curl = unique_ptr<CURL, void (*)(CURL *)>(curl_easy_init(), curl_easy_cleanup);
	if (!curl) {
		throw IOException("Failed to initialize curl for Coda DELETE request");
	}

	auto headers = BuildHeaders();
	for (auto &entry : params->extra_headers) {
		headers.Insert(entry.first, entry.second);
	}
	auto curl_headers = CurlHeaders(headers);

	string response_body;
	char error_buffer[CURL_ERROR_SIZE] = {0};
	curl_easy_setopt(curl.get(), CURLOPT_URL, url.c_str());
	curl_easy_setopt(curl.get(), CURLOPT_CUSTOMREQUEST, "DELETE");
	curl_easy_setopt(curl.get(), CURLOPT_HTTPHEADER, curl_headers.get());
	curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDS, body.c_str());
	curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDSIZE, body.size());
	curl_easy_setopt(curl.get(), CURLOPT_WRITEFUNCTION, CurlWriteCallback);
	curl_easy_setopt(curl.get(), CURLOPT_WRITEDATA, &response_body);
	curl_easy_setopt(curl.get(), CURLOPT_ERRORBUFFER, error_buffer);
	curl_easy_setopt(curl.get(), CURLOPT_FOLLOWLOCATION, params->follow_location ? 1L : 0L);
	curl_easy_setopt(curl.get(), CURLOPT_TIMEOUT, params->timeout);
	curl_easy_setopt(curl.get(), CURLOPT_CONNECTTIMEOUT, params->timeout);
	if (!params->http_proxy.empty()) {
		auto proxy = params->http_proxy;
		if (params->http_proxy_port != 0) {
			proxy += ":" + to_string(params->http_proxy_port);
		}
		curl_easy_setopt(curl.get(), CURLOPT_PROXY, proxy.c_str());
		if (!params->http_proxy_username.empty()) {
			curl_easy_setopt(curl.get(), CURLOPT_PROXYUSERNAME, params->http_proxy_username.c_str());
			curl_easy_setopt(curl.get(), CURLOPT_PROXYPASSWORD, params->http_proxy_password.c_str());
		}
	}
	if (params->override_verify_ssl && !params->verify_ssl) {
		curl_easy_setopt(curl.get(), CURLOPT_SSL_VERIFYPEER, 0L);
		curl_easy_setopt(curl.get(), CURLOPT_SSL_VERIFYHOST, 0L);
	}

	auto curl_result = curl_easy_perform(curl.get());
	long response_code = 0;
	curl_easy_getinfo(curl.get(), CURLINFO_RESPONSE_CODE, &response_code);

	auto response = make_uniq<HTTPResponse>(HTTPUtil::ToStatusCode(static_cast<int32_t>(response_code)));
	response->url = url;
	response->body = std::move(response_body);
	response->success = response_code >= 200 && response_code < 300;
	if (curl_result != CURLE_OK) {
		response->success = false;
		response->request_error = error_buffer[0] ? string(error_buffer) : curl_easy_strerror(curl_result);
	} else if (!response->success) {
		response->reason = response->body.empty() ? HTTPUtil::GetStatusMessage(response->status) : response->body;
	}
	return ParseResponse("DELETE", url, std::move(response));
}

vector<CodaTableInfo> CodaClient::ListTables() {
	vector<CodaTableInfo> result;
	string page_token;
	do {
		auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables?limit=100";
		if (!page_token.empty()) {
			path += "&pageToken=" + StringUtil::URLEncode(page_token);
		}
		auto doc = GetJSON(path);
		auto root = doc->GetRoot();
		auto items = RequireArrayMember(root, "items", "tables");
		size_t idx, max;
		yyjson_val *item;
		yyjson_arr_foreach(items, idx, max, item) {
			CodaTableInfo table;
			table.id = RequireStringMember(item, "id", "table");
			table.name = RequireStringMember(item, "name", "table");
			table.table_type = JSONStringMember(item, "tableType");
			if (table.table_type.empty()) {
				table.table_type = "table";
			}
			table.is_view = table.table_type != "table";
			table.columns = ListColumns(table.id);
			result.push_back(std::move(table));
		}
		page_token = JSONStringMember(root, "nextPageToken");
	} while (!page_token.empty());
	return result;
}

vector<CodaColumnInfo> CodaClient::ListColumns(const string &table_id) {
	vector<CodaColumnInfo> result;
	string page_token;
	do {
		auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" + StringUtil::URLEncode(table_id) +
		            "/columns?limit=100&visibleOnly=false";
		if (!page_token.empty()) {
			path += "&pageToken=" + StringUtil::URLEncode(page_token);
		}
		auto doc = GetJSON(path);
		auto root = doc->GetRoot();
		auto items = RequireArrayMember(root, "items", "columns");
		size_t idx, max;
		yyjson_val *item;
		yyjson_arr_foreach(items, idx, max, item) {
			CodaColumnInfo column;
			column.id = RequireStringMember(item, "id", "column");
			column.name = RequireStringMember(item, "name", "column");
			column.calculated = OptionalBooleanMember(item, "calculated");
			auto format = GetMember(item, "format");
			if (yyjson_is_obj(format)) {
				column.format_type = JSONStringMember(format, "type");
				if (column.format_type.empty()) {
					column.format_type = "text";
				}
				auto array = GetMember(format, "isArray");
				column.is_array = yyjson_is_bool(array) && yyjson_get_bool(array);
			} else {
				column.format_type = "text";
			}
			column.duckdb_type = CodaLogicalType(column.format_type, column.is_array);
			result.push_back(std::move(column));
		}
		page_token = JSONStringMember(root, "nextPageToken");
	} while (!page_token.empty());
	return result;
}

static void AppendQueryParam(string &path, const string &name, const string &value) {
	path += "&" + name + "=" + StringUtil::URLEncode(value);
}

CodaListRowsResponse CodaClient::ListRows(const string &table_id, const CodaListRowsRequest &request) {
	auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" + StringUtil::URLEncode(table_id) +
	            "/rows?valueFormat=simpleWithArrays&useColumnNames=false&"
	            "visibleOnly=false&limit=" +
	            to_string(request.limit);
	if (!request.page_token.empty()) {
		AppendQueryParam(path, "pageToken", request.page_token);
	}
	if (!request.sync_token.empty()) {
		AppendQueryParam(path, "syncToken", request.sync_token);
	}
	if (!request.query.empty()) {
		AppendQueryParam(path, "query", request.query);
	}
	if (!request.sort_by.empty()) {
		AppendQueryParam(path, "sortBy", request.sort_by);
	}

	auto doc = GetJSON(path);
	auto root = doc->GetRoot();
	CodaListRowsResponse response;
	auto items = RequireArrayMember(root, "items", "rows");
	size_t idx, max;
	yyjson_val *item;
	yyjson_arr_foreach(items, idx, max, item) {
		CodaRow row;
		row.id = RequireStringMember(item, "id", "row");
		row.deleted = OptionalBooleanMember(item, "deleted") || OptionalBooleanMember(item, "isDeleted");
		row.created_at = JSONStringMember(item, "createdAt");
		row.updated_at = JSONStringMember(item, "updatedAt");
		auto values = GetMember(item, "values");
		if (yyjson_is_obj(values)) {
			size_t value_idx, value_max;
			yyjson_val *key;
			yyjson_val *value;
			yyjson_obj_foreach(values, value_idx, value_max, key, value) {
				CodaCellValue cell;
				cell.type = GetJSONType(value);
				cell.value = JSONString(value);
				row.values[JSONString(key)] = std::move(cell);
			}
		}
		response.rows.push_back(std::move(row));
	}
	response.next_page_token = JSONStringMember(root, "nextPageToken");
	response.next_sync_token = JSONStringMember(root, "nextSyncToken");
	return response;
}

static string WriteJSON(yyjson_mut_doc *doc, yyjson_mut_val *root) {
	yyjson_mut_doc_set_root(doc, root);
	size_t len;
	auto data = yyjson_mut_val_write_opts(root, CODA_JSON_WRITE_FLAGS, nullptr, &len, nullptr);
	if (!data) {
		throw InternalException("Failed to serialize Coda JSON request body");
	}
	string result(data, len);
	free(data);
	return result;
}

static void AddJSONMember(yyjson_mut_doc *doc, yyjson_mut_val *object, const char *key, yyjson_mut_val *value) {
	if (!yyjson_mut_obj_add_val(doc, object, key, value)) {
		throw InternalException("Failed to construct Coda JSON request body");
	}
}

static void AppendJSONValue(yyjson_mut_val *array, yyjson_mut_val *value) {
	if (!yyjson_mut_arr_add_val(array, value)) {
		throw InternalException("Failed to construct Coda JSON request body");
	}
}

yyjson_mut_val *CodaClient::ValueToJSON(yyjson_mut_doc *doc, const Value &value) {
	if (value.IsNull()) {
		return yyjson_mut_null(doc);
	}
	switch (value.type().id()) {
	case LogicalTypeId::BOOLEAN:
		return yyjson_mut_bool(doc, BooleanValue::Get(value));
	case LogicalTypeId::TINYINT:
	case LogicalTypeId::SMALLINT:
	case LogicalTypeId::INTEGER:
	case LogicalTypeId::BIGINT:
		return yyjson_mut_sint(doc, value.GetValue<int64_t>());
	case LogicalTypeId::UTINYINT:
	case LogicalTypeId::USMALLINT:
	case LogicalTypeId::UINTEGER:
	case LogicalTypeId::UBIGINT:
		return yyjson_mut_uint(doc, value.GetValue<uint64_t>());
	case LogicalTypeId::FLOAT:
	case LogicalTypeId::DOUBLE:
		return yyjson_mut_real(doc, value.GetValue<double>());
	default: {
		auto string_value = value.ToString();
		return yyjson_mut_strncpy(doc, string_value.c_str(), string_value.size());
	}
	}
}

string CodaClient::RowEditJSON(const CodaTableInfo &table, DataChunk &chunk, idx_t row) {
	auto doc = yyjson_mut_doc_new(nullptr);
	if (!doc) {
		throw InternalException("Failed to construct Coda JSON request body");
	}
	auto root = yyjson_mut_obj(doc);
	auto cells = yyjson_mut_arr(doc);
	for (idx_t col_idx = 0; col_idx < table.columns.size(); col_idx++) {
		if (table.columns[col_idx].calculated || table.columns[col_idx].row_metadata) {
			continue;
		}
		auto cell = yyjson_mut_obj(doc);
		yyjson_mut_obj_add_strncpy(doc, cell, "column", table.columns[col_idx].id.c_str(),
		                           table.columns[col_idx].id.size());
		AddJSONMember(doc, cell, "value", ValueToJSON(doc, chunk.GetValue(col_idx, row)));
		AppendJSONValue(cells, cell);
	}
	AddJSONMember(doc, root, "cells", cells);
	auto result = WriteJSON(doc, root);
	yyjson_mut_doc_free(doc);
	return result;
}

idx_t CodaClient::InsertRows(const CodaTableInfo &table, DataChunk &chunk) {
	if (table.is_view) {
		throw NotImplementedException("INSERT is not supported for Coda views");
	}
	auto doc = yyjson_mut_doc_new(nullptr);
	if (!doc) {
		throw InternalException("Failed to construct Coda JSON request body");
	}
	auto root = yyjson_mut_obj(doc);
	auto rows = yyjson_mut_arr(doc);
	for (idx_t row_idx = 0; row_idx < chunk.size(); row_idx++) {
		auto row_json = RowEditJSON(table, chunk, row_idx);
		yyjson_read_err error;
		auto row_doc = yyjson_read_opts(row_json.data(), row_json.size(), CODA_JSON_READ_FLAGS, nullptr, &error);
		if (error.code != YYJSON_READ_SUCCESS) {
			throw InternalException("Failed to parse generated Coda row JSON");
		}
		auto row_value = yyjson_val_mut_copy(doc, yyjson_doc_get_root(row_doc));
		yyjson_doc_free(row_doc);
		AppendJSONValue(rows, row_value);
	}
	AddJSONMember(doc, root, "rows", rows);

	auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" + StringUtil::URLEncode(table.id) +
	            "/rows?disableParsing=false";
	auto body = WriteJSON(doc, root);
	yyjson_mut_doc_free(doc);
	PostJSON(path, body);
	return chunk.size();
}

idx_t CodaClient::UpdateRows(const CodaTableInfo &table, DataChunk &chunk, const vector<PhysicalIndex> &columns,
                             const vector<unique_ptr<Expression>> &expressions) {
	if (table.is_view) {
		throw NotImplementedException("UPDATE is not supported for Coda views");
	}
	idx_t updated = 0;
	auto row_id_index = chunk.ColumnCount() - 1;
	for (idx_t row_idx = 0; row_idx < chunk.size(); row_idx++) {
		auto row_id = chunk.GetValue(row_id_index, row_idx).ToString();
		auto doc = yyjson_mut_doc_new(nullptr);
		if (!doc) {
			throw InternalException("Failed to construct Coda JSON request body");
		}
		auto root = yyjson_mut_obj(doc);
		auto row = yyjson_mut_obj(doc);
		auto cells = yyjson_mut_arr(doc);
		for (idx_t expr_idx = 0; expr_idx < expressions.size(); expr_idx++) {
			auto col_idx = columns[expr_idx].index;
			if (col_idx >= table.columns.size() || table.columns[col_idx].calculated ||
			    table.columns[col_idx].row_metadata) {
				continue;
			}
			Value value;
			if (expressions[expr_idx]->GetExpressionType() == ExpressionType::BOUND_REF) {
				auto &binding = expressions[expr_idx]->Cast<BoundReferenceExpression>();
				value = chunk.GetValue(binding.index, row_idx);
			} else if (expressions[expr_idx]->GetExpressionType() == ExpressionType::VALUE_DEFAULT) {
				value = Value(table.columns[col_idx].duckdb_type);
			} else {
				throw NotImplementedException("Unsupported Coda UPDATE expression type");
			}
			auto cell = yyjson_mut_obj(doc);
			yyjson_mut_obj_add_strncpy(doc, cell, "column", table.columns[col_idx].id.c_str(),
			                           table.columns[col_idx].id.size());
			AddJSONMember(doc, cell, "value", ValueToJSON(doc, value));
			AppendJSONValue(cells, cell);
		}
		AddJSONMember(doc, row, "cells", cells);
		AddJSONMember(doc, root, "row", row);
		auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" + StringUtil::URLEncode(table.id) + "/rows/" +
		            StringUtil::URLEncode(row_id) + "?disableParsing=false";
		auto body = WriteJSON(doc, root);
		yyjson_mut_doc_free(doc);
		PutJSON(path, body);
		updated++;
	}
	return updated;
}

idx_t CodaClient::DeleteRows(const CodaTableInfo &table, DataChunk &chunk, idx_t row_id_index) {
	auto doc = yyjson_mut_doc_new(nullptr);
	if (!doc) {
		throw InternalException("Failed to construct Coda JSON request body");
	}
	auto root = yyjson_mut_obj(doc);
	auto row_ids = yyjson_mut_arr(doc);
	for (idx_t row_idx = 0; row_idx < chunk.size(); row_idx++) {
		auto row_id = chunk.GetValue(row_id_index, row_idx).ToString();
		AppendJSONValue(row_ids, yyjson_mut_strncpy(doc, row_id.c_str(), row_id.size()));
	}
	AddJSONMember(doc, root, "rowIds", row_ids);

	auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" + StringUtil::URLEncode(table.id) + "/rows";
	auto body = WriteJSON(doc, root);
	yyjson_mut_doc_free(doc);
	DeleteJSON(path, body);
	return chunk.size();
}

} // namespace duckdb
