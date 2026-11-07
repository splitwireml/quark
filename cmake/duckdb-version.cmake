# Pins the DuckDB version for the bundled CMake build. The cargo checkout of
# the duckdb-sources submodule has no git tags, so CMake would otherwise fall
# back to "v0.0.1" and the engine would not match the Python duckdb version.
# Keep in sync with the duckdb version in uv.lock.
#
# Known limits:
# - The libduckdb-sys build script does not watch this file: after a bump of
#   the pin, run `cargo clean -p libduckdb-sys` so DuckDB is configured again.
# - An externally set CMAKE_TOOLCHAIN_FILE overrides the pin (cargo's [env]
#   does not replace a variable already set); the engine_version test catches it.
# - The pin applies to cargo runs inside this repository (.cargo/config.toml).
set(OVERRIDE_GIT_DESCRIBE "v1.5.4" CACHE STRING "")
