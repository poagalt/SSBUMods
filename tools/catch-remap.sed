# Retargets Alolan Whip's command grab at the standard capture status.
#
# Incineroar's CATCH names FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED. That status IS global -- any
# fighter can be the victim of it -- but it is used by exactly one fighter in the whole game, and
# when the catch connects the engine follows through into Incineroar's OWN grabber statuses
# (FIGHTER_GAOGAEN_STATUS_KIND_SPECIAL_S_SHOULDER and friends). Pikachu's extra status table ends
# at 0x1F0 and has no equivalent, so the follow-through fails and the victim is released the frame
# after being caught -- which is exactly the reported symptom: the grab travels, appears to miss,
# and Pikachu snaps back while the opponent plays the freed-from-grab animation.
#
# FIGHTER_STATUS_KIND_CAPTURE_PULLED is the standard capture every normal grab uses, including the
# Palutena grab already ported here, and Pikachu can complete it.
#
# FIDELITY COST, stated plainly: the victim is held in the ordinary captured pose rather than
# roped and swung, so Alolan Whip loses Incineroar's distinctive victim animation. The mechanic --
# a travelling command grab with three follow-ups -- works. Getting the roped version would need
# Incineroar's grabber statuses, which cannot be added, or his ROPE article, which is the same
# clone machinery that still crashes for Ptooie.
s/\*FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED/*FIGHTER_STATUS_KIND_CAPTURE_PULLED/g
