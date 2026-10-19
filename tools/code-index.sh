#!/usr/bin/env bash
# Analyse the game's code ONCE, keep the analysis, and export it as a queryable index.
#
#   bash tools/code-index.sh analyze main    # full Ghidra auto-analysis of work/rea/main.elf (~50 min)
#   bash tools/code-index.sh analyze nro     # the same for every work/rea/lua2cpp_*.elf (~15 min)
#   bash tools/code-index.sh export main     # PointerFunctions.java + CodeIndex.java -> work/code-index/export/main/
#   bash tools/code-index.sh export nro      #   ... one directory per NRO
#   bash tools/code-index.sh build           # load the exports into work/code-index/ssbu-code.sqlite
#   bash tools/code-index.sh all             # all of the above, in order
#
# UPGRADING AN EXISTING EXPORT (what was done 2026-10-18, without redoing main's hour of decompile):
#   bash tools/code-index.sh pointers nro|main|lua2cpp_<m>.elf
#       functions at every code address only ever taken by pointer (tools/ghidra/PointerFunctions.java),
#       SAVED into the kept project, then an incremental export into work/code-index/export.next/:
#       inventories and asm.txt.gz in full, decompile only for the new functions (decomp-added.c)
#   python tools/code-index.py merge          # export.next/<m>/decomp.c = old decomp.c + decomp-added.c
#   python tools/code-index.py swap           # export -> export.old, export.next -> export; then build
#
# Then query with `python tools/code-index.py` (callers, refs-to, strings, fn, ...).
#
# WHY KEEP IT. REA (handbook/rea.md) re-runs this analysis in a throwaway project every session,
# which for `main` is an hour before the first answer. The game's code does not change, so the
# analysis is done once into a kept Ghidra project, and everything it found is exported to plain
# files: functions, symbols, every code AND data reference, strings, and a fresh decompile.
#
# WHERE. Ghidra runs in WSL (Ubuntu), the install `tools/rea-wsl.sh setup` puts there, and the kept
# projects live on WSL's own filesystem at /srv/ssbu/ghidra, which is far faster than /mnt/c for a
# database this size. Exports land in work/code-index/export/ on Windows. Rebuild the ELFs with
# tools/rea-elf.py first; a rebuilt ELF means `analyze` again, since its bytes are the input.
set -uo pipefail
export MSYS_NO_PATHCONV=1
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WROOT=/mnt/c/Users/poaga/SSBUMods
WSL=(wsl.exe -d Ubuntu --exec)

ghidra() {   # ghidra <project> <args...>   headless, with the REA server's environment
    "${WSL[@]}" bash -s -- "$@" <<'EOF'
. /usr/local/bin/rea-mcp-env
export GHIDRA_HEADLESS_MAXMEM="${KEEP_MAXMEM:-8G}"
P="$1"; shift
mkdir -p /srv/ssbu/ghidra/logs
exec /opt/ghidra_12.1.4_PUBLIC/support/analyzeHeadless /srv/ssbu/ghidra "$P" "$@"
EOF
}

analyze() {
    local proj="$1" elfs
    case "$proj" in
        main) elfs=("$WROOT/work/rea/main.elf") ;;
        nro)  elfs=(); for f in "$ROOT"/work/rea/lua2cpp_*.elf; do elfs+=("$WROOT/work/rea/$(basename "$f")"); done ;;
        *) echo "analyze main|nro" >&2; return 2 ;;
    esac
    "${WSL[@]}" sh -c 'mkdir -p /srv/ssbu/ghidra/elf && cp "$@" /srv/ssbu/ghidra/elf/' sh "${elfs[@]}" || return 1
    local args=()
    for e in "${elfs[@]}"; do args+=(-import "/srv/ssbu/ghidra/elf/$(basename "$e")"); done
    local s=$SECONDS
    ghidra "$proj" "${args[@]}" -overwrite > "$ROOT/work/code-index/analyze-$proj.log" 2>&1
    local rc=$?
    echo "analyze $proj: exit $rc, $((SECONDS - s)) s (log work/code-index/analyze-$proj.log)"
    return $rc
}

export_() {
    local proj="$1" s=$SECONDS
    mkdir -p "$ROOT/work/code-index/export"
    # WSLENV carries the settings across into Linux; CI_OUT is already a Linux path.
    # PointerFunctions first (so writable, not -readOnly): a fresh export then has the pointer-only
    # functions from the start; on a project that already has them it finds nothing and changes nothing.
    CI_OUT="$WROOT/work/code-index/export" \
        WSLENV="CI_OUT/u:CI_DECOMP/u:CI_ASM/u:PF_ROUNDS/u:PF_STRICT/u:KEEP_MAXMEM/u${WSLENV:+:$WSLENV}" \
        ghidra "$proj" -process -noanalysis -scriptPath "$WROOT/tools/ghidra" \
        -postScript PointerFunctions.java -postScript CodeIndex.java \
        > "$ROOT/work/code-index/export-$proj.log" 2>&1
    local rc=$?
    echo "export $proj: exit $rc, $((SECONDS - s)) s (log work/code-index/export-$proj.log)"
    return $rc
}

# The pointer-function pass writes to the kept project (no -readOnly), so it runs its export in
# the same session, against the program it just changed. CI_KNOWN is the export it adds to.
pointers() {
    local what="$1" proj tag s=$SECONDS
    local prog=()
    case "$what" in
        main) proj=main; tag=main ;;
        nro)  proj=nro;  tag=nro ;;
        lua2cpp_*) proj=nro; tag="${what%.elf}"; prog=("${tag}.elf") ;;
        *) echo "pointers main|nro|lua2cpp_<m>.elf" >&2; return 2 ;;
    esac
    mkdir -p "$ROOT/work/code-index/export.next"
    CI_OUT="$WROOT/work/code-index/export.next" CI_KNOWN="$WROOT/work/code-index/export" \
        WSLENV="CI_OUT/u:CI_KNOWN/u:CI_ASM/u:PF_ROUNDS/u:PF_STRICT/u:KEEP_MAXMEM/u${WSLENV:+:$WSLENV}" \
        ghidra "$proj" -process "${prog[@]}" -noanalysis -scriptPath "$WROOT/tools/ghidra" \
        -postScript PointerFunctions.java -postScript CodeIndex.java \
        > "$ROOT/work/code-index/pointers-$tag.log" 2>&1
    local rc=$?
    echo "pointers $tag: exit $rc, $((SECONDS - s)) s (log work/code-index/pointers-$tag.log)"
    return $rc
}

mkdir -p "$ROOT/work/code-index"
case "${1:-}" in
    analyze) analyze "${2:?analyze main|nro}" ;;
    export)  export_ "${2:?export main|nro}" ;;
    build)   python "$ROOT/tools/code-index.py" build ;;
    pointers) pointers "${2:?pointers main|nro|lua2cpp_<m>.elf}" ;;
    all)     analyze nro && export_ nro && analyze main && export_ main && python "$ROOT/tools/code-index.py" build ;;
    *)       sed -n '2,18p' "$0"; exit 2 ;;
esac
