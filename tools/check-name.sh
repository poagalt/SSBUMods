#!/usr/bin/env bash
# Parity harness: the CSS rename must be UTF-16LE with a BOM and carry every label.
#
# The mirror of the prcxml check, and deliberately a DIFFERENT constraint. prcxml must be UTF-8
# (ARCropolis parses it with prcx + xml-rs); xmsbt must be UTF-16 (msbt is a UTF-16 container).
# Shipping prcxml as UTF-16 silently dropped every attribute change, so each format is now
# checked against its own real requirement rather than one assumed rule.
#
# Labels were read out of a full vanilla msg_name.msbt, not from a guide.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
F="$ROOT/pikachu-moveset/mod/ui/message/msg_name.xmsbt"
rc=0

[ -f "$F" ] || { echo "  FAIL no msg_name.xmsbt shipped"; exit 1; }
if [ "$(xxd -l 2 -p "$F")" != "fffe" ]; then
    echo "  FAIL msg_name.xmsbt is not UTF-16LE with a BOM -- msbt is a UTF-16 container"; rc=1
fi
txt=$(iconv -f UTF-16LE -t UTF-8 < "$F" 2>/dev/null) || { echo "  FAIL does not decode as UTF-16LE"; exit 1; }

for l in nam_chr0_00_pikachu nam_chr1_00_pikachu nam_chr2_00_pikachu nam_chr3_00_pikachu nam_stage_name_00_pikachu; do
    printf '%s' "$txt" | grep -q "label=\"$l\"" || { echo "  FAIL missing label $l"; rc=1; }
done

o=$(printf '%s' "$txt" | grep -c "<entry "); c=$(printf '%s' "$txt" | grep -c "</entry>")
t=$(printf '%s' "$txt" | grep -c "<text>")
[ "$o" = "$c" ] || { echo "  FAIL $o <entry> against $c </entry>"; rc=1; }
[ "$o" = "$t" ] || { echo "  FAIL $o entries but $t <text> elements"; rc=1; }
printf '%s' "$txt" | grep -q "<xmsbt>" && printf '%s' "$txt" | grep -q "</xmsbt>" \
  || { echo "  FAIL xmsbt root element not balanced"; rc=1; }

[ "$rc" -eq 0 ] && echo "  ok  UTF-16LE with BOM, $o entries, all 5 labels present"
exit "$rc"
