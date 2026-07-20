#pragma once

namespace duckdb {

class TableFunction;

void ConfigureRustBridgeScanPlanning(TableFunction &function);

} // namespace duckdb
