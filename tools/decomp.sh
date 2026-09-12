#!/usr/bin/env bash
# Decompiles a function out of the game, to annotated C.
#
#   bash tools/decomp.sh main 0x3ee290                  CaptureCut, out of main's .text
#   bash tools/decomp.sh common 0x71000b1980            CaptureWaitCommon, out of lua2cpp_common
#   bash tools/decomp.sh gaogaen 0x7100022460 0x...     several at once
#
# WHY THIS EXISTS, AND WHY IT IS WORTH 800 MB OF TOOLING. The native code that decides "this catch
# connected, so put the victim there and me here" is compiled C++ in `main`, keyed on the fighter
# kind. It is the reason every grab in this moveset is a reimplementation, and six rounds of
# playtesting each fixed a real thing and each left the grab broken a new way -- which is what
# inference from disassembly looks like when it is nearly right.
#
# `vendor/ssbu-status` has Ghidra-decompiled C for five FIGHTER NROs and none for
# `lua2cpp_common.nro`, which is where the VICTIM side of every grab lives, and the public matching
# decomp is 35% by function count but 7% BY BYTES -- skewed to small functions, 46 bytes matched
# against 986 unmatched on average -- so the large capture functions are not in it and will not be
# soon.
#
# So: Ghidra 12.1.3 and a portable Temurin 21, both unzipped into vendor/tools with no installer
# and no admin, driven headless. Import once, decompile anything, for as long as the game is on
# 13.0.5.
#
# WHAT MAKES THE OUTPUT READABLE rather than merely correct:
#   * 8,905 merged symbols for `main` and 14,220 for the NROs, so calls have names.
#   * FUNCTIONS created at the 5,428 imported PLT stubs, not just labels -- the decompiler prints
#     `func_0x00710052f1a0` for a call target that has no function at it no matter how many labels
#     are on it, and unnamed calls are most of what makes a status script unreadable.
#   * lua_const reads resolved through vendor/ssbu-status/lua_dict.txt, so `lRam...0b578 + 0xb80`
#     becomes a constant with a name.
#   * hash40 immediates resolved through tools/hash40.sh's name list.
#
# Analysis is OFF (-noanalysis). A full auto-analysis of 60 MB of AArch64 takes hours and buys
# nothing here: the decompiler works one function at a time from a disassembly, and this
# disassembles exactly the entry points asked for.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

export JAVA_HOME="$ROOT/vendor/tools/jdk-21.0.12.1+1"
G="$ROOT/vendor/tools/ghidra_12.1.3_PUBLIC"
if [ ! -x "$JAVA_HOME/bin/java.exe" ] || [ ! -f "$G/support/analyzeHeadless.bat" ]; then
    echo "Ghidra or the JDK is missing from vendor/tools." >&2
    echo "Both are plain zips that need no installer:" >&2
    echo "  https://github.com/NationalSecurityAgency/ghidra/releases  (ghidra_*_PUBLIC_*.zip)" >&2
    echo "  https://api.adoptium.net/v3/binary/latest/21/ga/windows/x64/jdk/hotspot/normal/eclipse" >&2
    echo "Unzip with Windows' own tar -- Git Bash's GNU tar cannot read a zip." >&2
    exit 1
fi
export PATH="$JAVA_HOME/bin:$PATH"

which="${1:?usage: decomp.sh <main|common|gaogaen|donkey|gamewatch|packun|pikachu> <addr>...}"
shift
case "$which" in
  main)
    BIN="$ROOT/vendor/main/rat-main-text.bin"; SYMS="$ROOT/vendor/main/symbols-merged.txt"
    BASE=0x0; NAME=rat-main-text.bin ;;
  *)
    BIN="$ROOT/vendor/nro/prebuilt/nro/release/lua2cpp_$which.nro"
    SYMS="$ROOT/vendor/main/$which-nro-symbols.txt"
    BASE=0x7100000000; NAME="lua2cpp_$which.nro" ;;
esac
[ -f "$BIN" ] || { echo "no such binary: $BIN" >&2; exit 1; }

# The symbol table for an NRO is generated on demand, in the `addr size name kind` form the script
# reads. `kind` is what decides which addresses become functions.
if [ "$which" != "main" ] && [ ! -s "$SYMS" ]; then
    echo "  building $SYMS"
    tools/disasm/target/release/disasm.exe "$BIN" --sym "" 2>/dev/null \
      | awk '$2=="def" || $2=="plt" { a=$1; sub(/^0x/,"",a); print a" 0 "$3" "$2 }' > "$SYMS"
fi

mkdir -p work/ghidra
OUT="work/ghidra/$which-$(printf '%s' "$1" | tr -d '0x').c"
export RAT_SYMS="$(cygpath -w "$SYMS")"
export RAT_FUNCS="$(printf '%s,' "$@" | sed 's/,$//')"
export RAT_OUT="$(cygpath -w "$OUT")"

# Import on the first run for this binary, reuse the project afterwards -- the import is the slow
# part and the project keeps the symbols.
if [ -d "work/ghidra/rat.rep/$NAME" ] || grep -qs "$NAME" work/ghidra/.imported; then
    ACT=(-process "$NAME")
else
    ACT=(-import "$(cygpath -w "$BIN")" -processor AARCH64:LE:64:v8A -loader BinaryLoader
         -loader-baseAddr "$BASE" -overwrite)
    echo "$NAME" >> work/ghidra/.imported
fi

cmd //c "$(cygpath -w "$G/support/analyzeHeadless.bat")" "$(cygpath -w work/ghidra)" rat \
    "${ACT[@]}" -noanalysis \
    -scriptPath "$(cygpath -w tools/ghidra)" -postScript DecompAt.java 2>&1 \
  | grep -E "applied|decompiled|failed|ERROR" | sed 's/^INFO  DecompAt.java> /  /; s/ (GhidraScript)  *$//'

[ -s "$OUT" ] || { echo "  nothing was written" >&2; exit 1; }

# ---- annotate ---------------------------------------------------------------------------------
# The decompiler cannot know that one global is the lua_const array or that a 40-bit integer is a
# hashed name. Both are mechanical to resolve and both are the difference between reading the
# logic and decoding it.
ANN="${OUT%.c}.annotated.c"
awk -v dict="$ROOT/vendor/ssbu-status/lua_dict.txt" '
BEGIN {
    while ((getline l < dict) > 0) {
        n = index(l, ":")
        if (n > 0) { LC[tolower(substr(l, 1, n-1))] = substr(l, n+1) }
    }
}
{
    line = $0

    # THE L2CValue MARSHALLING IS NOISE. Every constant in a status script is wrapped into an
    # L2CValue before it is used, so the mangled constructors outnumber the real engine calls
    # about three to one and the logic is invisible between them.
    gsub(/_ZN3lib8L2CValueC1E[A-Za-z0-9_]*/, "L2CValue", line)
    gsub(/_ZNK?3lib8L2CValue[0-9]+/, "L2CValue::", line)
    gsub(/_ZN3app8lua_bind[0-9]+/, "", line)
    gsub(/_ZN7lua2cpp[0-9]+/, "", line)
    gsub(/_implEPNS_26BattleObjectModuleAccessorE[A-Za-z0-9_]*/, "", line)
    gsub(/EPNS_26BattleObjectModuleAccessorE[A-Za-z0-9_]*/, "", line)

    # lua_const reads. Ghidra puts the table pointer in a local and reads through it, so the
    # `lVarN + 0xNNN` form is as common as the direct one -- and it is what every work id, status
    # kind and flag in a status function looks like. Only 4-byte reads through a local count, and
    # only when the offset is actually IN the dictionary, which stops a struct field at the same
    # offset from being renamed after a constant it has nothing to do with.
    while (match(line, /\*\(undefined4 \*\)\((lVar|lRam|puVar)[0-9a-zA-Z_]* \+ 0x[0-9a-f]+\)/)) {
        m = substr(line, RSTART, RLENGTH)
        off = m
        sub(/.*\+ 0x/, "", off)
        sub(/\)$/, "", off)
        key = tolower(off)
        rep = (key in LC) ? "lua_const[" LC[key] "]" : "lua_const[+0x" off "]"
        line = substr(line, 1, RSTART-1) rep substr(line, RSTART+RLENGTH)
    }
    print line
}
' "$OUT" > "$ANN" 2>/dev/null || cp "$OUT" "$ANN"

echo "  wrote $ANN"
