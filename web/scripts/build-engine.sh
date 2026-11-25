#!/bin/sh
# Builds the Note engine (bindings/wasm, kition-note-kit) to WebAssembly into public/engine, where the
# editor loads it from its own package. Needs wasm-pack.
set -eu
web="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$web/.." && pwd)"
if toolchain="$(rustc --print sysroot 2>/dev/null)" && [ -d "$toolchain/lib" ]; then
  export DYLD_FALLBACK_LIBRARY_PATH="$toolchain/lib${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}"
fi
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target}"
# A dependency that compiles C for the target needs a WebAssembly C compiler; Apple's clang cannot target
# WebAssembly, so use Homebrew's LLVM when it is there and no compiler is set.
if [ -z "${CC_wasm32_unknown_unknown:-}" ] && command -v brew >/dev/null 2>&1; then
  for formula in llvm@21 llvm; do
    llvm="$(brew --prefix "$formula" 2>/dev/null || true)"
    if [ -n "$llvm" ] && [ -x "$llvm/bin/clang" ]; then
      export CC_wasm32_unknown_unknown="$llvm/bin/clang" AR_wasm32_unknown_unknown="$llvm/bin/llvm-ar"
      break
    fi
  done
fi
cd "$repo"
wasm-pack build bindings/wasm --target web --release --no-typescript --out-dir "$web/public/engine"
rm -f "$web/public/engine/.gitignore" "$web/public/engine/package.json"

# The faces the engine draws the note with (handed to it through addFont, Kition platform E9), from
# the kition-fonts checkout next to the repository, or KITION_FONTS_DIR. A web build embeds none.
fonts="${KITION_FONTS_DIR:-$repo/../kition-fonts}/fonts"
if [ ! -d "$fonts" ]; then
  echo "error: no fonts folder at $fonts: set KITION_FONTS_DIR to a folder whose fonts/ holds the faces (docs/development.md, \"Fonts\")" >&2
  exit 1
fi
rm -rf "$web/public/fonts"
mkdir -p "$web/public/fonts"
for entry in inter/Inter-Variable.ttf:OFL.txt jetbrains-mono/JetBrainsMono-Variable.ttf:OFL.txt droid-sans-fallback/DroidSansFallbackFull.ttf:NOTICE noto-color-emoji/Noto-COLRv1.ttf:OFL.txt; do
  file="${entry%%:*}"
  licence="${entry##*:}"
  family="${file%%/*}"
  if [ -f "$fonts/$file" ]; then
    cp "$fonts/$file" "$web/public/fonts/"
    cp "$fonts/$family/$licence" "$web/public/fonts/$family-$licence"
  else
    echo "error: no $file in $fonts: set KITION_FONTS_DIR (docs/development.md, \"Fonts\")" >&2
    exit 1
  fi
done

# The math face (STIX Two Math, SIL Open Font License) is embedded in the engine from
# vendor/latex-rust; its licence travels with the package beside the other faces'.
cp "$repo/vendor/latex-rust/fonts/stix-two-math/OFL.txt" "$web/public/fonts/stix-two-math-OFL.txt"
