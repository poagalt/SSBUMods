# Bone substitutions for hitboxes anchored to bones Pikachu does not have at RUNTIME.
#
# EMPTY AGAIN, AND THIS TIME BECAUSE THERE IS NOTHING LEFT TO REMAP.
#
# It held one rule, `stick` -> `havel`, and that rule existed to serve a scheme that has been
# deleted: `add-bones.sh` used to reparent the vanilla `HaveL` under `HaveR` and give it Palutena's
# `Stick` bind transform, so a hitbox offset expressed against `stick` could be repointed at
# `havel` with no transform. The remap was genuinely lossless under that scheme.
#
# The scheme was the mistake. Palutena's staff is a mesh in her BODY model single-bound to `Stick`,
# and `Stick` is a real bone in her skeleton under `HaveR` with the identity rotation and a
# (0,-2.2,0) translation. `add-bones.sh` now grafts exactly that bone, so `stick` IS a bone Pikachu
# has, her animation's own `Stick` track drives it with no rename, and her hitboxes anchor to
# `stick` the way her own ACMD wrote them. Nothing to substitute.
#
# The evidence that retired the scheme is in `add-bones.sh`'s header: the probe reading that
# justified it cannot see grafted bones at all -- `sword`, `sword1` and `fusionsword1` return
# byte-identical positions on all 262 samples despite having three different bind transforms --
# and every native skeleton in the game parents `HaveL` to `HandL`, which ours was alone in not
# doing.
#
# `Sword1` is still NOT remapped here. Chrom's jab hitboxes ride a bone his animation drives, and
# the right tool for that is `tools/reach/src/bin/rebase.rs`, which carries the bone's own
# per-frame transform into the offsets instead of discarding it. A sed cannot do that.
s/Hash40::new("stick")/Hash40::new("mouth")/g
