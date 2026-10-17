# Rust backend (required). Crate manifest is flare/sources/Cargo.toml (crate
# root flare_formats.rs); the modules live beside the C++ they port.
# Included from flare/sources/CMakeLists.txt before any subdirectory that links it.
include(FetchContent)
FetchContent_Declare(Corrosion GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git GIT_TAG v0.5.1)
FetchContent_MakeAvailable(Corrosion)
if(NOT COMMAND corrosion_import_crate)
  message(FATAL_ERROR "Corrosion (corrosion_import_crate) is required to build the Rust backend flare_formats")
endif()
corrosion_import_crate(MANIFEST_PATH ${CMAKE_CURRENT_LIST_DIR}/../Cargo.toml CRATES flare_formats CRATE_TYPES staticlib)
target_include_directories(flare_formats INTERFACE ${CMAKE_CURRENT_LIST_DIR}/../include)
target_compile_definitions(flare_formats INTERFACE FLARE_WITH_RUST=1)
if(WIN32)
  target_link_libraries(flare_formats INTERFACE ws2_32 userenv ntdll bcrypt)
endif()
