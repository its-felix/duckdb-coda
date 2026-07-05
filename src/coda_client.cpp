#include "coda_client.hpp"

#include "duckdb/common/exception.hpp"
#include "duckdb/common/string_util.hpp"
#include "duckdb/planner/expression/bound_reference_expression.hpp"

#include <curl/curl.h>

namespace duckdb {

static LogicalType CodaLogicalType(const string &format_type, bool is_array) {
  if (is_array) {
    return LogicalType::VARCHAR;
  }
  auto lower = StringUtil::Lower(format_type);
  if (lower == "checkbox") {
    return LogicalType::BOOLEAN;
  }
  if (lower == "number" || lower == "currency" || lower == "percent" ||
      lower == "duration" || lower == "slider" || lower == "scale") {
    return LogicalType::DOUBLE;
  }
  return LogicalType::VARCHAR;
}

static JSONValue RequireArrayMember(JSONValue value, const string &key,
                                    const string &context) {
  auto member = value.GetMember(key);
  if (!member.IsArray()) {
    throw InvalidInputException(
        "Coda API response for %s is missing array member '%s'", context, key);
  }
  return member;
}

static string RequireStringMember(JSONValue value, const string &key,
                                  const string &context) {
  auto member = value.GetMember(key);
  if (!member.IsString()) {
    throw InvalidInputException(
        "Coda API response for %s is missing string member '%s'", context, key);
  }
  return member.GetString();
}

static bool OptionalBooleanMember(JSONValue value, const string &key) {
  auto member = value.GetMember(key);
  return member.IsValid() && member.GetType() == JSONValueType::BOOLEAN &&
         member.GetBoolean();
}

static size_t CurlWriteCallback(char *data, size_t size, size_t nmemb,
                                void *userdata) {
  auto byte_count = size * nmemb;
  auto &body = *reinterpret_cast<string *>(userdata);
  body.append(data, byte_count);
  return byte_count;
}

static unique_ptr<curl_slist, void (*)(curl_slist *)>
CurlHeaders(const HTTPHeaders &headers) {
  curl_slist *list = nullptr;
  for (auto &entry : headers) {
    auto header = entry.first + ": " + entry.second;
    list = curl_slist_append(list, header.c_str());
  }
  return unique_ptr<curl_slist, void (*)(curl_slist *)>(
      list, [](curl_slist *headers) {
        if (headers) {
          curl_slist_free_all(headers);
        }
      });
}

CodaClient::CodaClient(ClientContext &context_p, string doc_id_p,
                       string token_p, string api_base_p)
    : context(context_p), doc_id(std::move(doc_id_p)),
      token(std::move(token_p)), api_base(std::move(api_base_p)) {
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

unique_ptr<JSONDocument>
CodaClient::ParseResponse(const string &method, const string &url,
                          unique_ptr<HTTPResponse> response) {
  if (!response) {
    throw IOException("Coda API %s request to '%s' returned no response",
                      method, url);
  }
  if (!response->Success()) {
    throw IOException("Coda API %s request to '%s' failed: %s", method, url,
                      response->GetError());
  }
  return JSONDocument::Parse(response->body.c_str(), response->body.size());
}

unique_ptr<JSONDocument> CodaClient::GetJSON(const string &path_and_query) {
  auto url = BuildURL(path_and_query);
  auto params = BuildParams(url);
  GetRequestInfo request(url, BuildHeaders(), *params, nullptr, nullptr);
  auto response = HTTPUtil::Get(*context.db).Request(request);
  return ParseResponse("GET", url, std::move(response));
}

unique_ptr<JSONDocument> CodaClient::PostJSON(const string &path_and_query,
                                              const string &body) {
  auto url = BuildURL(path_and_query);
  auto params = BuildParams(url);
  PostRequestInfo request(url, BuildHeaders(), *params,
                          const_data_ptr_cast(body.c_str()), body.size());
  auto response = HTTPUtil::Get(*context.db).Request(request);
  return ParseResponse("POST", url, std::move(response));
}

unique_ptr<JSONDocument> CodaClient::PutJSON(const string &path_and_query,
                                             const string &body) {
  auto url = BuildURL(path_and_query);
  auto params = BuildParams(url);
  string content_type = "application/json";
  PutRequestInfo request(url, BuildHeaders(), *params,
                         const_data_ptr_cast(body.c_str()), body.size(),
                         content_type);
  auto response = HTTPUtil::Get(*context.db).Request(request);
  return ParseResponse("PUT", url, std::move(response));
}

unique_ptr<JSONDocument> CodaClient::DeleteJSON(const string &path_and_query,
                                                const string &body) {
  auto url = BuildURL(path_and_query);
  auto params = BuildParams(url);

  auto curl = unique_ptr<CURL, void (*)(CURL *)>(curl_easy_init(),
                                                curl_easy_cleanup);
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
  curl_easy_setopt(curl.get(), CURLOPT_FOLLOWLOCATION,
                   params->follow_location ? 1L : 0L);
  curl_easy_setopt(curl.get(), CURLOPT_TIMEOUT, params->timeout);
  curl_easy_setopt(curl.get(), CURLOPT_CONNECTTIMEOUT, params->timeout);
  if (!params->http_proxy.empty()) {
    auto proxy = params->http_proxy;
    if (params->http_proxy_port != 0) {
      proxy += ":" + to_string(params->http_proxy_port);
    }
    curl_easy_setopt(curl.get(), CURLOPT_PROXY, proxy.c_str());
    if (!params->http_proxy_username.empty()) {
      curl_easy_setopt(curl.get(), CURLOPT_PROXYUSERNAME,
                       params->http_proxy_username.c_str());
      curl_easy_setopt(curl.get(), CURLOPT_PROXYPASSWORD,
                       params->http_proxy_password.c_str());
    }
  }
  if (params->override_verify_ssl && !params->verify_ssl) {
    curl_easy_setopt(curl.get(), CURLOPT_SSL_VERIFYPEER, 0L);
    curl_easy_setopt(curl.get(), CURLOPT_SSL_VERIFYHOST, 0L);
  }

  auto curl_result = curl_easy_perform(curl.get());
  long response_code = 0;
  curl_easy_getinfo(curl.get(), CURLINFO_RESPONSE_CODE, &response_code);

  auto response = make_uniq<HTTPResponse>(
      HTTPUtil::ToStatusCode(static_cast<int32_t>(response_code)));
  response->url = url;
  response->body = std::move(response_body);
  response->success = response_code >= 200 && response_code < 300;
  if (curl_result != CURLE_OK) {
    response->success = false;
    response->request_error =
        error_buffer[0] ? string(error_buffer) : curl_easy_strerror(curl_result);
  } else if (!response->success) {
    response->reason = response->body.empty()
                           ? HTTPUtil::GetStatusMessage(response->status)
                           : response->body;
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
    RequireArrayMember(root, "items", "tables")
        .IterateArray([&](JSONValue item) {
          CodaTableInfo table;
          table.id = RequireStringMember(item, "id", "table");
          table.name = RequireStringMember(item, "name", "table");
          auto table_type = item.GetMember("tableType");
          table.table_type =
              table_type.IsString() ? table_type.GetString() : "table";
          table.is_view = table.table_type != "table";
          table.columns = ListColumns(table.id);
          result.push_back(std::move(table));
        });
    auto next = root.GetMember("nextPageToken");
    page_token = next.IsString() ? next.GetString() : string();
  } while (!page_token.empty());
  return result;
}

vector<CodaColumnInfo> CodaClient::ListColumns(const string &table_id) {
  vector<CodaColumnInfo> result;
  string page_token;
  do {
    auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" +
                StringUtil::URLEncode(table_id) +
                "/columns?limit=100&visibleOnly=false";
    if (!page_token.empty()) {
      path += "&pageToken=" + StringUtil::URLEncode(page_token);
    }
    auto doc = GetJSON(path);
    auto root = doc->GetRoot();
    RequireArrayMember(root, "items", "columns")
        .IterateArray([&](JSONValue item) {
          CodaColumnInfo column;
          column.id = RequireStringMember(item, "id", "column");
          column.name = RequireStringMember(item, "name", "column");
          column.calculated = OptionalBooleanMember(item, "calculated");
          auto format = item.GetMember("format");
          if (format.IsObject()) {
            auto type = format.GetMember("type");
            column.format_type = type.IsString() ? type.GetString() : "text";
            auto array = format.GetMember("isArray");
            column.is_array = array.IsValid() &&
                              array.GetType() == JSONValueType::BOOLEAN &&
                              array.GetBoolean();
          } else {
            column.format_type = "text";
          }
          column.duckdb_type =
              CodaLogicalType(column.format_type, column.is_array);
          result.push_back(std::move(column));
        });
    auto next = root.GetMember("nextPageToken");
    page_token = next.IsString() ? next.GetString() : string();
  } while (!page_token.empty());
  return result;
}

static void AppendQueryParam(string &path, const string &name,
                             const string &value) {
  path += "&" + name + "=" + StringUtil::URLEncode(value);
}

CodaListRowsResponse
CodaClient::ListRows(const string &table_id,
                     const CodaListRowsRequest &request) {
  auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" +
              StringUtil::URLEncode(table_id) +
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
  RequireArrayMember(root, "items", "rows").IterateArray([&](JSONValue item) {
    CodaRow row;
    row.id = RequireStringMember(item, "id", "row");
    row.deleted = OptionalBooleanMember(item, "deleted") ||
                  OptionalBooleanMember(item, "isDeleted");
    auto created_at = item.GetMember("createdAt");
    if (created_at.IsString()) {
      row.created_at = created_at.GetString();
    }
    auto updated_at = item.GetMember("updatedAt");
    if (updated_at.IsString()) {
      row.updated_at = updated_at.GetString();
    }
    auto values = item.GetMember("values");
    if (values.IsObject()) {
      values.IterateObject([&](const string &key, JSONValue value) {
        CodaCellValue cell;
        cell.type = value.GetType();
        if (value.IsString()) {
          cell.value = value.GetString();
        } else {
          cell.value = value.ToString();
        }
        row.values[key] = std::move(cell);
      });
    }
    response.rows.push_back(std::move(row));
  });
  auto next = root.GetMember("nextPageToken");
  response.next_page_token = next.IsString() ? next.GetString() : string();
  auto next_sync = root.GetMember("nextSyncToken");
  response.next_sync_token =
      next_sync.IsString() ? next_sync.GetString() : string();
  return response;
}

JSONMutableValue CodaClient::ValueToJSON(JSONWriter &writer,
                                         const Value &value) {
  if (value.IsNull()) {
    return writer.CreateNull();
  }
  switch (value.type().id()) {
  case LogicalTypeId::BOOLEAN:
    return writer.CreateBoolean(BooleanValue::Get(value));
  case LogicalTypeId::TINYINT:
  case LogicalTypeId::SMALLINT:
  case LogicalTypeId::INTEGER:
  case LogicalTypeId::BIGINT:
    return writer.CreateSignedInteger(value.GetValue<int64_t>());
  case LogicalTypeId::UTINYINT:
  case LogicalTypeId::USMALLINT:
  case LogicalTypeId::UINTEGER:
  case LogicalTypeId::UBIGINT:
    return writer.CreateUnsignedInteger(value.GetValue<uint64_t>());
  case LogicalTypeId::FLOAT:
  case LogicalTypeId::DOUBLE:
    return writer.CreateDouble(value.GetValue<double>());
  default:
    return writer.CreateString(value.ToString());
  }
}

string CodaClient::RowEditJSON(const CodaTableInfo &table, DataChunk &chunk,
                               idx_t row) {
  JSONWriter writer;
  auto root = writer.CreateObject();
  auto cells = writer.CreateArray();
  for (idx_t col_idx = 0; col_idx < table.columns.size(); col_idx++) {
    if (table.columns[col_idx].calculated ||
        table.columns[col_idx].row_metadata) {
      continue;
    }
    auto cell = writer.CreateObject();
    cell.AddString("column", table.columns[col_idx].id);
    cell.Add("value", ValueToJSON(writer, chunk.GetValue(col_idx, row)));
    cells.Append(std::move(cell));
  }
  root.Add("cells", std::move(cells));
  writer.SetRoot(std::move(root));
  return writer.ToString();
}

idx_t CodaClient::InsertRows(const CodaTableInfo &table, DataChunk &chunk) {
  if (table.is_view) {
    throw NotImplementedException("INSERT is not supported for Coda views");
  }
  JSONWriter writer;
  auto root = writer.CreateObject();
  auto rows = writer.CreateArray();
  for (idx_t row_idx = 0; row_idx < chunk.size(); row_idx++) {
    auto row_json = RowEditJSON(table, chunk, row_idx);
    auto row_doc = JSONDocument::Parse(row_json.c_str(), row_json.size());
    rows.Append(writer.CreateCopy(row_doc->GetRoot()));
  }
  root.Add("rows", std::move(rows));
  writer.SetRoot(std::move(root));

  auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" +
              StringUtil::URLEncode(table.id) + "/rows?disableParsing=false";
  PostJSON(path, writer.ToString());
  return chunk.size();
}

idx_t CodaClient::UpdateRows(
    const CodaTableInfo &table, DataChunk &chunk,
    const vector<PhysicalIndex> &columns,
    const vector<unique_ptr<Expression>> &expressions) {
  if (table.is_view) {
    throw NotImplementedException("UPDATE is not supported for Coda views");
  }
  idx_t updated = 0;
  auto row_id_index = chunk.ColumnCount() - 1;
  for (idx_t row_idx = 0; row_idx < chunk.size(); row_idx++) {
    auto row_id = chunk.GetValue(row_id_index, row_idx).ToString();
    JSONWriter writer;
    auto root = writer.CreateObject();
    auto row = writer.CreateObject();
    auto cells = writer.CreateArray();
    for (idx_t expr_idx = 0; expr_idx < expressions.size(); expr_idx++) {
      auto col_idx = columns[expr_idx].index;
      if (col_idx >= table.columns.size() ||
          table.columns[col_idx].calculated ||
          table.columns[col_idx].row_metadata) {
        continue;
      }
      Value value;
      if (expressions[expr_idx]->GetExpressionType() ==
          ExpressionType::BOUND_REF) {
        auto &binding = expressions[expr_idx]->Cast<BoundReferenceExpression>();
        value = chunk.GetValue(binding.Index(), row_idx);
      } else if (expressions[expr_idx]->GetExpressionType() ==
                 ExpressionType::VALUE_DEFAULT) {
        value = Value(table.columns[col_idx].duckdb_type);
      } else {
        throw NotImplementedException(
            "Unsupported Coda UPDATE expression type");
      }
      auto cell = writer.CreateObject();
      cell.AddString("column", table.columns[col_idx].id);
      cell.Add("value", ValueToJSON(writer, value));
      cells.Append(std::move(cell));
    }
    row.Add("cells", std::move(cells));
    root.Add("row", std::move(row));
    writer.SetRoot(std::move(root));
    auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" +
                StringUtil::URLEncode(table.id) + "/rows/" +
                StringUtil::URLEncode(row_id) + "?disableParsing=false";
    PutJSON(path, writer.ToString());
    updated++;
  }
  return updated;
}

idx_t CodaClient::DeleteRows(const CodaTableInfo &table, DataChunk &chunk,
                             idx_t row_id_index) {
  JSONWriter writer;
  auto root = writer.CreateObject();
  auto row_ids = writer.CreateArray();
  for (idx_t row_idx = 0; row_idx < chunk.size(); row_idx++) {
    auto row_id = chunk.GetValue(row_id_index, row_idx).ToString();
    row_ids.Append(writer.CreateString(row_id));
  }
  root.Add("rowIds", std::move(row_ids));
  writer.SetRoot(std::move(root));

  auto path = "/docs/" + StringUtil::URLEncode(doc_id) + "/tables/" +
              StringUtil::URLEncode(table.id) + "/rows";
  DeleteJSON(path, writer.ToString());
  return chunk.size();
}

} // namespace duckdb
