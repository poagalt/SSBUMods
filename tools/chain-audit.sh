#!/usr/bin/env bash
# Reports, per ported move, the bones that lie on the FK chain to one of its hitboxes but do NOT
# exist on the source fighter.
#
# Such a bone is the last remaining source of hitbox error under ADR 0003. The dropped-in
# animation has no track for it, so Pikachu's own bind transform stays in the chain and inserts an
# offset the source never had. Fox's u-smash is the clear case: his leg chain is Hip > LegR, while
# Pikachu's is Hip > LegC > LegR, and that extra LegC accounted for the 0.46-0.69 world units of
# error left after scale compensation.
#
# DIAGNOSTIC, NOT A GATE, and deliberately so. tools/check-hitboxes.sh measures the actual error
# and would fail a move whose chain bone was inserting one; this says WHY. The one finding it still
# reports -- Fox's u-smash and `legc` -- is the case tools/anim-dropin.sh neutralises with an
# identity track, and hbtrace measures that move at 0.00 world units. So an EXTRA line here means
# "handled, here is the mechanism", and a nonzero deviation over there means "not handled".
#
# Its column read was also wrong: it destructured moves.tsv into seven fields when the table has
# six, so `label` was always empty and every line printed with a blank name.
#
# Note on jq: it emits CRLF on Windows, so every comparison here strips \r. Not doing that on both
# sides of a comparison reported all chain bones as missing -- a convincing-looking false result.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

jqr() { jq -r "$@" | tr -d '\r'; }

chainlist() { # chainlist <skel.json> <bone> -> chain bone names, lowercased, root first
    jqr --arg b "$2" '
      (.bones | to_entries) as $bs
      | ($bs[] | select(.value.name|ascii_downcase == ($b|ascii_downcase)) | .key) as $s
      | [$s]
      | until(.[-1] == null or .[-1] < 0; . + [ $bs[.[-1]].value.parent_index ])
      | map(select(. != null and . >= 0)) | map($bs[.].value.name | ascii_downcase) | .[]' "$2" 2>/dev/null
}

total=0
while read -r f acmd fn stem mod label; do
    [ -z "${f// }" ] && continue
    case "$f" in \#*) continue ;; esac
    [ -f "work/fk/${f}_skel.json" ] || { printf "  %-22s -- no FK skeleton\n" "$label"; continue; }

    jqr '.bones[].name | ascii_downcase' "work/fk/${f}_skel.json" | sort -u > /tmp/_src_bones.txt
    awk -v w="pikachu_game_$fn" '
        index($0, "unsafe extern \"C\" fn ") { i = (index($0, "fn " w "(") > 0) }
        i && index($0, "macros::ATTACK(")' "pikachu-moveset/plugin/src/$mod/mod.rs" \
      | grep -oE 'Hash40::new\("[a-z0-9_]+"\), [-0-9.]+, [-0-9]+' \
      | sed 's/Hash40::new("//; s/").*//' | sort -u > /tmp/_anchors.txt

    extra=""
    while read -r b; do
        [ -z "$b" ] && continue
        [ "$b" = "top" ] && continue
        jqr --arg b "$b" '
          (.bones | to_entries) as $bs
          | ($bs[] | select(.value.name|ascii_downcase == ($b|ascii_downcase)) | .key) as $s
          | [$s]
          | until(.[-1] == null or .[-1] < 0; . + [ $bs[.[-1]].value.parent_index ])
          | map(select(. != null and . >= 0)) | map($bs[.].value.name | ascii_downcase) | .[]' \
          work/fk/pikachu_shipped_skel.json 2>/dev/null > /tmp/_chain.txt
        while read -r cb; do
            [ -z "$cb" ] && continue
            grep -qx "$cb" /tmp/_src_bones.txt || extra="$extra $cb"
        done < /tmp/_chain.txt
    done < /tmp/_anchors.txt

    uniq=$(echo "$extra" | tr ' ' '\n' | grep -v '^$' | sort -u | tr '\n' ' ')
    if [ -z "$uniq" ]; then
        printf "  %-22s ok -- every chain bone exists on %s\n" "$label" "$f"
    else
        printf "  %-22s EXTRA: %s\n" "$label" "$uniq"
        total=$((total+1))
    fi
done < tools/moves.tsv
echo
echo "$total move(s) have a Pikachu-only bone in a hitbox chain"
