#!/usr/bin/env bash
# Pulls the lua2cpp fighter NROs out of the running game, for the functions the public decomp
# left out.
#
# WHAT IS MISSING AND WHY IT MATTERS. vendor/ssbu-status has every NAMED status function for the
# fighters this moveset ports from, and that has already settled Revenge's absorb, G&W's two up-B
# params and DK's whole carry. What it does not have is the unnamed `FUN_*` helpers, because the
# dump only emitted functions it could name. Three answers live in them:
#
#   FUN_7100000e70   gaogaen  SetStatusScripts delegates here -- every status registration
#                             except the Final Smash, including whether SPECIAL_LW registers a
#                             CHECK_DAMAGE line (which is how a counter would natively detect a
#                             hit)
#   FUN_710001d4b0   gaogaen  called from SpecialS/main_loop -- the command grab's catch check
#   FUN_710001ff80   gaogaen  the other half of the same
#   FUN_7100022520   donkey   called from every Shoulder loop -- the cargo throw input check
#
# NONE OF THEM BLOCKS ANYTHING TODAY. The catch is detected victim-side instead, which observes
# the outcome directly rather than reproducing the test; the counter transitions on a measured
# damage delta; the throw input is the documented attack-plus-direction. They would be
# confirmatory, and the place to start if one of those three turns out wrong.
#
# HOW. The arc tooling reads data.arc out of the RUNNING GAME over arc_network -- the same path
# the c00 extraction used -- because the copy on the card is encrypted and dumping it needs
# prod.keys we do not have. So this needs Smash open with the arc_network plugin loaded, exactly
# like tools/arc-pull. Nothing else: no keys, no nxdumptool, no homebrew menu.
#
#   bash tools/pull-nro.sh
#
# Then disassembling them is a separate job and a separate decision. They are AArch64 NROs with
# the standard header; vendor/ssbu-status ships blu-dev's Lua2CPPAnalysis.java, which is the
# Ghidra script that produced the rest of that repo and can be pointed at these.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
OUT=vendor/nro
mkdir -p "$OUT"

FIGHTERS="gaogaen donkey gamewatch packun pikachu"

LIST=work/nro-paths.txt
mkdir -p work
: > "$LIST"
for f in $FIGHTERS; do
    echo "prebuilt:/nro/release/lua2cpp_$f.nro" >> "$LIST"
done
echo "asking for:"
sed 's/^/  /' "$LIST"

# arc-pull reads paths on STDIN and takes host:port then an output directory.
if ! tools/arc-pull/target/release/arc-pull.exe "<console-ip>:43022" "$OUT" < "$LIST" 2>&1 | tail -20; then
    echo
    echo "FAILED. The tooling talks to the game, not to the card:"
    echo "  * Smash must be running with the arc_network plugin loaded"
    echo "  * the console must be reachable at the address arc-pull uses"
    echo "This is the same requirement as the c00 extraction, and the reason is that the"
    echo "data.arc on the card is encrypted -- decrypting it needs prod.keys, which this setup"
    echo "does not have. The running game has already decrypted it."
    exit 1
fi

echo
ls -l "$OUT" 2>/dev/null
