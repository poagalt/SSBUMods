#!/usr/bin/env bash
# Ships each source fighter's own animation in Pikachu's motion directory, so a ported move's
# verbatim bone-anchored hitboxes land exactly where the source's do. See ADR 0003.
#
# SSBU names an animation the same on every fighter -- motion `attack_s3_s` is always
# `c01attacks3s.nuanmb` -- so this is a same-path replacement. No motion_list or config.json
# change is needed, and tools/motion-patch/target/release/dropin.exe pre-flights the flags.
#
# A move listed here MUST have its ACMD ported with the source's original bone anchors intact.
# Re-anchoring a hitbox to `top` on top of a dropped-in animation would double-compensate.
#
# SCALE COMPENSATION
# The game multiplies a fighter's whole skeleton by its `scale` param, and Pikachu's is 0.96
# against e.g. Mythra's 1.112. A raw drop-in therefore lands short of the source's world reach --
# reported in playtest as Fox's u-smash lacking horizontal range and Little Mac's f-smash arm
# being too short.
#
# The correction multiplies the `Rot` node by ratio = source_scale / 0.96. `Rot` is the root of
# the posed skeleton (chain: Trans > Rot > Hip > ...), and `compensate_scale` is false throughout
# these animations, so a scale there inherits down every bone. Because a hitbox's bone-local
# offset is transformed by the bone's world matrix, that inherited scale corrects the offsets too
# -- one node instead of ~100 tracks. `Rot`'s own translation is the standing height and is
# scaled with it.
#
# Scales come from tools/scales.tsv, generated from the game's own fighter_param.prc. They used to
# be hand-entered here and two were wrong (Cloud 1.03 vs 0.98, K. Rool 1.15 vs 0.89 -- a 29%
# over-scale on his d-air) and the hitbox trace could not see it, because it applied the same
# wrong constant to both sides of its comparison.
#
# `Trans` is deliberately untouched: its translation is root motion, which the game consumes to
# move the fighter. Measured on hardware -- Little Mac's f-smash travels +20.59 world units
# against his own 19.72, within 4.4% -- so no correction is applied there.
set -euo pipefail
shopt -s extglob
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/extracted/fighter"
DST="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
W="$ROOT/work/anim"
SCALES="$ROOT/tools/scales.tsv"
mkdir -p "$DST" "$W" "$ROOT/work/fk"
[ -f "$SCALES" ] || { echo "no tools/scales.tsv -- run tools/gen-scales.sh" >&2; exit 1; }

PIKA_SCALE=$(awk -F'\t' '$1=="pikachu"{print $3}' "$SCALES")
[ -n "$PIKA_SCALE" ] || { echo "pikachu missing from scales.tsv" >&2; exit 1; }

# WHAT PIKACHU ACTUALLY HAS, so tracks for everything else can be thrown away.
#
# THIS IS A CORRECTNESS FIX WEARING A SIZE FIX'S CLOTHES. A dropped-in animation is the SOURCE
# fighter's file, and it carries a track per source bone and per source mesh. Incineroar's
# `d01specialsstart` has 123 Transform nodes, 57 Visibility nodes and 4 Material nodes -- and
# Pikachu's skeleton has 80 bones, of which the animation names 46. **77 of those 123 transform
# tracks are for bones that do not exist on him**, and all 57 visibility tracks are for meshes
# that do not exist on him either.
#
# The engine matches tracks to bones by name and ignores the rest, so they were harmless and
# enormous: `d01specialsstart` ships at 155632 bytes against vanilla's 21728, a factor of 7.16.
# Across the whole directory the port was taking Pikachu's animation set from 9720 KiB to 13393
# KiB -- 38% growth -- and the in-game self-test had started reporting 25 of the 30 added files
# UNRESOLVED, nondeterministically, differently between two loads of the same build. A fighter's
# loaded animation set has a memory budget; 38% over it fails some of the time depending on what
# else is resident, which is exactly the shape of what was observed.
#
# The Visibility tracks are worse than dead weight. Fifty-seven of them, naming meshes Pikachu
# does not have -- and this project has already lost a session to a working up-B that left him
# invisible. A visibility track that matches a Pikachu mesh by accident would do precisely that.
#
# So: keep a track only if the thing it animates exists. Nothing that has any effect is removed,
# which is what makes this safe to do to all 74 files at once.
BONES_JSON="$W/pika-bones.json"
# The realised-bone pairs, from merge-weapons.sh's `realise` column: a source bone that is not
# in Pikachu's skeleton and the vanilla bone put into its frame instead. `from` is lowercased for
# matching, `to` keeps the table's spelling, `to_lc` is for dropping the target's own track.
REALISE_JSON="$W/realise.json"
MESHES_JSON="$W/pika-meshes.json"
MODEL="$ROOT/pikachu-moveset/mod/fighter/pikachu/model/body/c00"
"$T" "$MODEL/model.nusktb" "$W/_skel.json" >/dev/null
"$T" "$MODEL/model.numshb" "$W/_mesh.json" >/dev/null
jq -c '[.bones[].name | ascii_downcase] | unique' "$W/_skel.json" > "$BONES_JSON"
{ printf '['
  rfirst=1
  while IFS='|' read -r _n _d _m _mat wbone _p _sp _r wreal; do
      [ -z "${wreal:-}" ] && continue
      [ "$rfirst" = 1 ] && rfirst=0 || printf ','
      printf '{"from":"%s","to":"%s","to_lc":"%s"}' \
          "$(printf '%s' "$wbone" | tr 'A-Z' 'a-z')" \
          "$wreal" \
          "$(printf '%s' "$wreal" | tr 'A-Z' 'a-z')"
  done < <(awk -F'|' '/^[a-z_]+\|/' "$ROOT/tools/merge-weapons.sh")
  printf ']'
} > "$REALISE_JSON"
jq -c '[.objects[].name | ascii_downcase] | unique' "$W/_mesh.json" > "$MESHES_JSON"
echo "  pikachu has $(jq -r 'length' "$BONES_JSON") bones and $(jq -r 'length' "$MESHES_JSON") meshes; foreign tracks will be dropped"

# The whole table read once into memory, and the ratio computed once per FIGHTER rather than once
# per animation. There are 74 animations and about fifteen fighters, and an `awk` for each of
# those two lookups was 148 process spawns -- roughly nine seconds here, spent re-deriving
# fifteen numbers.
declare -A SCALE=() RATIO=()
while IFS=$'\t' read -r f _ sc _; do
    [ -n "${f:-}" ] && SCALE["$f"]="$sc"
done < "$SCALES"

ratio_of() {
    if [ -z "${RATIO[$1]:-}" ]; then
        local s="${SCALE[$1]:-}"
        [ -n "$s" ] || { echo "no scale for fighter '$1' in scales.tsv" >&2; exit 1; }
        RATIO["$1"]=$(awk -v a="$s" -v b="$PIKA_SCALE" 'BEGIN{printf "%.6f", a/b}')
    fi
    printf '%s' "${RATIO[$1]}"
}

# NEUTRALISED BONES
# A bone Pikachu has but the source does not gets no track from the dropped-in animation, so his
# own bind transform stays in the chain and inserts an offset the source never had. Giving it an
# identity track removes it. tools/chain-audit.sh finds these.
#
# Little Mac's ANGLED f-smash animations are filenames Pikachu does not have, so they are NEW
# paths: they need a config.json new-dir-files entry AND tools/motion-patch synth must point
# attack_s4_hi/attack_s4_lw at them (adopt_anim). Without both they simply do not load.
#
# source fighter | animation file | neutralise | move it drives
MOVES="
littlemac | c00attack11.nuanmb        | -    | jab
littlemac | c00attack12.nuanmb        | -    | jab 2 (new file)
littlemac | c00attack13.nuanmb        | -    | jab 3 (new file)
littlemac | c00attack100start.nuanmb  | -    | rapid jab windup (new file)
littlemac | c00attack100.nuanmb       | -    | rapid jab loop (new file)
littlemac | c00attackend.nuanmb       | -    | rapid jab finisher -- Mac's attack_100_end entry names THIS stem, not c00attack100end (new file)
elight    | c00attackdash.nuanmb      | -    | dash attack
sheik     | c01attacks3s.nuanmb       | -    | f-tilt (all three angles; she has no angled animations)
falco     | c02attackhi3.nuanmb       | -    | u-tilt
gaogaen   | c02attacklw3.nuanmb       | -    | d-tilt
littlemac | c03attacks4s.nuanmb       | -    | f-smash, straight
littlemac | c03attacks4hi.nuanmb      | -    | f-smash, up-angled (new file: see config.json new-dir-files)
littlemac | c03attacks4lw.nuanmb      | -    | f-smash, down-angled (new file: see config.json new-dir-files)
littlemac | c03attacks4charge.nuanmb  | -    | f-smash charge hold
fox       | c04attackhi4.nuanmb       | LegC | u-smash
fox       | c04attackhi4charge.nuanmb | -    | u-smash charge hold
gamewatch | c04attacklw4.nuanmb       | -    | d-smash
gamewatch | c04attacklw4charge.nuanmb | -    | d-smash charge hold
master    | c05attackairn.nuanmb      | -    | n-air
wolf      | c05attackairf.nuanmb      | -    | f-air
cloud     | c05attackairb.nuanmb      | -    | b-air
kamui     | c05attackairhi.nuanmb     | -    | u-air
eflame    | c05attackairlw.nuanmb     | -    | d-air
master    | c05landingairn.nuanmb     | -    | n-air landing
wolf      | c05landingairf.nuanmb     | -    | f-air landing pose (Kazuya's, then Falco's; f-air is Wolf's now)
cloud     | c05landingairb.nuanmb     | -    | b-air landing
kamui     | c05landingairhi.nuanmb    | -    | u-air landing
eflame    | c05landingairlw.nuanmb    | -    | d-air landing
plizardon | e00catch.nuanmb           | -    | grab
plizardon | e00catchdash.nuanmb       | -    | dash grab
plizardon | e00catchturn.nuanmb       | -    | pivot grab
plizardon | e00catchwait.nuanmb       | -    | grab hold
plizardon | e00catchpull.nuanmb       | -    | grab pull
plizardon | e00catchcut.nuanmb        | -    | grab break
demon     | e00catchattack.nuanmb     | -    | pummel
ness      | e01throwb.nuanmb          | -    | b-throw
mewtwo    | e01throwhi.nuanmb         | -    | u-throw
mariod    | e01throwlw.nuanmb        | -    | d-throw
koopa     | f03downattacku.nuanmb     | -    | get-up attack, face up
koopa     | f04downattackd.nuanmb     | -    | get-up attack, face down
koopa     | g03slipattack.nuanmb      | -    | trip attack
ganon     | g02cliffattack.nuanmb     | -    | ledge attack
fox       | b02escapeair.nuanmb       | -    | air dodge (and directional, which shares it)
fox       | b02escapef.nuanmb         | -    | forward roll
gaogaen   | d03speciallw.nuanmb       | -    | Revenge counter attack (same name as Pikachu's Thunder)
gaogaen   | d03speciallwstart.nuanmb  | -    | Revenge stance (new file: config.json new-dir-files)
gaogaen   | d03speciallwturn.nuanmb   | -    | Revenge stance, turning (new file)
gaogaen   | d03specialairlw.nuanmb    | -    | AIR Revenge counter attack (same name as Pikachu air Thunder)
gaogaen   | d03specialairlwstart.nuanmb| -   | AIR Revenge stance (new file)
gaogaen   | d03specialairlwturn.nuanmb| -    | AIR Revenge stance, turning (new file)
gaogaen   | d01specialsstart.nuanmb   | -    | Alolan Whip lunge (same name as Skull Bash start)
gaogaen   | d01specialsend.nuanmb     | -    | Alolan Whip end (same name)
gaogaen   | d01specialsshoulder.nuanmb| -    | Alolan Whip shoulder toss (new file)
gaogaen   | d01specialslariat.nuanmb  | -    | Alolan Whip lariat (new file)
gaogaen   | d01specialsthrow.nuanmb   | -    | Alolan Whip throw (new file)
gaogaen   | d01specialsfailure.nuanmb | -    | Alolan Whip whiff (new file)
gaogaen   | d01specialairsstart.nuanmb| -    | Alolan Whip AIR lunge (same name as air Skull Bash start)
gaogaen   | d01specialairsend.nuanmb  | -    | Alolan Whip AIR end (same name)
gaogaen   | d01specialairsthrow.nuanmb| -    | Alolan Whip AIR throw (new file)
gaogaen   | d01specialairsshoulder.nuanmb| - | Alolan Whip AIR shoulder (new file)
gaogaen   | d01specialairslariat.nuanmb| -   | Alolan Whip AIR lariat (new file)
gaogaen   | d01specialairsfailure.nuanmb| -  | Alolan Whip AIR whiff (new file)
donkey    | e01throwf.nuanmb          | -    | cargo grab (same name as Pikachu's f-throw)
donkey    | e01throwfwait.nuanmb      | -    | cargo carry, standing (new file)
donkey    | e01throwfwalkslow.nuanmb  | -    | cargo carry, walking slow (new file)
donkey    | e01throwfwalkmiddle.nuanmb| -    | cargo carry, walking (new file)
donkey    | e01throwfwalkfast.nuanmb  | -    | cargo carry, walking fast (new file)
donkey    | e01throwfturn.nuanmb      | -    | cargo carry, turning (new file)
donkey    | e01throwfjumpsquat.nuanmb | -    | cargo carry, jump squat (new file)
donkey    | e01throwfjump.nuanmb      | -    | cargo carry, jumping (new file)
donkey    | e01throwffall.nuanmb      | -    | cargo carry, falling (new file)
donkey    | e01throwflanding.nuanmb   | -    | cargo carry, landing (new file)
packun    | d00specialnstart.nuanmb             | -    | Ptooie start (new file)
packun    | d00specialnblow.nuanmb              | -    | Ptooie blow (new file)
packun    | d00specialnwait.nuanmb              | -    | Ptooie wait (new file)
packun    | d00specialnshootf.nuanmb            | -    | Ptooie shoot f (new file)
packun    | d00specialnshootb.nuanmb            | -    | Ptooie shoot b (new file)
packun    | d00specialnend.nuanmb               | -    | Ptooie end (new file)
packun    | d00specialnhit.nuanmb               | -    | Ptooie hit (new file)
packun    | d00specialnfailure.nuanmb           | -    | Ptooie failure (new file)
packun    | d00specialairnstart.nuanmb          | -    | AIR Ptooie start (new file)
packun    | d00specialairnblow.nuanmb           | -    | AIR Ptooie blow (new file)
packun    | d00specialairnwait.nuanmb           | -    | AIR Ptooie wait (new file)
packun    | d00specialairnshootf.nuanmb         | -    | AIR Ptooie shoot f (new file)
packun    | d00specialairnshootb.nuanmb         | -    | AIR Ptooie shoot b (new file)
packun    | d00specialairnend.nuanmb            | -    | AIR Ptooie end (new file)
packun    | d00specialairnhit.nuanmb            | -    | AIR Ptooie hit (new file)
packun    | d00specialairnfailure.nuanmb        | -    | AIR Ptooie failure (new file)
gamewatch | d02specialhi.nuanmb       | -    | up-B Fire, grounded (new file)
gamewatch | d02specialairhi.nuanmb    | -    | up-B Fire, airborne (new file)
gamewatch | d02specialhiopen.nuanmb   | -    | up-B parachute open (new file)
gamewatch | d02specialhiclose.nuanmb  | -    | up-B parachute close (new file)
donkey    | e01throwff.nuanmb         | -    | cargo throw forward (new file)
donkey    | e01throwfb.nuanmb         | -    | cargo throw back (new file)
donkey    | e01throwfhi.nuanmb        | -    | cargo throw up (new file)
donkey    | e01throwflw.nuanmb        | -    | cargo throw down (new file)
"

# The script itself is part of every stamp: change the jq filters or the drop-in table and every
# cached animation must be rebuilt.
#
# AND SO IS THE SKELETON, WHICH IT WAS NOT, AND THAT IS THE SAME HOLE TWICE.
#
# The trim below drops every Transform track whose target is absent from Pikachu's skeleton, so its
# output depends on the BONE LIST as much as on the source animation. The stamp did not include it,
# so changing the skeleton left all 74 animations "unchanged and skipped" and the trim never re-ran.
#
# Caught the first time it mattered. Baking Mythra's sword removed her nine sword bones, taking the
# skeleton from 88 bones to 79 -- and `Sword1` with it, which CHROM's jab animates. His shipped
# animation kept driving a bone that no longer existed, and the build reported a clean
# "74 unchanged and skipped" while doing it. `tools/check-anim-tracks.sh` found it because it reads
# the shipped bytes instead of trusting this cache; the cache is what should not have needed it.
#
# The meshes and the realise pairs go in for the same reason: the Visibility and Material trim
# reads the mesh-name list, and the rename reads the pairs.
SELFSIG="$(sha1sum "$0" | cut -d" " -f1)"
SELFSIG="$SELFSIG:$(cat "$BONES_JSON" "$MESHES_JSON" "$REALISE_JSON" | sha1sum | cut -d" " -f1)"

# EVERY INPUT TO THE CACHE CHECK, GATHERED IN THREE PROCESSES RATHER THAN FIVE PER ANIMATION.
#
# A process spawn under Git Bash on Windows costs about sixty milliseconds, so a per-animation
# `sha1sum`, `stat` and `cat` is 74 x 5 spawns -- twenty-three seconds to decide that nothing
# needs doing, which is most of what the cache was meant to save. One `sha1sum` over all the
# sources, one `stat` over all the outputs and one stamp FILE instead of 74 brings that to
# about a second.
STAMPFILE="$ROOT/work/anim/.stamps"
declare -A SRCHASH=() DSTSIZE=() STAMP=() NEWSTAMP=()
srcs=(); dsts=()
while IFS='|' read -r f a _ _; do
    f="${f// }"; a="${a// }"
    [ -z "$f" ] && continue
    srcs+=("$SRC/$f/motion/body/c00/$a"); dsts+=("$DST/$a")
done <<< "$MOVES"
while read -r h p; do SRCHASH["$p"]="$h"; done < <(sha1sum "${srcs[@]}" 2>/dev/null)
while read -r sz p; do DSTSIZE["$p"]="$sz"; done < <(stat -c '%s %n' -- "${dsts[@]}" 2>/dev/null)
if [ -f "$STAMPFILE" ]; then
    while IFS=$'\t' read -r a v; do [ -n "${a:-}" ] && STAMP["$a"]="$v"; done < "$STAMPFILE"
fi

n=0; miss=0; scaled=0; neutralised=0; cached=0; trimmed=0
while IFS='|' read -r fighter anim neut label; do
    fighter="${fighter// }"; anim="${anim// }"; neut="${neut// }"
    # Trimmed with bash, not `echo | sed`: two more spawns per animation for a string that is
    # only ever printed.
    label="${label##+([[:space:]])}"; label="${label%%+([[:space:]])}"
    [ -z "$fighter" ] && continue
    s="$SRC/$fighter/motion/body/c00/$anim"
    if [ ! -f "$s" ]; then
        echo "  MISSING  $fighter/$anim -- $label" >&2
        miss=$((miss+1)); continue
    fi
    stem="${anim%.nuanmb}"
    ratio="${RATIO[$fighter]:-}"; [ -n "$ratio" ] || ratio=$(ratio_of "$fighter")

    # SKIP AN ANIMATION WHOSE INPUTS HAVE NOT MOVED.
    #
    # Every one of these is a round trip through ssbh_data_json and two to four jq passes, about
    # 0.7 seconds each, and there are 74 of them: 53 of build.sh's 74 seconds, on every build,
    # producing byte-identical output. The deploy manifest already knew this -- it stopped
    # trusting mtimes precisely BECAUSE this step rewrites all 74 files whose bytes never change.
    #
    # The stamp is the content of the source animation plus everything that decides what is done
    # to it: the ratio, the bone-neutralisation list, and this script itself. Anything that could
    # change the output changes the stamp, and the output file's own size is recorded so a
    # deleted or truncated result re-runs rather than being skipped.
    stamp="${SRCHASH[$s]:-nohash}|$ratio|$neut|$SELFSIG:${DSTSIZE[$DST/$anim]:-absent}"
    if [ "${ANIM_NOCACHE:-0}" != "1" ] && [ "${STAMP[$anim]:-}" = "$stamp" ] \
       && [ -n "${DSTSIZE[$DST/$anim]:-}" ]; then
        NEWSTAMP["$anim"]="$stamp"
        n=$((n+1)); cached=$((cached+1))
        continue
    fi

    # THE ~1.0 FAST PATH IS GONE. IT WAS NOT AN OPTIMISATION, IT WAS A HOLE.
    #
    # It read: if the scale ratio is within 0.0005 of 1.0 and nothing needs neutralising, `cp` the
    # source animation across verbatim and skip the JSON round trip. The reasoning was that with no
    # scaling to apply there is nothing to do -- and that was true when the only thing this script
    # did was scale `Rot`. It stopped being true when the FOREIGN-TRACK TRIM was added below, and
    # the trim is unconditional work: it has nothing to do with the ratio.
    #
    # WHAT IT COST, measured rather than supposed. Chrom's `scale` param is 0.9600 and Pikachu's is
    # 0.9600, so his ratio is exactly 1.0000 -- he is the ONLY source fighter in this moveset that
    # lands on the fast path. His jab therefore shipped with every one of Chrom's own bones still
    # in it: 96 Transform tracks against an 88-bone skeleton, 51 of them naming bones Pikachu does
    # not have (fingerl12, fingerl13, fingerl20 ...). An audit of all 74 shipped animations found
    # exactly two with foreign tracks, and the other is `g02cliffattack`, also from a 0.9600
    # fighter. Every animation that took the slow path has zero.
    #
    # AND IT SKIPPED THE SHIPPED DUMP. `cp` never wrote `work/fk/shipped_<stem>.json`, which is the
    # OURS side that `hbtrace` prefers -- so for the one animation with the defect, the harness was
    # measuring a file that was not what shipped, and could not have seen it.
    #
    # The cost of removing it is close to nothing: the stamp cache above already skips any
    # animation whose inputs have not moved, so this only runs on a genuine change, at about 0.7s.
    "$T" "$s" "$W/$stem.in.json" >/dev/null
    jq --argjson r "$ratio" '
      .groups |= map(
        if .group_type == "Transform" then
          .nodes |= map(
            if (.name | ascii_downcase) == "rot" then
              .tracks |= map(
                .values.Transform |= map(
                  .scale.x *= $r | .scale.y *= $r | .scale.z *= $r
                | .translation.x *= $r | .translation.y *= $r | .translation.z *= $r
                )
              )
            else . end
          )
        else . end
      )' "$W/$stem.in.json" > "$W/$stem.out.json"

    rotx() { jq -r '[.groups[]|select(.group_type=="Transform")|.nodes[]|select((.name|ascii_downcase)=="rot")|.tracks[0].values.Transform[0].scale.x]|.[0] // "none"' "$1"; }
    before=$(rotx "$W/$stem.in.json"); after=$(rotx "$W/$stem.out.json")
    if [ "$before" = "none" ]; then
        echo "  REFUSING $anim: no Rot node to scale -- would ship uncompensated" >&2
        exit 1
    fi
    if awk -v r="$ratio" 'BEGIN{exit !(r < 0.9995 || r > 1.0005)}' && [ "$before" = "$after" ]; then
        echo "  REFUSING $anim: Rot scale unchanged ($before) -- the jq filter did not apply" >&2
        exit 1
    fi

    if [ "$neut" != "-" ]; then
        for b in $(echo "$neut" | tr ',' ' '); do
            jq --arg b "$b" '
              ([.groups[] | select(.group_type=="Transform") | .nodes[0]] | .[0]) as $tpl
              | ($tpl.tracks[0].values.Transform | length) as $nf
              | .groups |= map(
                  if .group_type == "Transform" then
                    .nodes |= ( map(select((.name|ascii_downcase) != ($b|ascii_downcase)))
                                + [ $tpl
                                    | .name = $b
                                    | .tracks[0].values.Transform =
                                        [ range($nf) | {
                                            scale:       {x:1.0, y:1.0, z:1.0},
                                            rotation:    {x:0.0, y:0.0, z:0.0, w:1.0},
                                            translation: {x:0.0, y:0.0, z:0.0}
                                          } ] ] )
                  else . end )' "$W/$stem.out.json" > "$W/$stem.neut.json"
            got=$(jq -r --arg b "$b" '[.groups[]|select(.group_type=="Transform")|.nodes[]|select((.name|ascii_downcase)==($b|ascii_downcase))]|length' "$W/$stem.neut.json")
            if [ "$got" != "1" ]; then
                echo "  REFUSING $anim: neutralising $b produced $got nodes, expected 1" >&2
                exit 1
            fi
            mv "$W/$stem.neut.json" "$W/$stem.out.json"
            neutralised=$((neutralised+1))
        done
    fi
    # DROP EVERY TRACK WHOSE TARGET PIKACHU DOES NOT HAVE. Last, so the scale pass and the
    # neutralisation pass above are unaffected by it -- neutralisation ADDS identity tracks
    # for bones Pikachu has and the source lacks, which is the mirror image of this and has
    # to happen first or its new node would be examined before it exists.
    #
    # A Transform group whose nodes all vanish is kept as an empty group rather than removed:
    # the file format allows it, and removing a group changes the group ordering that
    # `Visibility` and `Material` indices are read against.
    # A REALISED BONE IS RENAMED BEFORE THE TRIM, NOT AFTER, and doing it after cost a build.
    #
    # Palutena's n-air drives `Stick`, a bone Pikachu deliberately does NOT have -- her staff
    # and its hitboxes ride the vanilla `HaveL`, which `add-bones.sh` reparents under `HaveR`
    # and gives Stick's bind transform so the two frames coincide exactly. The track therefore
    # has to move from `Stick` to `HaveL`.
    #
    # That started as a separate step AFTER this one, and this trim ate the track first: a
    # `Stick` track names a bone the skeleton lacks, which is precisely what "foreign" means
    # here. The rename reported "0 animations repointed", the staff shipped with a
    # one-keyframe `HaveL` inherited from the source, and `check-hitboxes.sh` caught it as
    # `nair_palutena is 14.03 world units off`. Renaming inside the same filter, before the
    # select, is the only ordering that cannot lose it.
    #
    # The pairs come from `merge-weapons.sh`'s `realise` column, so they cannot drift from what
    # the skeleton and the merge actually did. The target's own track is dropped first: the
    # source has a one-keyframe `HaveL` of its own and two nodes with one name would leave the
    # engine reading whichever it saw last.
    jq -c --slurpfile b "$BONES_JSON" --slurpfile m "$MESHES_JSON" --slurpfile r "$REALISE_JSON" '
      ($b[0]) as $bones | ($m[0]) as $meshes | ($r[0]) as $realise
      | .groups |= map(
          if .group_type == "Transform" then
            .nodes |= ( [ .[] | . as $n
                          | select(($realise | any(.to_lc == ($n.name | ascii_downcase))) | not) ]
                        | [ .[] | . as $n
                            | (($realise | map(select(.from == ($n.name | ascii_downcase))) | first) // null) as $mm
                            | if $mm != null then ($n | .name = $mm.to) else $n end ] )
            | .nodes |= map(select((.name | ascii_downcase) as $n | ($bones | index($n)) != null))
          elif .group_type == "Visibility" or .group_type == "Material" then
            .nodes |= map(select((.name | ascii_downcase) as $n | ($meshes | index($n)) != null))
          else . end
        )' "$W/$stem.out.json" > "$W/$stem.trim.json"
    # REFUSE RATHER THAN SHIP A POSELESS ANIMATION. `Trans` and `Rot` are the root of the
    # chain -- Trans carries the root motion the game consumes to move the fighter, Rot the
    # scale compensation this script just applied -- so an animation that lost either of them
    # would play as a T-pose that does not travel, silently.
    kept=$(jq -r '[.groups[]|select(.group_type=="Transform")|.nodes[].name|ascii_downcase] | length' "$W/$stem.trim.json")
    roots=$(jq -r '[.groups[]|select(.group_type=="Transform")|.nodes[].name|ascii_downcase]|map(select(.=="trans" or .=="rot"))|length' "$W/$stem.trim.json")
    if [ "$kept" -lt 2 ] || [ "$roots" -lt 2 ]; then
        echo "  REFUSING $anim: trimming left $kept transform track(s) and $roots root(s) -- need Trans and Rot" >&2
        exit 1
    fi
    mv "$W/$stem.trim.json" "$W/$stem.out.json"
    trimmed=$((trimmed+1))

    "$T" "$W/$stem.out.json" "$DST/$anim" >/dev/null
    # Keep a dump of exactly what we ship, so tools/trace-all.sh measures the real thing.
    cp "$W/$stem.out.json" "$ROOT/work/fk/shipped_$stem.json"
    printf "  %-26s <- %-10s ratio %s (Rot %s -> %s)%s  %s\n" "$anim" "$fighter" "$ratio" \
           "$before" "$after" "$([ "$neut" != "-" ] && echo " +neut $neut")" "$label"
    scaled=$((scaled+1))
    # The stamp is recorded against the output SIZE AS JUST WRITTEN, not as it was before.
    NEWSTAMP["$anim"]="${SRCHASH[$s]:-nohash}|$ratio|$neut|$SELFSIG:$(stat -c%s "$DST/$anim")"
    n=$((n+1))
done <<< "$MOVES"

: > "$STAMPFILE"
for a in "${!NEWSTAMP[@]}"; do printf "%s	%s
" "$a" "${NEWSTAMP[$a]}" >> "$STAMPFILE"; done

echo "anim-dropin: $n shipped ($cached unchanged and skipped, $scaled scale-compensated, $trimmed trimmed of foreign tracks, $neutralised bones neutralised), $miss missing"
[ "$miss" -eq 0 ] || exit 1
