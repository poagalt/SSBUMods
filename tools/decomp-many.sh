#!/usr/bin/env bash
# Decompile a whole address list in ONE Ghidra invocation, on every core.
#
#   bash tools/decomp-many.sh common work/decomp/common.todo.all out.c
#
# `decomp-all.sh` batches 150 addresses per invocation and pays a fresh import plus a 14,220-symbol
# application every time -- about 7s of the 9s each batch takes. This pays that once and hands the
# whole list to `ParallelDecompiler`, which runs a pool of decompiler processes across the 24 cores
# this machine has. See tools/ghidra/DecompMany.java.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
MOD="${1:?usage: decomp-many.sh <module> <address-list> <out.c>}"
LIST="${2:?}"; OUT="${3:?}"

export JAVA_HOME="$ROOT/vendor/tools/jdk-21.0.12.1+1"
G="$ROOT/vendor/tools/ghidra_12.1.3_PUBLIC"
export PATH="$JAVA_HOME/bin:$PATH"

# analyzeHeadless defaults to a 2G heap (support/analyzeHeadless.bat: MAXMEM_DEFAULT=2G), which
# is not enough for a list this size. The 32,857-function run produced 155 MB of C and the JVM
# sat at 1.66 GB of that 2 GB; 100,508 functions produce ~470 MB, and the heap has to hold the
# results map plus Ghidra's own program database for 100k created functions. The decompiler pool
# itself runs as NATIVE CHILD PROCESSES, so its memory is outside this number.
export GHIDRA_HEADLESS_MAXMEM="${GHIDRA_HEADLESS_MAXMEM:-6G}"

if [ "$MOD" = "main" ]; then
    BIN="$ROOT/vendor/main/rat-main-text.bin"; SYMS="$ROOT/vendor/main/symbols-merged.txt"
    BASE=0x0; NAME=rat-main-text.bin
else
    BIN="$ROOT/vendor/nro/prebuilt/nro/release/lua2cpp_$MOD.nro"
    SYMS="$ROOT/vendor/main/$MOD-nro-symbols.txt"
    BASE=0x7100000000; NAME="lua2cpp_$MOD.nro"
fi

mkdir -p work/ghidra
export RAT_SYMS="$(cygpath -w "$SYMS")"
export RAT_LIST="$(cygpath -w "$LIST")"
export RAT_OUT="$(cygpath -w "$OUT")"

if [ -d "work/ghidra/rat.rep/$NAME" ] || grep -qs "$NAME" work/ghidra/.imported; then
    ACT=(-process "$NAME")
else
    ACT=(-import "$(cygpath -w "$BIN")" -processor AARCH64:LE:64:v8A -loader BinaryLoader
         -loader-baseAddr "$BASE" -overwrite)
    echo "$NAME" >> work/ghidra/.imported
fi

cmd //c "$(cygpath -w "$G/support/analyzeHeadless.bat")" "$(cygpath -w work/ghidra)" rat \
    "${ACT[@]}" -noanalysis \
    -scriptPath "$(cygpath -w tools/ghidra)" -postScript DecompMany.java 2>&1 \
  | grep -E "applied|prepared|decompiled|wrote|ERROR|Exception" \
  | sed 's/^INFO  DecompMany.java> /  /; s/ (GhidraScript)  *$//'

# NOT ANNOTATED, DELIBERATELY. `decomp.sh` pipes its output through an awk pass that resolves
# lua_const reads and hash40 immediates, which is what makes ONE function readable. This produces
# raw Ghidra C, because for bulk coverage the point is that the body EXISTS and is greppable --
# `FUN_7100021af0` and `FUN_710002cab0` were both read out of raw output. Use `decomp.sh` when you
# want a specific function annotated; use this when you want all of them present.
