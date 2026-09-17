# Bone substitutions for hitboxes anchored to bones Pikachu does not have at RUNTIME.
#
# NOT EMPTY ANY MORE, AND THE REASON IS NEW EVIDENCE RATHER THAN A CHANGE OF MIND.
#
# This file was emptied because remapping was LOSSY: the old version rewrote `stick` to `haver`,
# which threw away Stick's [0,-2.2,0] offset and misaimed Palutena's n-air hitboxes, and rewrote
# `sword1` to `haver`, which threw away a large bind rotation and misaimed an 8.4-unit offset.
# Both of those criticisms were correct. The conclusion drawn from them -- that the bones should
# simply be grafted on instead -- is the part that turned out to be wrong.
#
# `telemetry::mirror_probe` now says, with two controls in the same log line, that a GRAFTED bone
# is not reliably in the runtime transform chain:
#
#   handr   a vanilla bone, HaveR's own parent: reads IDENTICALLY in both facings on every frame,
#           so the pose is the same and the probe's frame math is right.
#   stick   differs between facings in all three components INCLUDING Y, which no facing changes.
#   sword2  a bone the animation drives and this mod never grafts: comes back as the exact
#           negation of `haver_from_top`, which is algebraically the fighter origin. So a missing
#           bone returns the ORIGIN rather than failing, and appended bones read like memory past
#           the end of an array sized for Pikachu's vanilla bone count.
#
# Playtest, after the weapon meshes were baked onto HaveR: "the facing left problem remains for
# all three attacks", "palutena nair isn't animated correctly and has wrong hitbox".
#
# WHAT MAKES THE REMAP LOSSLESS NOW. `tools/add-bones.sh` reparents the vanilla `HaveL` under
# `HaveR` and gives it Stick's bind transform, and `tools/rename-stick-track.sh` moves the
# animation's `Stick` track onto it. `HaveL` therefore occupies Stick's frame EXACTLY, on every
# frame, on a bone the runtime owns. A hitbox offset expressed relative to `stick` means the same
# thing relative to `havel`, so this substitution needs no transform and loses nothing -- which is
# the thing the old version could not do.
#
# `Sword1` is NOT remapped here. Chrom's jab hitboxes ride a bone his animation drives, and the
# right tool for that is `tools/reach/src/bin/rebase.rs`, which carries the bone's own per-frame
# transform into the offsets instead of discarding it. A sed cannot do that.
s/Hash40::new("stick")/Hash40::new("havel")/g
