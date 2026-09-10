# Bone substitutions for hitboxes anchored to bones Pikachu does not have.
#
# Now empty, and that is the point. Every bone the ported ACMDs reference is present on Pikachu:
#   - the 12 limb bones (armr, arml, kneer, kneel, toel, toer, shoulderl, shoulderr, handr,
#     handl, haver, havel) are in his vanilla skeleton;
#   - `Sword1` (Chrom jab) and `Stick` (Palutena n-air) are grafted on by tools/add-bones.sh;
#   - `top` is the fighter origin, not a skeleton bone.
#
# The previous version rewrote sword1/stick/to_open/to_close to `haver`. That silently discarded
# Sword1's bind rotation and Stick's [0,-2.2,0] offset, misaiming those hitboxes -- see the header
# of tools/add-bones.sh. `to_open`/`to_close` were never referenced by any ported script at all.
#
# tools/check.sh fails the build if a referenced bone is missing, so a new port that needs one
# will be caught rather than quietly remapped.
