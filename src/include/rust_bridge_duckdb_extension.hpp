#pragma once

#include "duckdb/main/extension.hpp"

#ifndef RUST_BRIDGE_EXTENSION_CLASS
#error "RUST_BRIDGE_EXTENSION_CLASS must be provided by the build system"
#endif

namespace duckdb {

class RUST_BRIDGE_EXTENSION_CLASS : public Extension {
public:
	void Load(ExtensionLoader &loader) override;
	std::string Name() override;
};

} // namespace duckdb
