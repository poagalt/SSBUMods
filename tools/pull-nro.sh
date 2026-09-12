#!/usr/bin/env bash
# Pulls the lua2cpp fighter NROs out of the running game, for the functions the public decomp
# left out.
#
# WHAT IS MISSING AND WHY IT MATTERS. vendor/ssbu-status has every NAMED status function for the
# fighters this moveset ports from, and that has already settled Revenge's absorb, G&W's two up-B
# params and DK's whole carry. What it does not have is the unnamed `FUN_*` helpers, because the
# dump only emitted functions it could name. Three answers live in them:
#
#   FUN_7100000e70   gaogaen  ANSWERED. 53 sv_set_status_func calls using four conditions:
#                             three appearing 16/16/15 times (pre, main, end, one per status) and
#                             one appearing 6 times. The only gaogaen statuses with a file beyond
#                             pre/main/main_loop/end are the six SpecialHi* with check_atk.c, so
#                             the outlier is CHECK_ATTACK on his up-B. SPECIAL_LW registers pre,
#                             main and end and NOTHING ELSE -- there is no native CHECK_DAMAGE
#                             line for a counter to hang off. See docs/reading-the-games-code.md.
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

# EVERY SOURCE FIGHTER IN SPEC.md, not just the five with ported STATUS code.
#
# The original five were the fighters whose status trees this moveset transcribes. That was the
# wrong boundary, and it was drawn by accident: SPEC.md lists TWENTY-ONE source fighters, and for
# the other sixteen the port was assembled from `vendor/SSBU-Dumped-Scripts` (ACMD only -- hitboxes
# and timing, no status logic) and from param prcs. Those two sources answer "what does the hitbox
# do"; neither answers "what does the move DO", which is why a question like Little Mac's f-smash
# super armour or K. Rool's d-air had to be reasoned about rather than read.
#
# So the boundary is now SPEC.md's source-fighter column. Cheap -- one arc-pull run, a couple of
# hundred MB -- and it retires the whole category of "that fighter's code was never available".
#
# INTERNAL NAMES, since almost none of them match the character: Mythra is `elight` (Pyra is
# `eflame`), Kazuya is `demon`, Corrin is `kamui`, Bowser is `koopa`, R.O.B. is `robot`, Ganondorf
# is `ganon`, Piranha Plant is `packun`, Incineroar is `gaogaen`. Checked against the 96 directory
# names in vendor/SSBU-Dumped-Scripts/smashline, not guessed.
#
# The attribute-only sources in SPEC.md's second table -- Zero Suit Samus, Marth, Jigglypuff, Joker
# -- are deliberately NOT here. Weight, walk speed and fall speed come out of `fighter_param_vl`,
# which is a prc, so their NROs would answer nothing.
FIGHTERS="${*:-gaogaen donkey gamewatch packun pikachu chrom elight falco robot littlemac fox palutena demon cloud kamui krool mewtwo koopa ganon}"

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
