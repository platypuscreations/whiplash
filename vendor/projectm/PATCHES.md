# Local changes to the `projectm` crate

This is version 3.1.2 of the [`projectm`](https://crates.io/crates/projectm)
crate (LGPL-3.0-or-later, https://github.com/projectM-visualizer/projectm-rs),
used by skinamp through `[patch.crates-io]` in skinamp's `Cargo.toml`. Only
the source, tests, license and readme are included.

Changes in `src/core.rs`, marked "skinamp patch":

- C strings use the platform's `c_char` instead of a hard-coded `i8`. On ARM
  Linux `char` is unsigned, and the original didn't compile there.
- `load_preset_file` and `load_preset_data` pass NUL-terminated copies of
  their arguments. The original passed a Rust `&str` pointer, which has no
  terminator, so projectM could read past the end of the string.
- The preset-switch-failed callback converts names and messages lossily
  rather than unwrapping: a panic inside a C callback aborts the program.

And in `Cargo.toml`: removed a stray tab from the `license` field.

When upstream fixes these, remove this folder and the `[patch.crates-io]`
entry.
