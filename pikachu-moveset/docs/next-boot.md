# What the next boot needs to check, and why I could not

Written 2026-09-17, updated the same day after a second unattended session. The card holds a fully built, fully gated
build; what it does not hold is any evidence about how it LOOKS, because this machine has no way
to see the screen or press a button.

## The honest limit

There is no programmatic input path to this console. `/atmosphere/contents/` holds five titles --
three system ones, Smash, and `420000000000000E` (sys-ftpd) -- and no `sys-botbase` or equivalent.
So file transfer and log readback work, and **nothing can start a match or perform a move.**

Installing an input-injection sysmodule would change that and was deliberately NOT done: this
console runs HOS 23.0.0 on an Atmosphère 1.12.0 built by hand from `23_support`, which no released
Atmosphère supports, and it took real work to get there. Adding an unvetted sysmodule that hooks
HID to the only test device, with nobody present, is not a trade worth making without being asked.
It is a reasonable thing to set up later, with a person watching.

What that leaves genuinely verified is everything that is a number, and everything that is a file:
39 `check.sh` sections, hitbox parity at 38 moves and 460 hitbox-frames within 0.05 world units
(radii included), the weapon gates, the blast-radius gates, and 639 files on the card verified by
size with the plugin `.nro` md5-identical to the local build.

## Boot once and look at five things

One change this round, and it is a revert to the game's own structure rather than a new idea. The
full derivation is in `the-weapons.md`; the short version is that `add-bones.sh` had reparented
Pikachu's vanilla `HaveL` from `HandL` to `HaveR` so Palutena's staff could ride a bone he already
owned, and **every native skeleton in the game parents `HaveL` to `HandL`.** Ours was alone in
hanging the left hand's item socket off the right hand's. Palutena's staff now rides a grafted
`Stick` under `HaveR`, verbatim from her own skeleton -- same parent, same (0,-2.2,0) transform --
which is exactly how her body model does it.

**1. Does it load at all?** Any Pikachu, training mode. Gates everything else. Rollback:

    git revert 370c6b6 && bash tools/merge-weapons.sh && bash tools/build.sh
    bash tools/deploy.sh

`tools/merge-weapons.sh` is NOT a build step -- it reads weapon models pulled off the console into
`work/arc-weapons` -- so a revert touching it needs that line before `build.sh`.

**2. Corrin's up-air, facing left.** This is the one that identifies the culprit. He was reported
correct in both directions, then broke, and the `HaveL` reparent is the change in that window --
he single-binds his Yato to `HaveR`, so he was the weapon most exposed to a foreign child hanging
off it. If Corrin is right again, the reparent was the fault and the same cause covers the rest.

**3. Palutena's n-air, facing left.** Her staff moved from the repurposed `HaveL` to a native
`Stick`. Her 65-keyframe `Stick` track drives it with no rename and her hitboxes anchor to `stick`
the way her own ACMD wrote them, so this is the first time her staff has been on the structure the
game uses. Watch the sweep as well as the placement.

**4. Chrom's jab and Mythra's dash attack / side tilt, facing left.** These ride grafted bones
under `HaveR`. If they come right too, `HaveR` was being corrupted by its foreign child and the
whole facing bug is one cause. If Corrin and Palutena come right and these two do not, then the
grafted-bone case is separate and that is a genuinely new fact -- say which.

**5. Cloud's back-air must still be CORRECT in both directions.** Nothing about him changed and he
is the control. He is also the only weapon that was never broken, and the reason is now understood:
his Buster Sword is held in BOTH hands on the body's centreline, where a mirror across the midline
is nearly the identity. George's mod is immune the same way -- it dual-wields, appending the same
object twice under one material on `HaveR` and `HaveL`, identical extents in all three axes. Both
are symmetric, not correct.

Also worth a glance: two `sys_smash_flash` effects the source ACMD anchors to `havel` were firing
at the RIGHT hand while `HaveL` was reparented. They should now appear at the left hand.

### If it is still wrong, the probe has already narrowed it

The 262-sample capture in `work/pt44.log` established something that holds regardless of whether
this fix works, and it is the sharpest fact this project has about facing:

    |handr - haver|   1.030 on EVERY frame facing right;   9.14 .. 13.31 facing left
    |handr - origin|  identical between facings, all six motions
    |haver - origin|  differs between facings, all six motions

measured on frames where `|handr - origin|` agrees between the two facings to within 0.002, so the
two performances are the same pose frame for frame. The distance from a bone to its own PARENT is
rigid skeletal geometry and cannot depend on facing. `HandR` is correct in both facings; its child
`HaveR` is not, and neither is `HaveL`. Every weapon in the moveset hangs off `HaveR`.

To re-measure after this build:

    bash tools/ftpq.sh get /rat-telemetry.log > work/pt45.log
    grep 'MIRROR ' work/pt45.log

**Note the `%20`.** `ftpq.sh` takes a URL path, so a space in the mod folder name must be encoded
or `get` returns 0 bytes with exit 0 -- an empty file that compares as DIFFER on every retry and
looks exactly like a failed deploy.

What the probe CANNOT do: read a grafted bone. Across all 262 samples `sword`, `sword1` and
`fusionsword1` return byte-identical positions to each other, and they carry three different bind
transforms (an arbitrary rotation, a 90-degree turn about Z, and the identity), so they cannot be
in one place. A name the skeleton lacks returns the fighter origin rather than failing -- `sword2`
comes back as the exact negation of `haver_from_top`. Every grafted-bone conclusion this project
ever drew from that call is void, including the one that justified the `HaveL` repurpose.

## What is settled and does not need looking at

**Palutena's staff sweep.** `hbtrace` against her own skeleton: 96 active hitbox-frames, deviation
0.00 on every one. The two staff-end hitboxes trace a clean circle in the Y-Z plane, radius ~7.3,
x constant at -2.18 -- and two hitboxes at different offsets both matching pins the bone's
rotation, not just its position, up to an invisible roll about the staff's own axis. The hitbox
half of that complaint was the 18.53-unit rebase and it is fixed. If it still looks odd, the cause
is not in any number this project can compute.

**The lariat's reach.** 11 active hitbox-frames, deviation 0.00, `arml` measured rather than
skipped. The swing is mostly in Z (+2.97 to -7.20) with x between -3.35 and -0.26, so it passes
through Pikachu reaching about 7 units either side, which is why every hit lands at `off=+7.08`.
That is Incineroar's own move on a body a fraction of his size. Changing it means departing from
the source, which is an ADR, not a bug fix.

## What is blocked, and on what

**Ptooie, the rope, the parachute and the dragon hand** all need a real article, and the clone
crashes the fight load -- now root-caused rather than suspected, to
`Singleton<FighterParamAccessor2>::instance_->field_0x1228` being null at `main + 0x3476e44`. Two
one-line experiments are written up in `the-weapons.md` and neither was run, because each costs a
crash on entry to a match and there was nobody to see it. `CLONE_ENABLED` is `false`.

**Performance** is the one axis of "exactly as native" that was not measured at all, because it
cannot be measured from here. One thing was found and fixed in passing: the new per-frame probe
first used `sink::diag`, which pays a file open and an SD flush PER LINE by design, and at up to
190 calls a session that is exactly the kind of hitch that would make the mod differ from native.
It uses the buffered `sink::log` now, which flushes every 8 frames.
