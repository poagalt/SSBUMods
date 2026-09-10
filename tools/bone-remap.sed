# Bone substitutions for hitboxes anchored to bones Pikachu does not have.
# Verified against Pikachu's vanilla model.nusktb; `top` is present despite not
# appearing in a strings dump (287 uses in Pikachu's own scripts).
#
# All four map to `haver`, the item-hold bone -- semantically where a held weapon
# sits, and where the Chrom Falchion will be attached. Offsets are inherited from
# the source fighter and are NOT yet recalibrated for Pikachu's proportions.
s/Hash40::new("sword1")/Hash40::new("haver")/g
s/Hash40::new("stick")/Hash40::new("haver")/g
s/Hash40::new("to_open")/Hash40::new("haver")/g
s/Hash40::new("to_close")/Hash40::new("haver")/g
