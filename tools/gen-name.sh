#!/usr/bin/env bash
# Generates ui/message/msg_name.xmsbt, which renames the fighter on the character select screen,
# the versus splash, the results screen and the Boxing Ring title.
#
# ENCODING: UTF-16LE WITH A BOM. This is the opposite of fighter_param.prcxml, which must be
# UTF-8 -- shipping that one as UTF-16 meant ARCropolis silently dropped it and not one attribute
# change applied. The two formats genuinely differ: msbt is a UTF-16 text container, and
# ARCropolis converts before parsing. Verified against a working example already installed on the
# console ("Shrek over Ganondorf" ships ui/message/msg_name.xmsbt, byte 0 = ff fe).
#
# LABELS come from the game's own data, not from a guide. A full vanilla msg_name.msbt was read
# and grepped for pikachu, which gives exactly:
#   nam_chr0_00_pikachu        nam_chr1_00_pikachu        nam_chr2_00_pikachu
#   nam_chr3_00_pikachu        nam_chr_no_00_pikachu      nam_stage_name_00_pikachu
# The two-digit number is part of the label and is per-character -- Pikachu's is 00, Ganondorf's
# is 44 -- so it cannot be assumed to be a costume slot index.
#
# `nam_chr_no` is deliberately left alone: its purpose is unclear and the working reference mod
# does not touch it.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/pikachu-moveset/mod/ui/message/msg_name.xmsbt"

# THE LABEL CARRIES THE ROW'S OWN name_id, NOT THE HOST FIGHTER'S -- and writing `pikachu` here is
# what put "Rat of Doom" over VANILLA PIKACHU on the character select screen while the mod's own
# row had no name at all. Reported from hardware 2026-10-17 and it is exactly what these labels do:
# `nam_chr0_00_pikachu` is the display name of the fighter called `pikachu`, so overriding it
# renames HIM. The mod's row is called `ratofdoom` (the `name_id` registered in
# plugin/src/charaslot/mod.rs) and had no `nam_chr*_00_ratofdoom` entry to find.
#
# `Documents/switch/Sans Moveset` confirms the convention outright: every one of its labels is
# `nam_*_00_sans`, against a `Name ID: sans` in its README, and it renames nothing of Palutena's.
#
# Parsed from the Rust rather than restated, for the same reason the slot number is.
SLOTNAME=$(grep 'CStrCSK::new(' "$ROOT/pikachu-moveset/plugin/src/charaslot/mod.rs" | head -1 | awk -F'"' '{print $2}')
if [ -z "${SLOTNAME:-}" ]; then
    echo "  REFUSING: cannot read the row's name_id out of plugin/src/charaslot/mod.rs" >&2
    exit 1
fi
mkdir -p "$(dirname "$OUT")"

NAME="Rat of Doom"
UPPER="RAT OF DOOM"
# The Boxing Ring title renders on two lines; a literal newline is what the reference mod uses.
TITLE="The Electric Terror"

tmp=$(mktemp)
{
    echo '<?xml version="1.0" encoding="utf-16"?>'
    echo '<xmsbt>'
    for l in nam_chr0_00_$SLOTNAME nam_chr1_00_$SLOTNAME nam_chr3_00_$SLOTNAME; do
        printf '\t<entry label="%s">\n\t\t<text>%s</text>\n\t</entry>\n' "$l" "$NAME"
    done
    printf '\t<entry label="%s">\n\t\t<text>%s</text>\n\t</entry>\n' "nam_chr2_00_$SLOTNAME" "$UPPER"
    printf '\t<entry label="%s">\n\t\t<text>%s\n%s</text>\n\t</entry>\n' \
           "nam_stage_name_00_$SLOTNAME" "$NAME" "$TITLE"
    echo '</xmsbt>'
} > "$tmp"

# Assemble the whole file, THEN move it into place. The previous version wrote the BOM and
# appended the converted body as two separate redirections to the live path, so anything reading
# in between saw a half-written file -- tools/check-name.sh failed immediately after a build with
# "5 <entry> against 0 </entry>" and then passed on the next read, with the file byte-identical.
# An intermittent check failure is worse than none, because it teaches you to re-run instead of
# to look. Same atomic pattern as tools/gen-params.sh.
out_tmp=$(mktemp)
printf '\xff\xfe' > "$out_tmp"
iconv -f UTF-8 -t UTF-16LE < "$tmp" >> "$out_tmp"
rm -f "$tmp"
mv "$out_tmp" "$OUT"
echo "wrote $OUT ($(wc -c < "$OUT") bytes, UTF-16LE with BOM)"
