# Local changes to the `projectm-sys` crate

This is version 1.2.2 of the [`projectm-sys`](https://crates.io/crates/projectm-sys)
crate (LGPL-3.0-or-later, https://github.com/projectM-visualizer/projectm-rs),
with the libprojectM source it bundles, used by Whiplash through
`[patch.crates-io]` in Whiplash's `Cargo.toml`. As published, it doesn't link
on Windows.

Changes in `build.rs`, marked "whiplash patch":

- projectM is always built in CMake's Release configuration, without the "d"
  suffix on library names. The original built a Debug configuration in debug
  builds and linked `projectM-4d`; Rust always uses the release C runtime, so
  on Windows the two couldn't be linked together.
- On Windows, the library names include the "lib" prefix CMake gives static
  libraries there (`libprojectM-4`, `libprojectM-4-playlist`).
- On Windows, the bundled projectm-eval and GLM are built in, as on other
  systems, instead of being fetched as separate libraries through vcpkg, and
  GLEW, which vcpkg builds, is linked.
- On Windows, static builds define `PROJECTM_STATIC_DEFINE` for all C++
  code. The playlist's object library didn't get it, so it imported
  projectM's functions as if from a DLL (linker warning LNK4217).
- On Windows, the C runtime and the vcpkg triplet follow Rust's: the DLL
  runtime normally, the static one (`x64-windows-static`) with `+crt-static`.

When upstream fixes these, remove this folder and its `[patch.crates-io]`
entry.
