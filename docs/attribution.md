# Attribution

The Note kit is Kition's own code (Apache-2.0, [LICENSE](../LICENSE)).
The engine links these libraries; every one is shipped inside the kit's WebAssembly module and the
headless `kition-note` executable.

| Library | License | Through |
|---|---|---|
| Kition SDK (`kition-sdk`, `kition-kdm`, `kition-headless`, `kition-kit-server`, `kition-convergence`, `kition-kits`, `kition-env`) | Apache-2.0 | direct, pinned by commit in `Cargo.toml` |
| Kition Tessera icons (`kition-tessera-icons`) | Apache-2.0; its `lucide:` icons are Lucide path data (ISC, Feather-derived ones MIT), see [NOTICE](../NOTICE) | direct, pinned by commit in `Cargo.toml` |
| Loro (`loro`) | MIT | `kition-kdm` (the KDM's replication engine) |
| serde, serde_json | MIT OR Apache-2.0 | direct |
| latex-rust 2.1.1 | MIT OR Apache-2.0 | direct (math formulas), vendored in `vendor/latex-rust` with its license files and NOTICE; changed by the Kition authors to take its dvips color names from a table of our own (`src/color.rs`) in place of its data file derived from the LaTeX graphics bundle |
| STIX Two Math 2.13 | SIL Open Font License 1.1 (text below) | the math face, unmodified, as latex-rust ships it (`vendor/latex-rust/fonts/stix-two-math/`, with its `OFL.txt`); Copyright 2001-2021 The STIX Fonts Project Authors. STIX Fonts is a trademark of The Institute of Electrical and Electronics Engineers, Inc. |

The view draws 14 icons from path data copied from Lucide (https://lucide.dev, ISC license,
copyright Lucide Icons and Contributors), in `crates/view/view/src/paint.rs`: the 13 callout
icons `pencil`, `flame`, `triangle-alert`, `zap`, `clipboard-list`, `info`, `circle-check`,
`check`, `circle-help`, `x`, `bug`, `list` and `quote`, and `external-link`; the folding chevrons
are Lucide's `chevron-right` and `chevron-down` shapes. `triangle-alert`, `info`, `check`,
`circle-help`, `x`, `external-link` and the chevrons derive from Feather (MIT, copyright Cole
Bemis). Both license texts are in [NOTICE](../NOTICE).

The faces the engine draws a note with are not in this repository: the editor's build
(`web/scripts/build-engine.sh`) and the native tests copy them from a fonts directory
(`KITION_FONTS_DIR`) into the package, each with its license file beside it: Inter (OFL-1.1),
JetBrains Mono (OFL-1.1), Droid Sans Fallback (Apache-2.0), and Noto Color Emoji (OFL-1.1).

The code highlighter (`crates/view/view/src/highlight/`) is a Rust port of the language modes of
CodeMirror 5 (https://codemirror.net/5/), which are under the MIT license:

```text
MIT License

Copyright (C) 2017 by Marijn Haverbeke <marijn@haverbeke.berlin> and others

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

The full dependency list with versions is `Cargo.lock`; `cargo tree -e normal` shows which crate
brings in which. When a dependency is added, add its row here in the same change.

The editor in `web/` is built on `@kition/kit-editor` (kition-sdk, Apache-2.0); its npm dependencies
are listed in `web/package.json` under their own licenses.
The editor draws Mermaid diagrams with Mermaid (https://mermaid.js.org, MIT, copyright Knut
Sveidqvist), pinned to the version Obsidian ships (11.13.0) and bundled into the package; its edit
button draws Lucide's `code-2` icon (ISC). HTML blocks are sanitized with DOMPurify
(https://github.com/cure53/DOMPurify, MPL-2.0 OR Apache-2.0, used under Apache-2.0), bundled into
the package. Pasted HTML becomes Markdown through Turndown (https://github.com/mixmark-io/turndown,
MIT, copyright Dom Christie), the version Obsidian ships (7.2.0), bundled into the package.

STIX Two Math is under the SIL Open Font License 1.1:

```text
Copyright 2001-2021 The STIX Fonts Project Authors (https://github.com/stipub/stixfonts)

STIX Fonts™ is a trademark of The Institute of Electrical and
Electronics Engineers, Inc.

Portions copyright © 1998–2003 by MicroPress, Inc.
(www.micropress-inc.com), with Reserved Font Name TM Math.
To obtain additional mathematical fonts, please contact
    MicroPress, Inc.,
    68-30 Harrow Street,
    Forest Hills, NY 11375,
    USA
    Phone: (718) 575-1816.  

Portions copyright © 1990 by Elsevier, Inc.  

Portions copyright © 2010, 2012, 2014 Adobe Systems Incorporated.

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
http://scripts.sil.org/OFL

-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded, 
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```
