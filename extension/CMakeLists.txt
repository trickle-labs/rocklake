cmake_minimum_required(VERSION 3.14)
project(slateduck_extension)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Find the slateduck shared library
find_library(SLATEDUCK_LIB slateduck_ffi
    PATHS ${CMAKE_CURRENT_SOURCE_DIR}/../target/release
          ${CMAKE_CURRENT_SOURCE_DIR}/../target/debug)

# Include the generated C header
include_directories(${CMAKE_CURRENT_SOURCE_DIR}/include)

# Extension source
add_library(slateduck_extension SHARED
    src/slateduck_extension.cpp
)

target_link_libraries(slateduck_extension ${SLATEDUCK_LIB})

set_target_properties(slateduck_extension PROPERTIES
    OUTPUT_NAME "slateduck"
    PREFIX ""
    SUFFIX ".duckdb_extension"
)
