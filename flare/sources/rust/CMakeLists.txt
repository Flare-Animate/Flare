# Optional Rust backend. Crate manifest is flare/sources/Cargo.toml; the modules
# themselves live beside the C++ they port (common/<subsystem>/rust/), so the
# source tree keeps matching OpenToonz/Tahoma as the rewrite proceeds.
include(FetchContent)
FetchContent_Declare(Corrosion GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git GIT_TAG v0.5.1)
FetchContent_MakeAvailable(Corrosion)
corrosion_import_crate(MANIFEST_PATH ${CMAKE_CURRENT_LIST_DIR}/../Cargo.toml CRATES flare_formats CRATE_TYPES staticlib)
target_include_directories(flare_formats INTERFACE ${CMAKE_CURRENT_LIST_DIR}/../include)
target_compile_definitions(flare_formats INTERFACE FLARE_WITH_RUST=1)
if(WIN32)
  target_link_libraries(flare_formats INTERFACE ws2_32 userenv ntdll bcrypt)
endif()