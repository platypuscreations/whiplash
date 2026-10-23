# Third-party notices

Whiplash is licensed under the GNU General Public License, version 3 or (at
your option) any later version; see `LICENSE`. It includes or is based on
the following works, under their own licenses.

## projectM

MilkDrop visualizations are rendered by projectM, which is compiled into
Whiplash.

- **libprojectM** 4.1.3: GNU Lesser General Public License 2.1
  (`licenses/LGPL-2.1.txt`). Copyright the projectM team and contributors.
  Source: https://github.com/projectM-visualizer/projectm
- **projectm** and **projectm-sys** Rust crates: GNU Lesser General Public
  License 3.0 or later (`licenses/LGPL-3.0.txt`, which builds on the GPL in
  `LICENSE`). Source: https://github.com/projectM-visualizer/projectm-rs.
  Whiplash uses modified copies of both crates; their source and a
  description of the changes are in `vendor/projectm` and
  `vendor/projectm-sys` in Whiplash's source.

libprojectM bundles these components:

- **projectm-eval**: MIT License, copyright 2023 the projectM team.
- **HLSLParser**: MIT License, copyright 2013–2014 Unknown Worlds
  Entertainment, Inc.
- **GLM** (OpenGL Mathematics): MIT License, copyright G-Truc Creation.
- **SOIL2** and the **stb** image libraries: public domain.

On Windows, projectM also uses **GLEW** (the OpenGL Extension Wrangler
Library), which is compiled into Whiplash there: Modified BSD License,
copyright Milan Ikits, Marcelo E. Magallon and others, with parts under
the MIT-style Mesa 3-D and Khronos licenses. Source:
https://github.com/nigels-com/glew

The full source of Whiplash, including everything needed to rebuild it with a
modified projectM, is available at https://github.com/platypuscreations/whiplash.

## Webamp

The layout of classic skins (sprite positions, window layouts and
visualizer behavior) follows Webamp, https://github.com/captbaritone/webamp:

> The MIT License (MIT)
>
> Copyright (c) [2015] [Jordan Eldredge]
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

The `.eqf` equalizer preset format follows the description in Webamp's
`winamp-eqf` package (ISC License, copyright Jordan Eldredge).

## Rust crates

Whiplash is built from the Rust crates below (on every platform it supports;
any one build uses a subset). Each crate's full license text is included in
its source, available from https://crates.io.

| Crate | Version | License |
|---|---|---|
| ab_glyph | 0.2.32 | Apache-2.0 |
| ab_glyph_rasterizer | 0.1.10 | Apache-2.0 |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| ahash | 0.8.12 | MIT OR Apache-2.0 |
| alsa | 0.11.0 | Apache-2.0/MIT |
| alsa-sys | 0.4.0 | MIT |
| android-activity | 0.6.1 | MIT OR Apache-2.0 |
| android-properties | 0.2.2 | MIT |
| anyhow | 1.0.104 | MIT OR Apache-2.0 |
| arboard | 3.7.0 | MIT OR Apache-2.0 |
| arrayref | 0.3.9 | BSD-2-Clause |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 |
| as-raw-xcb-connection | 1.0.1 | MIT OR Apache-2.0 |
| async-broadcast | 0.7.2 | MIT OR Apache-2.0 |
| async-channel | 2.5.0 | Apache-2.0 OR MIT |
| async-executor | 1.14.0 | Apache-2.0 OR MIT |
| async-io | 2.6.0 | Apache-2.0 OR MIT |
| async-lock | 3.4.2 | Apache-2.0 OR MIT |
| async-process | 2.5.0 | Apache-2.0 OR MIT |
| async-recursion | 1.2.0 | MIT OR Apache-2.0 |
| async-signal | 0.2.14 | Apache-2.0 OR MIT |
| async-task | 4.7.1 | Apache-2.0 OR MIT |
| async-trait | 0.1.92 | MIT OR Apache-2.0 |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| base64 | 0.22.1 | MIT OR Apache-2.0 |
| bitflags | 1.3.2 | MIT/Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| bitstream-io | 4.10.0 | MIT/Apache-2.0 |
| block2 | 0.5.1 | MIT |
| block2 | 0.6.2 | MIT |
| blocking | 1.7.0 | Apache-2.0 OR MIT |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT |
| byteorder-lite | 0.1.0 | Unlicense OR MIT |
| bytes | 1.12.1 | MIT |
| bzip2 | 0.6.1 | MIT OR Apache-2.0 |
| calloop | 0.13.0 | MIT |
| calloop-wayland-source | 0.3.0 | MIT |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| cgl | 0.3.2 | MIT / Apache-2.0 |
| clipboard-win | 5.4.1 | BSL-1.0 |
| combine | 4.6.9 | MIT |
| concurrent-queue | 2.5.0 | Apache-2.0 OR MIT |
| core-foundation | 0.9.4 | MIT OR Apache-2.0 |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 |
| core-graphics | 0.23.2 | MIT OR Apache-2.0 |
| core-graphics-types | 0.1.3 | MIT OR Apache-2.0 |
| coreaudio-rs | 0.14.2 | MIT/Apache-2.0 |
| cpal | 0.18.2 | Apache-2.0 |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 |
| ctor | 0.10.1 | Apache-2.0 OR MIT |
| ctrlc | 3.5.2 | MIT/Apache-2.0 |
| cursor-icon | 1.2.0 | MIT OR Apache-2.0 OR Zlib |
| dasp_sample | 0.11.0 | MIT OR Apache-2.0 |
| deflate64 | 0.1.12 | MIT |
| directories | 6.0.0 | MIT OR Apache-2.0 |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 |
| dispatch | 0.2.0 | MIT |
| dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT |
| displaydoc | 0.2.5 | MIT OR Apache-2.0 |
| dlib | 0.5.3 | MIT |
| downcast-rs | 1.2.1 | MIT/Apache-2.0 |
| dpi | 0.1.2 | Apache-2.0 AND MIT |
| drm | 0.14.1 | MIT |
| drm-ffi | 0.9.1 | MIT |
| drm-fourcc | 2.2.0 | MIT |
| drm-sys | 0.8.1 | MIT |
| endi | 1.1.1 | MIT |
| enumflags2 | 0.7.12 | MIT OR Apache-2.0 |
| enumflags2_derive | 0.7.12 | MIT OR Apache-2.0 |
| equivalent | 1.0.3 | Apache-2.0 OR MIT |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| error-code | 3.4.0 | BSL-1.0 |
| event-listener | 5.4.2 | Apache-2.0 OR MIT |
| event-listener-strategy | 0.5.4 | Apache-2.0 OR MIT |
| extended | 0.1.0 | MIT |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| font-types | 0.12.7 | MIT OR Apache-2.0 |
| fontconfig-parser | 0.5.8 | MIT |
| fontdb | 0.24.0 | MIT |
| foreign-types | 0.5.0 | MIT/Apache-2.0 |
| foreign-types-macros | 0.2.4 | MIT/Apache-2.0 |
| foreign-types-shared | 0.3.1 | MIT/Apache-2.0 |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 |
| futures-io | 0.3.34 | MIT OR Apache-2.0 |
| futures-lite | 2.6.1 | Apache-2.0 OR MIT |
| futures-task | 0.3.34 | MIT OR Apache-2.0 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 |
| gethostname | 1.1.0 | Apache-2.0 |
| getrandom | 0.2.17 | MIT OR Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| getrandom | 0.4.3 | MIT OR Apache-2.0 |
| glutin | 0.32.3 | Apache-2.0 |
| glutin_egl_sys | 0.7.1 | Apache-2.0 |
| glutin_glx_sys | 0.6.1 | Apache-2.0 |
| glutin_wgl_sys | 0.6.1 | Apache-2.0 |
| glutin-winit | 0.5.0 | MIT |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| hermit-abi | 0.5.3 | MIT OR Apache-2.0 |
| hex | 0.4.3 | MIT OR Apache-2.0 |
| icu_collections | 2.1.1 | Unicode-3.0 |
| icu_locale_core | 2.1.1 | Unicode-3.0 |
| icu_normalizer | 2.1.1 | Unicode-3.0 |
| icu_normalizer_data | 2.1.1 | Unicode-3.0 |
| icu_properties | 2.1.1 | Unicode-3.0 |
| icu_properties_data | 2.1.1 | Unicode-3.0 |
| icu_provider | 2.1.1 | Unicode-3.0 |
| idna | 1.1.0 | MIT OR Apache-2.0 |
| idna_adapter | 1.2.1 | Apache-2.0 OR MIT |
| image | 0.25.10 | MIT OR Apache-2.0 |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| jni | 0.22.4 | MIT OR Apache-2.0 |
| jni-macros | 0.22.4 | MIT OR Apache-2.0 |
| jni-sys | 0.3.1 | MIT OR Apache-2.0 |
| jni-sys | 0.4.1 | MIT OR Apache-2.0 |
| jni-sys-macros | 0.4.1 | MIT OR Apache-2.0 |
| js-sys | 0.3.106 | MIT OR Apache-2.0 |
| lazy_static | 1.5.1 | MIT OR Apache-2.0 |
| libbz2-rs-sys | 0.2.5 | bzip2-1.0.6 |
| libc | 0.2.190 | MIT OR Apache-2.0 |
| libloading | 0.8.9 | ISC |
| libredox | 0.1.25 | MIT |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| linux-raw-sys | 0.4.15 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| linux-raw-sys | 0.9.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| litemap | 0.8.1 | Unicode-3.0 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.34 | MIT OR Apache-2.0 |
| mach2 | 0.6.0 | BSD-2-Clause OR MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| memmap2 | 0.9.11 | MIT OR Apache-2.0 |
| memoffset | 0.9.1 | MIT |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 |
| mpris-server | 0.10.0 | MPL-2.0 |
| ndk | 0.9.0 | MIT OR Apache-2.0 |
| ndk-context | 0.1.1 | MIT OR Apache-2.0 |
| ndk-sys | 0.6.0+11769913 | MIT OR Apache-2.0 |
| nix | 0.31.3 | MIT |
| no_std_io2 | 0.9.4 | Apache-2.0 OR MIT |
| num_enum | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 |
| num_enum_derive | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 |
| num-complex | 0.4.6 | MIT OR Apache-2.0 |
| num-derive | 0.4.2 | MIT OR Apache-2.0 |
| num-integer | 0.1.47 | MIT OR Apache-2.0 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| objc-sys | 0.3.5 | MIT |
| objc2 | 0.5.3 | MIT |
| objc2 | 0.6.5 | MIT |
| objc2-app-kit | 0.2.2 | MIT |
| objc2-app-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-audio-toolbox | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-avf-audio | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-core-audio | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-core-audio-types | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-core-graphics | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-encode | 4.1.0 | MIT |
| objc2-foundation | 0.2.2 | MIT |
| objc2-foundation | 0.3.2 | MIT |
| objc2-media-player | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-quartz-core | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-ui-kit | 0.2.2 | MIT |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| option-ext | 0.2.0 | MPL-2.0 |
| orbclient | 0.3.55 | MIT |
| ordered-stream | 0.2.0 | MIT OR Apache-2.0 |
| owned_ttf_parser | 0.25.1 | Apache-2.0 |
| parking | 2.2.1 | Apache-2.0 OR MIT |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| pin-project | 1.1.13 | Apache-2.0 OR MIT |
| pin-project-internal | 1.1.13 | Apache-2.0 OR MIT |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| piper | 0.2.5 | MIT OR Apache-2.0 |
| plain | 0.2.3 | MIT/Apache-2.0 |
| polling | 3.11.0 | Apache-2.0 OR MIT |
| pollster | 0.4.0 | Apache-2.0/MIT |
| potential_utf | 0.1.4 | Unicode-3.0 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| primal-check | 0.3.4 | MIT OR Apache-2.0 |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| projectm | 3.1.2 | LGPL-3.0-or-later |
| projectm-sys | 1.2.2 | LGPL-3.0-or-later |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 |
| quick-xml | 0.41.0 | MIT |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| rand | 0.8.8 | MIT OR Apache-2.0 |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 |
| rand_core | 0.6.4 | MIT OR Apache-2.0 |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| read-fonts | 0.41.0 | MIT OR Apache-2.0 |
| redox_syscall | 0.4.1 | MIT |
| redox_syscall | 0.5.18 | MIT |
| redox_syscall | 0.9.4 | MIT |
| redox_users | 0.5.3 | MIT |
| regex-lite | 0.1.9 | MIT OR Apache-2.0 |
| rfd | 0.17.2 | MIT |
| ring | 0.17.14 | Apache-2.0 AND ISC |
| ringbuf | 0.5.3 | MIT OR Apache-2.0 |
| roxmltree | 0.20.0 | MIT OR Apache-2.0 |
| rustfft | 6.4.1 | MIT OR Apache-2.0 |
| rustix | 0.38.44 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustix | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustls | 0.23.35 | Apache-2.0 OR ISC OR MIT |
| rustls-pki-types | 1.13.0 | MIT OR Apache-2.0 |
| rustls-webpki | 0.103.8 | ISC |
| rustversion | 1.0.23 | MIT OR Apache-2.0 |
| scoped-tls | 1.0.1 | MIT/Apache-2.0 |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| sctk-adwaita | 0.10.1 | MIT |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 |
| signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 |
| simd_cesu8 | 1.2.0 | Apache-2.0 OR MIT |
| simd-adler32 | 0.3.10 | MIT |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 |
| skrifa | 0.44.0 | MIT OR Apache-2.0 |
| slab | 0.4.12 | MIT |
| slotmap | 1.1.1 | Zlib |
| smallvec | 1.16.3 | MIT OR Apache-2.0 |
| smithay-client-toolkit | 0.19.2 | MIT |
| smol_str | 0.2.2 | MIT OR Apache-2.0 |
| softbuffer | 0.4.8 | MIT OR Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| strength_reduce | 0.2.4 | MIT OR Apache-2.0 |
| strict-num | 0.1.1 | MIT |
| subtle | 2.6.1 | BSD-3-Clause |
| symphonia | 0.6.1 | MPL-2.0 |
| symphonia-bundle-flac | 0.6.1 | MPL-2.0 |
| symphonia-bundle-mp3 | 0.6.1 | MPL-2.0 |
| symphonia-codec-aac | 0.6.1 | MPL-2.0 |
| symphonia-codec-adpcm | 0.6.1 | MPL-2.0 |
| symphonia-codec-alac | 0.6.1 | MPL-2.0 |
| symphonia-codec-pcm | 0.6.1 | MPL-2.0 |
| symphonia-codec-vorbis | 0.6.1 | MPL-2.0 |
| symphonia-common | 0.6.1 | MPL-2.0 |
| symphonia-core | 0.6.1 | MPL-2.0 |
| symphonia-format-caf | 0.6.1 | MPL-2.0 |
| symphonia-format-isomp4 | 0.6.1 | MPL-2.0 |
| symphonia-format-mkv | 0.6.1 | MPL-2.0 |
| symphonia-format-ogg | 0.6.1 | MPL-2.0 |
| symphonia-format-riff | 0.6.1 | MPL-2.0 |
| symphonia-metadata | 0.6.1 | MPL-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| synstructure | 0.13.2 | MIT |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| thiserror | 1.0.69 | MIT OR Apache-2.0 |
| thiserror | 2.0.21 | MIT OR Apache-2.0 |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 |
| tiny-skia | 0.11.4 | BSD-3-Clause |
| tiny-skia-path | 0.11.4 | BSD-3-Clause |
| tiny-xlib | 0.2.5 | MIT OR Apache-2.0 OR Zlib |
| tinystr | 0.8.2 | Unicode-3.0 |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT |
| tokio | 1.53.3 | MIT |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_edit | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| tracing | 0.1.44 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| trait-variant | 0.1.3 | MIT OR Apache-2.0 |
| transpose | 0.2.3 | MIT OR Apache-2.0 |
| ttf-parser | 0.25.1 | MIT OR Apache-2.0 |
| typed-path | 0.12.3 | MIT OR Apache-2.0 |
| uds_windows | 1.2.1 | MIT |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |
| untrusted | 0.9.0 | ISC |
| ureq | 2.12.1 | MIT OR Apache-2.0 |
| url | 2.5.7 | MIT OR Apache-2.0 |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT |
| uuid | 1.28.0 | Apache-2.0 OR MIT |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-futures | 0.4.79 | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.129 | MIT OR Apache-2.0 |
| wayland-backend | 0.3.17 | MIT |
| wayland-client | 0.31.15 | MIT |
| wayland-csd-frame | 0.3.0 | MIT |
| wayland-cursor | 0.31.14 | MIT |
| wayland-protocols | 0.32.13 | MIT |
| wayland-protocols-plasma | 0.3.12 | MIT |
| wayland-protocols-wlr | 0.3.12 | MIT |
| wayland-scanner | 0.31.11 | MIT |
| wayland-sys | 0.31.11 | MIT |
| web-sys | 0.3.106 | MIT OR Apache-2.0 |
| web-time | 1.1.0 | MIT OR Apache-2.0 |
| webpki-roots | 0.26.11 | CDLA-Permissive-2.0 |
| webpki-roots | 1.0.4 | CDLA-Permissive-2.0 |
| windows | 0.62.2 | MIT OR Apache-2.0 |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 |
| windows-core | 0.62.2 | MIT OR Apache-2.0 |
| windows-future | 0.3.2 | MIT OR Apache-2.0 |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 |
| windows-result | 0.4.1 | MIT OR Apache-2.0 |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 |
| winit | 0.30.13 | Apache-2.0 |
| winnow | 1.0.4 | MIT |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| writeable | 0.6.2 | Unicode-3.0 |
| x11-dl | 2.21.0 | MIT |
| x11rb | 0.13.2 | MIT OR Apache-2.0 |
| x11rb-protocol | 0.13.2 | MIT OR Apache-2.0 |
| xcursor | 0.3.11 | MIT |
| xkbcommon-dl | 0.4.2 | MIT |
| xkeysym | 0.2.1 | MIT OR Apache-2.0 OR Zlib |
| yoke | 0.8.1 | Unicode-3.0 |
| yoke-derive | 0.8.1 | Unicode-3.0 |
| zbus | 5.19.0 | MIT |
| zbus_macros | 5.19.0 | MIT |
| zbus_names | 4.3.5 | MIT |
| zcheapstr | 1.1.0 | MIT |
| zeno | 0.3.3 | Apache-2.0 OR MIT |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerofrom | 0.1.6 | Unicode-3.0 |
| zerofrom-derive | 0.1.6 | Unicode-3.0 |
| zeroize | 1.8.2 | Apache-2.0 OR MIT |
| zerotrie | 0.2.3 | Unicode-3.0 |
| zerovec | 0.11.5 | Unicode-3.0 |
| zerovec-derive | 0.11.2 | Unicode-3.0 |
| zip | 9.0.3 | MIT |
| zlib-rs | 0.6.8 | Zlib |
| zmij | 1.0.23 | MIT |
| zopfli | 0.8.4 | Apache-2.0 |
| zvariant | 5.15.1 | MIT |
| zvariant_derive | 5.15.1 | MIT |
| zvariant_utils | 4.2.0 | MIT |
