#!/usr/bin/env bash
# Exports the shipped animation of every ported move whose hitboxes ride a GRAFTED bone, as JSON,
# for `tools/reach rebase` to read the bone's per-frame transform out of.
#
# WHY ONLY THOSE MOVES. `ssbh_data_json` on a .nuanmb takes a second or two and there are 77 of
# them; two of them animate a grafted bone. So the list is derived from the generated scripts
# rather than declared: whichever ones still mention `sword1` or `stick` at this point in the build
# are the ones that need it, which also means the list cannot go out of step with the graft the way
# gen-moves.sh's tables, moves.tsv and topscale's MAP have all managed to (see check-lists.sh).
#
# Runs AFTER gen-moves.sh (the scripts must exist to be grepped) and BEFORE reach rebase.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
OUT="$ROOT/work/rebase"
MOT="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00"
mkdir -p "$OUT"
n=0

# `<fighter> <AcmdFile> <acmd fn> <animation stem> <module> <label>`, whitespace separated.
while read -r f acmd fn stem mod label; do
    case "${f:-}" in ''|\#*) continue ;; esac
    [ -z "${stem:-}" ] && continue
    # Does the generated script for this move still anchor a hitbox to a grafted bone?
    grep -rqs "fn pikachu_game_$fn(" "$ROOT/pikachu-moveset/plugin/src" || continue
    body=$(awk -v t="fn pikachu_game_$fn(" 'index($0,t){i=1} i{print} i&&/^}/{exit}' \
           $(grep -rl "fn pikachu_game_$fn(" "$ROOT/pikachu-moveset/plugin/src") 2>/dev/null)
    case "$body" in
        *'Hash40::new("sword1")'*|*'Hash40::new("stick")'*) ;;
        *) continue ;;
    esac
    src="$MOT/$stem.nuanmb"
    [ -f "$src" ] || { echo "  no shipped animation $stem.nuanmb for $label" >&2; continue; }
    # Keyed by the ACMD FN, which is what rebase derives from the generated function's name.
    "$T" "$src" "$OUT/$fn.json" >/dev/null 2>&1 \
        && n=$((n+1)) \
        || echo "  could not export $stem.nuanmb" >&2
done < tools/moves.tsv

echo "  rebase-anims: $n animation(s) exported for the grafted-bone rebase"
