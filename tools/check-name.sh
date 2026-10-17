#!/usr/bin/env bash
# Parity harness: the character-select rename must be real UTF-16LE and carry every label.
#
# The mirror of the prcxml check, and deliberately the OPPOSITE constraint. prcxml must be UTF-8
# (ARCropolis parses it with prcx + xml-rs); xmsbt must be UTF-16 (msbt is a UTF-16 container).
# Shipping the prcxml as UTF-16 silently dropped every attribute change, so each format is now
# checked against its own real requirement rather than one assumed rule.
#
# Labels were read out of a full vanilla msg_name.msbt, not from a guide. The two-digit number is
# part of the label and is per-character: Pikachu's is 00, Ganondorf's is 44.
#
# Decoding is done in awk rather than with iconv or `tr -d`. iconv failed five times in a row
# immediately after a write and then passed on every later read of a byte-identical file, which I
# could not explain and would not trust in a gate; and `tr -d` needed a NUL byte written literally
# into this script, which made it a binary file. awk reading the bytes is deterministic and leaves
# the script plain text.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
F="$ROOT/pikachu-moveset/mod/ui/message/msg_name.xmsbt"
rc=0

[ -f "$F" ] || { echo "  FAIL no msg_name.xmsbt shipped"; exit 1; }

# Two separate facts, because a BOM alone does not prove the body is UTF-16: bytes 0-1 are the
# little-endian BOM, and byte 3 is the high byte of the first character ('<' stored as 3c 00).
# Checking only the BOM let a NUL-stripped file pass, since stripping NULs leaves ff fe in place.
head4=$(xxd -l 4 -p "$F")
if [ "${head4:0:4}" != "fffe" ]; then
    echo "  FAIL msg_name.xmsbt is not UTF-16LE with a BOM -- msbt is a UTF-16 container"
    rc=1
elif [ "${head4:6:2}" != "00" ]; then
    echo "  FAIL msg_name.xmsbt has a UTF-16 BOM but a single-byte body"
    rc=1
fi

# The content is pure ASCII held as UTF-16LE, so removing the NUL bytes recovers it.
#
# Done in perl, and that matters. The awk version used strtonum(), which is a GNU extension: the
# check passed when run on its own and failed inside tools/build.sh, because the build puts
# w64devkit on PATH ahead of it and that awk has no strtonum. A gate whose result depends on which
# awk is first on PATH is worse than no gate. perl is used throughout this repo's tooling.
txt=$(perl -0777 -ne 's/\x00//g; print' "$F")
[ -n "$txt" ] || { echo "  FAIL msg_name.xmsbt is empty or could not be decoded"; exit 1; }

# THE LABELS CARRY THE ROW'S name_id, NOT THE HOST FIGHTER'S, and this gate asserted the host's
# until 2026-10-17 -- which is what let the bug ship. A label ending `_pikachu` is the display name
# of the fighter called `pikachu`, so the mod's name went over VANILLA Pikachu on the character
# select screen while its own row had no name text at all. Reported from hardware in exactly those
# words. Read from the plugin's registration so the two cannot drift.
NAMEID=$(grep 'CStrCSK::new(' "$ROOT/pikachu-moveset/plugin/src/charaslot/mod.rs" | head -1 | awk -F'"' '{print $2}')
if [ -z "${NAMEID:-}" ]; then
    echo "  FAIL cannot read the row's name_id out of plugin/src/charaslot/mod.rs"; exit 1
fi
for l in nam_chr0_00_$NAMEID nam_chr1_00_$NAMEID nam_chr2_00_$NAMEID nam_chr3_00_$NAMEID          nam_stage_name_00_$NAMEID; do
    printf '%s' "$txt" | grep -q "label=\"$l\"" || { echo "  FAIL missing label $l"; rc=1; }
done

o=$(printf '%s' "$txt" | grep -c "<entry ")
c=$(printf '%s' "$txt" | grep -c "</entry>")
t=$(printf '%s' "$txt" | grep -c "<text>")
[ "$o" = "$c" ] || { echo "  FAIL $o <entry> against $c </entry>"; rc=1; }
[ "$o" = "$t" ] || { echo "  FAIL $o entries but $t <text> elements"; rc=1; }
printf '%s' "$txt" | grep -q "<xmsbt>" && printf '%s' "$txt" | grep -q "</xmsbt>" \
  || { echo "  FAIL xmsbt root element not balanced"; rc=1; }

[ "$rc" -eq 0 ] && echo "  ok  real UTF-16LE, $o entries, all 5 labels present"
exit "$rc"
