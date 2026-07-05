cmake_minimum_required(VERSION 3.5...3.29)

set(TARGET_NAME coda)
set(EXTENSION_NAME ${TARGET_NAME}_extension)

project(${TARGET_NAME})

include_directories(src/include)
find_package(CURL REQUIRED)

set(EXTENSION_SOURCES
    src/coda_client.cpp
    src/coda_extension.cpp
    src/coda_scan.cpp
    src/coda_storage.cpp
    src/storage/coda_catalog.cpp
    src/storage/coda_dml.cpp
    src/storage/coda_schema.cpp
    src/storage/coda_table.cpp
    src/storage/coda_transaction_manager.cpp)

add_library(${EXTENSION_NAME} STATIC ${EXTENSION_SOURCES})
build_loadable_extension(${TARGET_NAME} "-warnings" ${EXTENSION_SOURCES})

target_link_libraries(${EXTENSION_NAME} CURL::libcurl)
target_link_libraries(${TARGET_NAME}_loadable_extension CURL::libcurl)

install(
  TARGETS ${EXTENSION_NAME}
  EXPORT "${DUCKDB_EXPORT_SET}"
  LIBRARY DESTINATION "${INSTALL_LIB_DIR}"
  ARCHIVE DESTINATION "${INSTALL_LIB_DIR}")
