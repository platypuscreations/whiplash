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
