#pragma once

#include "rust_bridge_extension.h"

namespace duckdb {

bool RustBridgeRegisterSecret(void *loader, RustExtSecretRegistration registration, RustExtError *error);

} // namespace duckdb
