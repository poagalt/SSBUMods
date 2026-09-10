#!/usr/bin/env bash
# Generates the forward-kinematics inputs tools/trace-all.sh needs: each source fighter's skeleton
# and the specific animation a traced move plays, as ssbh JSON.
#
# WHY THIS HAD TO EXIST. These files were created by hand in earlier sessions and nothing
# regenerated them, so they were undeclared build inputs -- and a move added to tools/moves.tsv
# without them does not fail, it just silently stops being measured. trace-all prints
# "no FK data (skeleton or animation not dumped)" while check-hitboxes.sh only failed on the
# string "no FK skeleton", so the two never met. That is the fifth false-clean of this shape on
# this project, and the pattern is always the same: a check that reports success on work it did not
# do.
#
# Everything it needs is already local. The full c00 extraction pulled every fighter's
# model.nusktb and every .nuanmb, so this is a pure local transform with no console involved.
#
# Idempotent and incremental: a dump is skipped if it is newer than its source.
#
# Regenerate with:  bash tools/gen-fk.sh
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
T="vendor/ssbh_lib/ssbh_data_json.exe"
EX="extracted/fighter"
FK="work/fk"
mkdir -p "$FK"

[ -x "$T" ] || { echo "no ssbh_data_json at $T" >&2; exit 1; }

made=0; skipped=0; missing=0

dump() {
    local src="$1" out="$2" what="$3"
    if [ ! -f "$src" ]; then
        printf "  %-40s MISSING SOURCE %s\n" "$what" "$src"
        missing=$((missing+1))
        return
    fi
    if [ -f "$out" ] && [ "$out" -nt "$src" ]; then
        skipped=$((skipped+1))
        return
    fi
    if "$T" "$src" "$out" >/dev/null 2>&1; then
        made=$((made+1))
    else
        printf "  %-40s FAILED to convert\n" "$what"
        missing=$((missing+1))
    fi
}

# Pikachu's SHIPPED skeleton, which is the grafted one the mod installs -- not his vanilla one.
# The trace measures what we ship, so this has to come from the mod tree.
dump "pikachu-moveset/mod/fighter/pikachu/model/body/c00/model.nusktb" \
     "$FK/pikachu_shipped_skel.json" "pikachu shipped skeleton"

# One skeleton per source fighter, plus the animation each traced move plays.
while read -r f acmd fn stem mod label; do
    [ -z "${f// }" ] && continue
    case "$f" in \#*) continue ;; esac
    dump "$EX/$f/model/body/c00/model.nusktb" "$FK/${f}_skel.json" "$f skeleton"
    dump "$EX/$f/motion/body/c00/${stem}.nuanmb" "$FK/${f}_${stem}.json" "$f/$stem"
done < tools/moves.tsv

echo "  fk dumps: $made written, $skipped already current, $missing unavailable"
[ "$missing" -eq 0 ] || exit 1
