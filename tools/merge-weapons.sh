#!/usr/bin/env bash
# Merges every source fighter's weapon into Pikachu's body model as toggleable meshes.
#
# GENERALISES tools/merge-sword.sh, which did Chrom's Falchion and nothing else. The technique is
# unchanged -- LilyLavender Modding Workshop episode 39: give the mesh object a distinct name so
# `ModelModule::set_mesh_visibility` can target it, single-bind it to a bone grafted under
# Pikachu's own skeleton, and SHIP the textures, declared through new-dir-files.
#
# SHIP, not alias. `share-to-added` cannot read the vanilla arc -- it resolves only when a mod on
# the SD card provides a real file at the source path, and when it cannot, ARCropolis HANGS the
# fight load rather than erroring. Chrom's worked for weeks only because the test console carries
# an "Alucard over Chrom" mod replacing those exact three textures. The mod tree is gitignored, so
# shipping them redistributes nothing.
#
# WHY THIS WORKS AT ALL, AND WHY IT IS NOT `clone_weapon`. A real article needs the game's own
# article initialisation, and `smashline::clone_weapon` crashes inside
# `fighter_initialize_object_data` -- the Ptooie blocker in handbook/open-questions.md. A merged
# mesh needs none of that: it is part of Pikachu's model, it inherits his skeleton's animation, and
# a visibility flag is the whole of its lifecycle.
#
# WHAT IT CANNOT DO, stated up front because it decides which weapons are in this table.
#
# The graft carries ONE bone. That is exact for a RIGID weapon -- a sword is a stick, its bone is
# the handle, and Pikachu's hand animation moves it correctly for free. It is not exact for an
# article whose own skeleton ANIMATES: Incineroar's rope bends across RopeC/RopeL1..3/RopeR1..3,
# Game & Watch's parachute across Parachutea1/a2/b1/b2, Corrin's dragon hand across Hand1..4. Those
# shapes come from the article's own `model.nuanmb`, which nothing here plays, so a graft would
# show them as rigid geometry. They are deliberately absent from this table; see
# docs/the-weapons.md for what they would need instead.
#
# EVERY NUMBER BELOW IS THE GAME'S OWN. The bone parent, the material label, the mesh object name
# and the bounding sphere are all read out of the source fighter's own files -- the spheres in
# particular come from each article's `model.numshexb`, so culling behaves as the game intends
# rather than as a radius someone guessed. `merge-sword.sh` computed Chrom's by hand from the
# blade's vertices and got (0,0,4.5) r=7; that row keeps those values so this tool reproduces the
# shipped model byte-for-byte on the Chrom entry alone.
#
# Regenerate with:  bash tools/merge-weapons.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
T="$ROOT/vendor/ssbh_lib/ssbh_data_json.exe"
PIK="$ROOT/extracted/fighter/pikachu/model/body/c00"
OUT="$ROOT/pikachu-moveset/mod/fighter/pikachu/model/body/c00"
WORK="$ROOT/work/weapons"

# mesh_name | source model dir | source mesh object | material | bone | parent | cx,cy,cz | radius
#
# `source model dir` is relative to a fighter root and searched in BOTH extracted/ and
# work/arc-weapons/, because the four weapons past Chrom were pulled from the running game by
# `tools/arc-fetch.sh` and only Chrom's was in the original extraction.
TABLE="
chrom_sword|chrom/model/sword|fal_RShape|pasted__met_chrom_001|Sword|HaveR|0,0,4.5|7.0
cloud_sword|cloud/model/fusionsword|fusionsword1_VIS_O_OBJShape|def_cloud_004|fusionsword1|HaveR|0,6.26053,0|10.0666
mythra_sword|elight/model/esword|lightsword_VIS_O_OBJShape|alp_light_002|Sword1|HaveR|0,0,4.29851|6.57543
palutena_staff|palutena/model/body|staff_group_VIS_O_OBJShape|metal_palutena_002_a|Stick|HaveR|0.0275341,-0.65733,0.0205462|10.1638|Mouth
corrin_sword|kamui/model/body|Kamui_Weapon_VIS_O_OBJShape|def_sword_001|HaveR|HandR|-0.000500202,5.6193275,-0.0007674639|7.7049875
"

rm -rf "$WORK"; mkdir -p "$WORK" "$OUT"
cd "$WORK"

# Pikachu's five files, as JSON, once. Every weapon appends to these in turn.
for f in numshb numdlb numatb nusktb numshexb; do
    "$T" "$PIK/model.$f" "cur_$f.json" >/dev/null
done

# ==== AND `model.nusrcmdlb`, WHICH IS A SECOND MESH LIST THIS TOOL WAS NOT MAINTAINING ====
#
# FOUND BY READING A MOD THAT WORKS. "George over diddy kong (GAMEPLAY)" replaces Diddy's body
# model and mirrors correctly in both facings, and its `model/body/c00` ships `model.numshb`,
# `model.numdlb`, `model.numatb`, `model.numshexb`, `model.nuhlpb`, `model.xmb` AND
# `model.nusrcmdlb` -- while conspicuously shipping NO `model.nusktb` at all. This tool shipped
# five files and not that one.
#
# `nusrcmdlb` is a MODL file: same `HBSS`/`LDOM` magic as `.numdlb`, and `ssbh_data_json` parses it
# once the extension says so. Pikachu's has FORTY entries, one per mesh object with its material --
# `CameraHit_VIS_O_OBJShape` appears three times, once per subindex, exactly as in `numdlb`. Our
# `numdlb` has fifty-one.
#
# So the shipped model had a per-mesh table ELEVEN ENTRIES SHORT, with vanilla's copy being read
# against a mesh list we had grown. That is the same shape as the `entry_flags` defect fixed just
# before it, and per-mesh state read past the end of an array is exactly the kind of fault that
# behaves differently in one view than another.
#
# `ssbh_data_json` dispatches on the file EXTENSION, so the round trip goes through a `.numdlb`
# name in the work directory and is renamed on the way out.
cp "$PIK/model.nusrcmdlb" "src_as.numdlb"
"$T" "src_as.numdlb" "cur_nusrcmdlb.json" >/dev/null
base_srcmdl=$(jq '.entries | length' cur_nusrcmdlb.json)
cp "$PIK/model.adjb" "$OUT/model.adjb"

base_bones=$(jq '.bones | length' cur_nusktb.json)
# VANILLA PIKACHU'S BONE LIST, SNAPSHOTTED BEFORE ANY GRAFT. `cur_nusktb.json` accumulates grafts
# as the loop runs, so asking it "does Pikachu have this bone?" answers yes for a bone an earlier
# weapon appended. The bake decision below turns on exactly that question, so it needs the
# untouched list.
cp cur_nusktb.json vanilla_nusktb.json
base_meshes=$(jq '.objects | length' cur_numshb.json)

printf '%-16s %-9s %-7s %s\n' WEAPON BONE MESH SOURCE
while IFS='|' read -r name dir mesh material bone parent sphere radius realise; do
    case "${name:-}" in '' | \#*) continue ;; esac

    src=""
    for root in "$ROOT/extracted/fighter" "$ROOT/work/arc-weapons/fighter"; do
        [ -f "$root/$dir/c00/model.numshb" ] && src="$root/$dir/c00" && break
    done
    [ -n "$src" ] || { echo "MISSING source model for $name ($dir)" >&2; exit 1; }

    for f in numshb numdlb numatb nusktb numshexb; do
        "$T" "$src/model.$f" "src_$f.json" >/dev/null
    done

    # THE PARENT IS LOOKED UP BY NAME, NOT ASSUMED. Every weapon in this table hangs its bone off
    # `HaveR`, and so does Pikachu -- but a future row with a different anchor must not silently
    # graft onto bone 0.
    pidx=$(jq --arg p "$parent" '[.bones | to_entries[] | select(.value.name == $p)][0].key' cur_nusktb.json)
    [ "$pidx" != "null" ] || { echo "$parent not found in Pikachu's skeleton" >&2; exit 1; }

    # A MESH NAME COLLISION WOULD SILENTLY REBIND SOMETHING OF PIKACHU'S. Checked, not hoped for.
    # Bone collisions cannot happen at all now: every grafted bone is prefixed, see below.
    if jq -e --arg n "$name" '[.objects[] | .name] | index($n)' cur_numshb.json >/dev/null 2>&1; then
        echo "name collision: $name already exists in Pikachu's .objects" >&2; exit 1
    fi

    # EVERY GRAFTED BONE IS PREFIXED WITH THE WEAPON'S NAME, and that is not cosmetic.
    #
    # `tools/add-bones.sh` ALSO puts bones in this skeleton, from its own hand-kept list, for a
    # different reason: a ported ACMD anchors hitboxes to a source bone name, so Chrom's jab needs
    # a `Sword1` and Palutena's n-air needs a `Stick`. It takes `Sword1` from CHROM's skeleton.
    #
    # This tool used to graft a `Sword1` too -- MYTHRA's, the bone her sword mesh is bound to.
    # Same name, completely different bone:
    #
    #     chrom  Sword1  [[-0.18,-0.49,-0.85,0],[-0.98,0.11,0.15,0],[0.01,0.86,-0.51,0],...]
    #     elight Sword1  [[  0  ,  1   ,  0  ,0],[ -1 ,  0  ,  0  ,0],[  0 ,  0  ,  1  ,0],...]
    #
    # This tool runs FIRST and `add-bones.sh` is idempotent -- "a bone already present is left
    # alone" -- so the merge silently won and Chrom's jab hitboxes, documented as riding the blade
    # at (0,0,1.7) and (0,0,8.4), were aimed with Mythra's bind rotation. Two tools maintaining
    # overlapping lists from different source skeletons, which is the exact shape
    # `tools/check-lists.sh` exists to prevent.
    #
    # Prefixing removes the class of bug rather than this instance of it: a graft can no longer
    # shadow any bone Pikachu has or any bone another tool provides, whatever order they run in.
    # BARE NAMES, BECAUSE THE ANIMATION DRIVES BONES BY NAME.
    #
    # Prefixing every graft with the weapon name removed the add-bones collision and BROKE all
    # three weapons that need animating. The dropped-in `.nuanmb` animates bones by name:
    #
    #     c00attack11.nuanmb   (chrom jab)     HaveR Sword1 Sword2
    #     c05attackairn.nuanmb (palutena nair) HaveR Stick
    #     e00catch*.nuanmb     (palutena grab) HaveR Stick
    #     c00attackdash.nuanmb (mythra dash)   HaveR only
    #
    # A graft called `palutena_staff_Stick` has no track, so the staff sat at bind pose and did not
    # sweep; `chrom_sword_Sword` likewise, which is why the Falchion was in the wrong hand jabbing
    # left. Playtest, exactly: "Chrom sword doesn't appear in pikachu's correct hand when jabbing to
    # the left. Palutena staff doesn't spin correctly when using nair."
    #
    # THE COLLISION IS NOT A PROBLEM AFTER ALL, which is what made prefixing look necessary.
    # `add-bones.sh` wants `Sword1` for chrom's jab and this tool wants one for mythra's sword --
    # different bones, same name. But an ANIMATION supplies the transform per move, so one `Sword1`
    # serves both: chrom's jab animates it, and mythra's dash has no track and therefore wants a
    # bind pose, which is hers. The bind only decides where an UNANIMATED bone sits.
    #
    # Two skips, both load-bearing. Already in the skeleton: it is the same bone by name, leave it.
    # Not in the SOURCE skeleton: chrom's table bone is `Sword1` and his sword MODEL only carries
    # `HaveR` and `Sword` -- `Sword1` lives in his BODY skeleton, which add-bones.sh supplies from
    # work/fk/chrom_skel.json. The mesh binds to the name either way.
    # THE GRAFT SET IS THE TABLE'S BONE PLUS EVERY BONE THE MESH'S INFLUENCES NAME, walked in
    # SOURCE SKELETON ORDER so a parent is always appended before its children. Bare names, because
    # the dropped-in animation drives bones BY NAME -- see the note below the table.
    #
    # A SKINNED GRAFT IS FINE, and believing otherwise cost Mythra's sword. Her
    # `lightsword_VIS_O_OBJShape` is weighted 672 vertices to Sword1 and 32 each to Sword_lightC
    # and Sword_lightD. I forced every graft rigid on the theory that a skinned mesh needs a
    # runtime matrix palette entry for bones that are not in the transform chain -- reasoned from
    # rebase.rs's proof that grafted bones are not in that chain, and motivated by a load hang that
    # turned out to be `share-to-added` instead. Forcing it rigid reinterprets model-space vertices
    # as bone-local and collapses the blade: "Mythra sword doesn't appear, but if I go
    # frame-by-frame I can see very small slivers where the middle of the sword would be".
    #
    # So the weights are left alone and the bones they name are grafted. Two skips: already in the
    # skeleton (same bone by name, leave it), and absent from the SOURCE skeleton -- chrom's table
    # bone is `Sword1` and his sword MODEL carries only `HaveR` and `Sword`, because `Sword1` lives
    # in his BODY skeleton and `add-bones.sh` supplies it.
    # THE BONE SET IS UNIONED ACROSS EVERY SUBINDEX, not read off the first one. Mythra's four
    # subindices name four DIFFERENT bone sets -- sub 0 has Sword1/Sword_lightC/Sword_lightD, and
    # sub 2 (the 6,876-vertex bulk of the blade) needs nine: Sword1 plus Sword_lightA, B, C, D,
    # E1, E2, F1, F2. Taking sub 0's three left six of the nine ungrafted, so even the bones the
    # grafted geometry was weighted to were missing.
    # NO BONE IS GRAFTED ANY MORE, AND THAT IS THE POINT.
    #
    # Every weapon now renders and hits on a bone Pikachu already owns: four are baked onto
    # `HaveR`, and Palutena's staff is realised on the vanilla `HaveL`. Nothing references
    # `Sword`, `Sword1`, `fusionsword1`, `Stick` or the nine `Sword_light*` bones, so grafting them
    # adds twelve bones that only create risk -- the playtest is consistent that appended bones
    # misbehave, and `tools/reach/src/bin/rebase.rs` carries the evidence.
    #
    # IT ALSO REMOVES A COLLISION THIS FILE'S OWN HEADER DOCUMENTS. `add-bones.sh` wants CHROM's
    # `Sword1` for his jab hitboxes; this tool grafted MYTHRA's, which is a different bone with the
    # same name, and it ran first while add-bones is idempotent -- so Mythra's won:
    #
    #     chrom  Sword1  [-0.185,-0.494,-0.85] [-0.983,0.106,0.152] [0.014,0.863,-0.505]
    #     elight Sword1  [0,1,0]               [-1,0,0]             [0,0,1]
    #     SHIPPED        [0,1,0]               [-1,0,0]             [0,0,1]   <- Mythra's
    #
    # Harmless while the bone is animated, because the track replaces the bind transform outright,
    # which is why `check-hitboxes.sh` still measured chrom's jab at 0.00. A latent trap all the
    # same, and grafting nothing closes it rather than ordering two tools carefully.
    # ==== BIND TO `HaveR` AND BAKE THE BONE OUT, WHENEVER THE BONE IS NOT ANIMATED ====
    #
    # THE EVIDENCE THIS EXISTS FOR. Playtest, with all five swords finally visible: "chrom's sword
    # still doesn't appear correctly when pikachu jabs to the left", "mythra's sword doesn't
    # animate correctly when ftilting/dash attacking to the left". Corrin's is the ONLY one bound
    # to a bone Pikachu already owns -- `HaveR` -- and Corrin's is the only one nobody reported.
    # Four grafted-bone weapons misbehave; the one vanilla-bone weapon does not.
    #
    # `telemetry::mirror_probe` says the same thing from the other side, and says it in a way that
    # cannot be explained away: `haver` reads IDENTICALLY to three decimals across all 24 frames of
    # both facings -- so the pose is provably the same -- while `sword` differs between facings in
    # Y, which no facing can change, and `sword` and `sword1` report byte-identical positions even
    # though Chrom's jab animates `Sword1`, does NOT animate `Sword`, and `Sword`'s local
    # translation is (0,0,0) so it must sit exactly AT `haver`.
    #
    # WHY THIS IS SAFE EITHER WAY, which is the point. If grafted bones are fine at runtime this
    # rewrite is a mathematical no-op: a rigid mesh on bone B under `HaveR` renders at
    # `HaveR_world . M_B . v`, and baking `M_B` into the vertices and binding to `HaveR` renders at
    # `HaveR_world . v'` with `v' = M_B . v`. Identical. If grafted bones are NOT fine, it removes
    # them from the render path entirely and puts these meshes in the one configuration the
    # playtest proves works. So it cannot make anything worse, and it needs no boot to justify.
    #
    # THE RULE IS DERIVED, NOT DECLARED. Bake when NO animation this mod ships for that fighter
    # names the bone -- measured out of `work/fk/`, which `anim-dropin.sh` fills:
    #
    #     chrom    attack_11        Sword        no track   -> bake
    #     cloud    attack_air_b     fusionsword1 no track   -> bake
    #     elight   attack_dash      Sword1       no track   -> bake
    #     elight   attack_s3_*      Sword1       no track   -> bake
    #     palutena attack_air_n     Stick        460 KEYS   -> KEEP THE GRAFT
    #
    # Palutena's staff is the one weapon whose bone its own move actually animates, and baking it
    # would freeze the sweep that IS the move. So she keeps the graft and stays the open case.
    # A future move that starts animating one of the other bones flips its row automatically,
    # which is why this is computed instead of a column in the table.
    # `work/fk/` HOLDS SKELETONS AS WELL AS ANIMATIONS, and globbing both made this answer wrong.
    # `<fighter>_skel.json` is the source rig, so it contains a NODE for every bone by definition
    # -- `elight_skel.json` has `Sword1` -- and matching it set `animated` for every weapon. Mythra
    # was skipped when she should bake, and Palutena was kept for the wrong reason (her `Stick`
    # really is animated, but the skeleton would have triggered it regardless). Only the animation
    # dumps count.
    fighter="${dir%%/*}"
    animated=0
    for a in "$ROOT"/work/fk/${fighter}_*.json; do
        [ -f "$a" ] || continue
        case "$a" in *_skel.json) continue ;; esac
        n=$(jq -r --arg b "$bone" '[.. | objects | select(.name? == $b)] | length' "$a" 2>/dev/null | tr -d '\r')
        if [ "${n:-0}" != "0" ]; then animated=1; break; fi
    done
    # ==== THE BAKE IS OFF. THE PLAYTEST RETIRED IT, TWICE. ====
    #
    # The idea was that binding every mesh to `HaveR` would be either a no-op (if grafted bones
    # work) or a fix (if they do not), so it could not make anything worse. That reasoning was
    # wrong, and the evidence is a straight read of what each state produced:
    #
    #   BEFORE the bake   every mesh on its own grafted bone. All five swords APPEAR. Cloud and
    #                     Corrin correct in both directions. Chrom and Mythra wrong facing left.
    #                     Palutena wrong both ways.
    #   WITH the bake     Cloud's back air goes visually wrong. Chrom and Mythra still wrong left.
    #   WITH the bake computed to the model ROOT instead of the bone
    #                     Chrom and both Mythra moves wrong in BOTH directions, Corrin wrong left,
    #                     Palutena wrong right. "Almost every sword move broken."
    #
    # Cloud is the control that settles it: he was CORRECT on a grafted bone and went wrong when
    # baked onto `HaveR`, with a bake matrix that is provably the identity for him -- so the only
    # thing that changed was which bone he binds to. A grafted bone is therefore NOT the thing that
    # breaks a mesh, and moving meshes off them cost two working weapons.
    #
    # So every mesh binds to the bone its source model binds it to, which is where this started and
    # the only configuration in which anything was ever fully correct. `realise` still applies --
    # Palutena's staff goes on the vanilla `HaveL`, which is the one change that made a broken
    # weapon work in one direction.
    #
    # The derivation above is kept because it is a real measurement of which bones the ported moves
    # animate, and `check-weapons.sh` reads it to decide which bind to expect.
    #
    # ==== EXCEPT FOR A SKINNED SOURCE MESH, WHERE NOT BAKING IS PROVABLY WRONG. ====
    #
    # The retirement above is sound for a RIGID source mesh, and it is worth writing down exactly
    # why, because "the graft's frame might be wrong" was a live suspicion for weeks and it is now
    # closed by algebra rather than by another playtest.
    #
    # A rigid weapon mesh single-binds to a bone B one level under the weapon MODEL's own `HaveR`,
    # which is the frame the engine aligns with the fighter's hand. Natively, then
    #
    #     v_world = v . world_w(B) . world_w(HaveR)^-1 . world_pika(HaveR)
    #
    # and since `world_w(B) = local_w(B) . world_w(HaveR)`, the middle terms cancel and this is
    # EXACTLY `v . local_w(B) . world_pika(HaveR)` -- which is what grafting B under Pikachu's
    # `HaveR` with its own local transform and single-binding the mesh to it computes. Chrom and
    # Cloud are both of that shape (chrom/model/sword is Root->HaveR->Sword,
    # cloud/model/fusionsword is Root->HaveR->fusionsword1), so their grafts are algebraically
    # identical to the native attachment. The bake is a true no-op for them, and the playtest
    # showed it made Cloud worse anyway, so it stays off.
    #
    # A SKINNED mesh does not reduce. Skinning computes
    #
    #     v_world = v . Wbind_pika(B)^-1 . W_pika(B)
    #
    # which substitutes PIKACHU's bind pose for the weapon model's hand frame. Those are different
    # matrices, and what is left is the bone's DEVIATION from Pikachu's bind pose rather than its
    # absolute pose -- so the mesh renders near the model origin and drifts as the arm moves
    # instead of sitting in the hand. Measured on Mythra's blade tip: the native formula puts it
    # at (3.347, 2.014, 10.409), which is 10.86 units from Pikachu's `HaveR` and so exactly the
    # blade's own length; the shipped skinning puts it at (-0.019, 0.000, 10.874), a 3.951-unit
    # error at bind pose and a growing one in motion.
    #
    # WHY THE BAKE IS EXACT HERE AND NOT MERELY BETTER. Two facts, both checked rather than
    # assumed:
    #   - Every one of Mythra's skin weights is exactly 1.0 -- 4 subindices over 9 influence bones,
    #     `Sword1` and `Sword_lightA..F2`. There is no blending to lose.
    #   - NO shipped Mythra animation drives `Sword1` or any light bone. `c00attackdash` and
    #     `c01attacks3s` carry 43 Transform tracks each and not one names them, so the whole sword
    #     is rigid relative to `HaveR` for the entire move.
    # That is also why ONE bake matrix is valid for vertices weighted to nine different bones: at
    # bind pose `inverse(world_esword(HaveR))` maps all of them into the hand frame correctly, and
    # bind pose is the only pose they are ever in.
    #
    # AND THE EARLIER PLAYTEST DID NOT TEST THIS. "With the bake, Chrom and Mythra still wrong
    # left" was measured when the bake was GLOBAL and when Chrom's jab was shipping with 51
    # Transform tracks naming bones Pikachu does not have -- see `tools/check-anim-tracks.sh`.
    # Both weapons were confounded. This is the narrow case instead: bake the one mesh whose
    # construction is provably wrong, and leave the four that provably match native alone.
    # ==== A MESH MUST BIND TO A BONE VANILLA PIKACHU ALREADY HAS. BAKE IF IT DOES NOT. ====
    #
    # THE A/B THAT ESTABLISHED THIS held the geometry exactly fixed and changed only the bone's
    # identity, which is why it is worth more than the ten theories before it.
    #
    # Palutena's staff was shipped two ways. First on the vanilla `HaveL` (index 17) reparented
    # under `HaveR` and given `Stick`'s bind transform; then on a `Stick` (index 88) grafted under
    # `HaveR` with that same bind transform. Same parent, same local transform, same vertices --
    # geometrically the SAME PLACE by construction. The only difference is that one bone ships with
    # Pikachu and the other is appended.
    #
    #     on vanilla `HaveL`   correct facing right, wrong facing left
    #     on appended `Stick`  wrong in BOTH directions
    #
    # And the playtest settled what the failure is NOT: "sweeps, but in the wrong place". The
    # appended bone IS driven by the animation -- her 65-keyframe `Stick` track moves it -- so the
    # bone is alive and it is the MESH that renders somewhere else. That also kills the reason this
    # mechanism was originally built ("a grafted bone is not driven at runtime") while confirming
    # the mechanism itself.
    #
    # IT EXPLAINS THE WHOLE TABLE, which no earlier theory did:
    #
    #     corrin    `HaveR`         index 22, VANILLA    correct both ways
    #     cloud     `fusionsword1`  index 78, appended   "correct" -- but his Buster Sword is held
    #                                                    in both hands on the centreline, where a
    #                                                    mirror across the midline is nearly the
    #                                                    identity, so the fault is invisible
    #     chrom     `Sword`         index 77, appended   "at the other hand's position" facing left
    #     palutena  `Stick`         index 88, appended   displaced, both directions
    #
    # And George's mod, the reference that works on both sides, ships NO `model.nusktb` at all. He
    # adds no bones and single-binds to the vanilla `HaveR` and `HaveL`.
    #
    # SO: bake whenever the mesh would otherwise bind to a bone Pikachu does not ship. The bake
    # folds the bone's own transform into the vertices and single-binds to `HaveR`, which Corrin
    # proves is a correct mesh parent because that is exactly what his own body model does.
    #
    # THE TWO CONDITIONS, and each is load-bearing:
    #   - the bind bone is not in VANILLA Pikachu's skeleton, or the source mesh is SKINNED (which
    #     substitutes Pikachu's bind pose for the weapon model's hand frame -- see below);
    #   - and no shipped animation drives that bone, because baking freezes it. Palutena's `Stick`
    #     carries 65 keyframes, so she CANNOT be baked and needs the `realise` route instead.
    #
    # KNOWN RISK, recorded rather than hidden. An earlier round baked every mesh onto `HaveR` and
    # the playtest said Cloud's back-air went wrong -- on a bake matrix that is the identity for
    # him, so his geometry provably did not change. That result is unexplained and it was measured
    # while Chrom's jab was shipping 51 Transform tracks for bones that do not exist, so the round
    # was confounded. It is still the one piece of evidence against this, and Cloud is the weapon
    # to check first.
    nin0=$(jq -r --arg m "$mesh" '[.objects[]|select(.name==$m)][0].bone_influences|length' \
             src_numshb.json | tr -d '\r')
    vanilla_bone=$(jq -r --arg b "$bone" '[.bones[]|select(.name==$b)]|length' \
             vanilla_nusktb.json | tr -d '\r')
    if [ "$animated" = "0" ] && { [ "${nin0:-0}" != "0" ] || [ "${vanilla_bone:-0}" = "0" ]; }; then
        bake=1
        why="binds to \`$bone\`, which vanilla Pikachu does not have"
        [ "${nin0:-0}" != "0" ] && why="source mesh is SKINNED ($nin0 influence bones)"
        echo "  $name: $why and no shipped animation drives $bone -- baking into HaveR's frame"
    else
        bake=0
        if [ "${vanilla_bone:-0}" = "0" ]; then
            echo "  $name: binds to appended \`$bone\` and CANNOT be baked -- $bone is animated." \
                 "It needs a vanilla bone realised into its frame, or the mesh will render displaced."
        fi
    fi

    # THE BAKE DOES NOT REMOVE THE BONES, AND MAKING IT DO SO BROKE THE BUILD.
    #
    # A baked mesh does not NEED its source bones -- its vertices are in `HaveR`'s frame and it
    # single-binds there -- but "does not need" is not "must remove", and removing them has a cost
    # that took a failed build to find. Baking Mythra dropped `Sword1` and her eight light bones,
    # nine bones, and `Sword1` is the bone CHROM's jab animates. `anim-dropin.sh` then correctly
    # trimmed his `Sword1` track as foreign, and `rebase.rs` -- which reads that track to move his
    # hitboxes onto `haver`, and prefers the animation over the bind pose precisely because the
    # animation is the truth -- lost its input and panicked with
    # "Sword1 not in work/bones/out.json".
    #
    # Whose `Sword1` it is never mattered for Chrom: `rebase.rs` takes the TRACK, not the bind
    # transform, so Mythra's esword bind pose was never read on his behalf. What matters is only
    # that a bone by that name exists, so the trim keeps his track.
    #
    # So the bake now governs the MESH alone. The bones are grafted either way, which also keeps
    # the skeleton at the 88 bones every other configuration was measured against -- and bone count
    # and ordering have broken working weapons twice in this project, so holding them still while
    # changing one mesh is the point.
    if [ -n "${realise:-}" ]; then
        cp cur_nusktb.json next_nusktb.json
    else
    jq --argjson anchor "$pidx" --arg m "$mesh" --arg b "$bone" \
       --slurpfile sm src_numshb.json --slurpfile sk src_nusktb.json '
      . as $doc
      | ( ( [ $sm[0].objects[] | select(.name == $m) | (.bone_influences // [])[] | .bone_name ] )
          + [$b] )
      | unique as $want
      | [ $sk[0].bones[] | select(.name | IN($want[])) ] as $graft
      | reduce $graft[] as $g ($doc;
          if ([.bones[].name] | index($g.name)) != null then .
          else
            ( if ($g.parent_index == null or $g.parent_index < 0) then ""
              else ($sk[0].bones[$g.parent_index].name // "") end ) as $pname
            | ( [.bones[].name] | index($pname) ) as $found
            | .bones += [ $g | .parent_index = (if $found == null then $anchor else $found end) ]
          end
        )
    ' cur_nusktb.json > next_nusktb.json
    fi

    # EVERY SUBINDEX IS GRAFTED, AND TAKING ONLY THE FIRST WAS THE BUG.
    #
    # This block used to say "SUBINDEX 0 ONLY ... the game's own LOD and variant slots. Taking the
    # first is what `set_mesh_visibility` can address by name and is the highest-detail one."
    # Both halves of that are false, and the playtest reports were the two weapons it broke.
    #
    # THEY ARE NOT LOD SLOTS. LOD is a separate mesh NAME with a `_low` suffix -- the same files
    # carry `lightsword_lowShape`, `staff_group_lowShape`, `body_lowShape`. Several subindices
    # under ONE name are MATERIAL SPLITS: one draw call per material, each with its own entry in
    # `model.numdlb`. Mythra's four are `alp_light_002`, `def_light_004`, `def_light_005`,
    # `def_light_006`.
    #
    # AND SUBINDEX 0 IS NOT THE HIGHEST DETAIL. Measured, per subindex, in vertex indices:
    #
    #     mythra  lightsword_VIS_O_OBJShape   1320 /  936 / 6876 /  912    sub 0 is 13%
    #     corrin  Kamui_Weapon_VIS_O_OBJShape 11964 /  168 / 2976         sub 0 is 79%
    #     palutena staff_group_VIS_O_OBJShape  2442 /  684                sub 0 is 78%
    #     chrom   fal_RShape                  single subindex
    #     cloud   fusionsword1_VIS_O_OBJShape  single subindex
    #
    # That table is the whole diagnosis. The two weapons that worked in game are exactly the two
    # with one subindex; the two that were reported broken are exactly the two with more, and
    # 13% of a blade is "Mythra sword doesn't appear, but if I go frame-by-frame I can see very
    # small slivers where the middle of the sword would be" -- the playtest was describing sub 0.
    #
    # THE MATERIALS COME FROM THE SOURCE `numdlb`, NOT FROM THE TABLE. The table's `material`
    # column can only name one, which is how three of Mythra's four went missing without a word.
    # It is kept as an ASSERTION against subindex 0 below, so a stale table fails loudly instead
    # of being quietly overridden.
    #
    # `parent_bone_name` is overridden ONLY for a rigid mesh, and it has to be: chrom's source
    # mesh single-binds to `Sword`, which no animation drives, while the table names `Sword1`,
    # which his jab animates. A SKINNED mesh keeps its own weights and an empty parent, which is
    # how the game authored it -- and forcing one rigid reinterprets model-space vertices as
    # bone-local and collapses the blade.

    nsub=$(jq --arg m "$mesh" '[.objects[] | select(.name == $m)] | length' src_numshb.json)
    sub0mat=$(jq -r --arg m "$mesh" '
        [.entries[] | select(.mesh_object_name == $m)] | sort_by(.mesh_object_subindex)
        | .[0].material_label // ""' src_numdlb.json)
    if [ "$sub0mat" != "$material" ]; then
        echo "TABLE STALE for $name: subindex 0 of $mesh uses material '$sub0mat', table says '$material'" >&2
        exit 1
    fi

    # The matrix the bake applies, in the same row-vector convention ssbh_data stores: a bone's
    # `transform` has its LOCAL AXES AS ROWS with the translation in row 3, so a point goes
    # `p = v . M`. (rebase.rs's `bind` proves that convention against a real animated quaternion
    # rather than assuming it.)
    #
    #   RIGID  mesh: its vertices are already in bone B's local frame, and B's parent IS `HaveR`,
    #          so B's own local transform is exactly the map into `HaveR`'s frame.
    #   SKINNED mesh: its vertices are in the source rig's MODEL space, so the map is the INVERSE
    #          of `HaveR`'s world transform in that rig. This is the step the earlier attempt
    #          missed -- "stripping the weights and single-binding" collapsed Mythra's blade
    #          because it rebound model-space vertices to `Sword1` and let the chain transform
    #          them a second time, rather than converting them into the target bone's frame first.
    BAKEM=""
    if [ "$bake" = "1" ]; then
        nin0=$(jq -r --arg m "$mesh" '[.objects[]|select(.name==$m)][0].bone_influences|length' src_numshb.json | tr -d '\r')
        BAKEM=$(jq -c --arg b "$bone" --arg skinned "$nin0" '
          def mul(A;B): [ range(0;4) as $i | [ range(0;4) as $j |
              reduce range(0;4) as $k (0; . + A[$i][$k]*B[$k][$j]) ] ];
          def world($bones; $i):
              if $i == null or $i < 0
              then [[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]]
              else mul($bones[$i].transform; world($bones; $bones[$i].parent_index)) end;
          def invrt(M):
              ( [ range(0;3) as $i | [ range(0;3) as $j | M[$j][$i] ] ] ) as $RT
              | ( [ range(0;3) as $j | - (reduce range(0;3) as $k (0; . + M[3][$k]*$RT[$k][$j])) ] ) as $t
              | [ [$RT[0][0],$RT[0][1],$RT[0][2],0],
                  [$RT[1][0],$RT[1][1],$RT[1][2],0],
                  [$RT[2][0],$RT[2][1],$RT[2][2],0],
                  [$t[0],$t[1],$t[2],1] ];
          .bones as $B
          | if ($skinned|tonumber) == 0
            then ( [ $B[] | select(.name == $b) ][0].transform )
            else ( [ $B | to_entries[] | select(.value.name == "HaveR") ][0].key ) as $h
                 | invrt(world($B; $h))
            end
        ' src_nusktb.json | tr -d '\r')
        [ -n "$BAKEM" ] && [ "$BAKEM" != "null" ] || { echo "$name: cannot compute bake matrix" >&2; exit 1; }
    fi

    # `realise` IS WHERE THE BONE ACTUALLY LIVES ON PIKACHU, AND IT EXISTS BECAUSE GRAFTED BONES
    # ARE NOT RELIABLY IN THE RUNTIME TRANSFORM CHAIN.
    #
    # Proven by `telemetry::mirror_probe` with two controls in the log. `handr`, a vanilla bone,
    # reads IDENTICALLY in both facings on every frame -- so the probe's frame math is right and
    # the pose is the same. Against that, `sword`, `sword1` and `stick` differ between facings in
    # all three components including Y, which no facing can change, and `sword`/`sword1`/
    # `fusionsword1` report byte-identical values to each other. And `sword2` -- Chrom's blade tip,
    # which his animation drives and this mod never grafts -- comes back as the exact NEGATION of
    # `haver_from_top`, which is algebraically the fighter origin. So a missing bone returns the
    # origin rather than failing, and appended bones read like memory past the end of an array
    # sized for Pikachu's vanilla bone count.
    #
    # Four of the five weapons no longer care, because the bake above puts them on `HaveR`.
    # Palutena's staff cannot be baked -- her hand is nearly static from frame 6 while the staff
    # traces a full circle, so the sweep IS `Stick`'s own track -- so instead `add-bones.sh`
    # REPURPOSES the vanilla `HaveL`: reparents it under `HaveR` and gives it Stick's bind
    # transform, and the shipped animation's `Stick` track is renamed onto it. `HaveL` then
    # occupies exactly Stick's frame, on a bone the runtime owns.
    #
    # That makes the rename LOSSLESS, which is why `tools/bone-remap.sed` can do the hitbox side
    # with no transform at all -- the thing it was emptied for being unable to do. It was lossy
    # before because it mapped to `haver` and discarded Stick's [0,-2.2,0]; mapping to a bone that
    # HAS that offset discards nothing.
    #
    # `HaveL` is safe to repurpose: nothing in the shipped model is weighted to it, it is a leaf
    # with no children, and its only role is as the left-hand item attachment. The cost is that an
    # item held in the left hand would follow the right, which this moveset never does.
    bindbone="${realise:-$bone}"
    jq --arg n "$name" --arg m "$mesh" --arg pb "$bindbone" --argjson bake "$bake" \
       --argjson M "${BAKEM:-null}" --slurpfile s src_numshb.json '
      def pt($v; $M): [ range(0;3) as $j
          | ( reduce range(0;3) as $k (0; . + $v[$k]*$M[$k][$j]) ) + $M[3][$j] ];
      def dir($v; $M): [ range(0;3) as $j
          | reduce range(0;3) as $k (0; . + $v[$k]*$M[$k][$j]) ];
      def bakeobj($o; $M):
          $o
          | .positions = [ .positions[] | .data.Vector3 = [ .data.Vector3[] | pt(.; $M) ] ]
          | .normals   = [ .normals[]?  | .data.Vector4 = [ .data.Vector4[]
                             | dir(.; $M) + [.[3]] ] ]
          | .tangents  = [ .tangents[]? | .data.Vector4 = [ .data.Vector4[]
                             | dir(.; $M) + [.[3]] ] ]
          | .binormals = [ .binormals[]? | .data.Vector3 = [ .data.Vector3[]? | dir(.; $M) ] ]
          # Bound to HaveR now, and NOT skinned: the weights described a chain that no longer
          # applies once the vertices sit in the frame of HaveR itself.
          | .parent_bone_name = "HaveR"
          | .bone_influences = [];
      ( [ $s[0].objects[] | select(.name == $m) ] | sort_by(.subindex) ) as $objs
      | .objects += [ $objs[]
          | if $bake == 1 then bakeobj(.; $M)
            else ( .parent_bone_name = ( if ((.bone_influences // []) | length) == 0 then $pb
                                         else (.parent_bone_name // "") end ) )
            end
          | .name = $n ]
    ' cur_numshb.json > next_numshb.json

    # One modl entry per subindex, each keeping the source's own subindex AND its own material.
    jq --arg n "$name" --arg m "$mesh" --slurpfile d src_numdlb.json '
      ( [ $d[0].entries[] | select(.mesh_object_name == $m) ]
        | sort_by(.mesh_object_subindex) ) as $es
      | .entries += [ $es[] | .mesh_object_name = $n ]
    ' cur_numdlb.json > next_numdlb.json

    # THE SAME ENTRIES INTO `nusrcmdlb`, because it is the same kind of list and the game reads
    # both. Identical filter, different file -- if these two ever disagree the shipped model has a
    # mesh the source list does not know about, which is the state that shipped for weeks.
    jq --arg n "$name" --arg m "$mesh" --slurpfile d src_numdlb.json '
      ( [ $d[0].entries[] | select(.mesh_object_name == $m) ]
        | sort_by(.mesh_object_subindex) ) as $es
      | .entries += [ $es[] | .mesh_object_name = $n ]
    ' cur_nusrcmdlb.json > next_nusrcmdlb.json

    # Every material any subindex uses, appended only if Pikachu does not already carry that label
    # -- two weapons from one fighter would otherwise duplicate it and the game rejects duplicate
    # labels. `reduce` because there are now several per weapon and each has to see the result of
    # the last.
    jq --arg m "$mesh" --slurpfile d src_numdlb.json --slurpfile a src_numatb.json '
      ( [ $d[0].entries[] | select(.mesh_object_name == $m) | .material_label ] | unique ) as $mats
      | reduce $mats[] as $lbl (.;
          if ([.entries[] | .material_label] | index($lbl)) then .
          else .entries += [ ( $a[0].entries[] | select(.material_label == $lbl) ) ] end
        )
    ' cur_numatb.json > next_numatb.json

    # ==== THE BOUNDING SPHERE IS COMPUTED FROM THE SHIPPED GEOMETRY, NOT COPIED FROM A TABLE ====
    #
    # THIS IS WHAT "FACING LEFT" WAS. `numshexb`'s bounding sphere is what the renderer CULLS
    # against, so a sphere in the wrong place makes the mesh vanish or mis-sort depending on where
    # it happens to be relative to the view -- which is exactly a bug that appears swinging one way
    # and not the other. Playtest: "chrom's sword still doesn't appear correctly when pikachu jabs
    # to the left", "mythra's sword doesn't animate correctly when ftilting/dash attacking to the
    # left", and both of those survived binding the meshes to `HaveR`.
    #
    # The `sphere` and `radius` columns were hand-entered from each source model, in the SOURCE
    # bone's frame -- and the mesh does not end up in that frame. Measured against the actual
    # shipped vertices:
    #
    #     chrom_sword     declared (0,0,4.5)  r7.00   actual (0.03,3.83,-2.11) r>=5.58   WRONG
    #     mythra_sword    declared (0,0,4.3)  r6.58   actual (0,4.3,0)         r>=6.58   WRONG, y/z swapped
    #     cloud_sword     declared (0,6.26,0) r10.07  actual (0,6.26,0)        r>=10.06  ok
    #     corrin_sword    declared (0,5.62,0) r7.70   actual (0,5.60,0)        r>=7.68   ok
    #     palutena_staff  declared (0.03,-0.66,0.02)  actual the same                    ok
    #
    # The two that were wrong are exactly the two the playtest reported; the two that were right
    # were never mentioned. So it is computed now, over EVERY subindex of the mesh -- the group is
    # keyed by mesh NAME, so it has to enclose all of them -- and the radius is the true enclosing
    # radius, the largest distance from the centre to any vertex, not half the longest bbox extent.
    #
    # The table columns are left in place because `check-weapons.sh` uses them to assert that the
    # computed sphere still encloses the geometry, and because they record what the source model
    # declared. They are no longer the source of truth.
    SPHERE=$(jq -c --arg n "$name" '
      [ .objects[] | select(.name == $n) | .positions[0].data.Vector3[] ] as $V
      | ( [ $V[][0] ] ) as $X | ( [ $V[][1] ] ) as $Y | ( [ $V[][2] ] ) as $Z
      | { x: (($X|min) + ($X|max)) / 2,
          y: (($Y|min) + ($Y|max)) / 2,
          z: (($Z|min) + ($Z|max)) / 2 } as $c
      | { center: $c,
          radius: ( [ $V[] | ((.[0]-$c.x)*(.[0]-$c.x) + (.[1]-$c.y)*(.[1]-$c.y) + (.[2]-$c.z)*(.[2]-$c.z)) ] | max | sqrt ) }
    ' next_numshb.json | tr -d '\r')
    [ -n "$SPHERE" ] && [ "$SPHERE" != "null" ] || { echo "$name: cannot compute bounding sphere" >&2; exit 1; }

    # ONE `entry_flags` ELEMENT PER SUBINDEX, which is the game's own convention and was the third
    # thing the subindex change broke.
    #
    # `entry_flags` is indexed BY SUBINDEX, so a group with one element and four subindices makes
    # the game read `draw_model` and `cast_shadow` out of bounds for subindices 1..3 -- per-mesh
    # render flags taken from whatever sits after the array. Flags read from adjacent memory are
    # exactly the kind of fault that shows up in one view and not another, which is what "still
    # broken facing left" looked like after the bone and the bounding sphere had both been fixed.
    #
    # Verified against vanilla rather than assumed: all 40 of Pikachu's own groups have exactly as
    # many `entry_flags` as the mesh has subindices, and `CameraHit` is the proof that it is not
    # always one -- three subindices, three flags. Ours had one apiece while `mythra_sword` has
    # four subindices, `corrin_sword` three and `palutena_staff` two.
    jq --arg n "$name" --argjson bs "$SPHERE" --argjson nsub "$nsub" '
      .mesh_object_groups += [{
        bounding_sphere: $bs,
        mesh_object_full_name: $n,
        mesh_object_name: $n,
        entry_flags: [ range(0; $nsub) | { draw_model: true, cast_shadow: true } ]
      }]
    ' cur_numshexb.json > next_numshexb.json

    for f in numshb numdlb numatb nusktb numshexb nusrcmdlb; do mv "next_$f.json" "cur_$f.json"; done
    printf '%-16s %-9s %-7s %s\n' "$name" "$bone" "ok" "${src#$ROOT/}"
done <<< "$TABLE"

for f in numshb numdlb numatb nusktb numshexb; do
    "$T" "cur_$f.json" "$OUT/model.$f" >/dev/null
done
# Out through a `.numdlb` name, because ssbh_data_json chooses its writer by extension too.
"$T" "cur_nusrcmdlb.json" "out_as.numdlb" >/dev/null
mv -f "out_as.numdlb" "$OUT/model.nusrcmdlb"
# THE TWO MESH LISTS MUST AGREE, and this is the assertion rather than the hope. A `nusrcmdlb`
# shorter than `numdlb` is the state that shipped for weeks.
n_dlb=$(jq '.entries | length' cur_numdlb.json)
n_src=$(jq '.entries | length' cur_nusrcmdlb.json)
if [ "$n_dlb" != "$n_src" ]; then
    echo "REFUSING: numdlb has $n_dlb entries and nusrcmdlb has $n_src -- they describe the same meshes" >&2
    exit 1
fi

echo
echo "skeleton: $(jq '.bones | length' cur_nusktb.json) bones (Pikachu alone has $base_bones)"
echo "mesh:     $(jq '.objects | length' cur_numshb.json) objects (Pikachu alone has $base_meshes)"
echo "matl:     $(jq '.entries | length' cur_numatb.json) materials"
echo "meshex:   $(jq '.mesh_object_groups | length' cur_numshexb.json) groups"
echo
echo "textures are NOT shipped: config.json declares the paths and share-to-added points them at"
echo "each source fighter's vanilla textures, so no Nintendo texture data is redistributed."
