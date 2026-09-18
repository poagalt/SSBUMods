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

One rule this round, and it came from the cleanest A/B this project has had: **a weapon mesh must
be single-bound to a bone vanilla Pikachu already ships.** Palutena's staff was shipped in two
configurations that are geometrically IDENTICAL by construction -- same parent `HaveR`, same
`Stick` bind transform, same vertices -- once on the vanilla `HaveL` (index 17) and once on an
appended `Stick` (index 88). The vanilla one was correct facing right; the appended one was wrong in
both directions. And the playtest said of the appended version "sweeps, but in the wrong place", so
the bone was driven and it was the MESH that rendered elsewhere.

All five weapons are now on vanilla bones: chrom, cloud, mythra and corrin single-bound to `HaveR`,
and palutena's staff on the vanilla `Mouth` repurposed into `Stick`'s frame.

**1. Does it load at all?** Any Pikachu, training mode. Rollback:

    git revert 475a795 && bash tools/merge-weapons.sh && bash tools/build.sh
    bash tools/deploy.sh

`tools/merge-weapons.sh` is NOT a build step -- it reads models pulled off the console into
`work/arc-weapons` -- so a revert touching it needs that line before `build.sh`.

**2. CLOUD'S BACK-AIR, FIRST, because it is the one thing that argues against this.** He moved from
the appended `fusionsword1` onto the vanilla `HaveR`, and his bake matrix is the IDENTITY, so his
geometry provably did not change -- only which bone he binds to. An earlier round made that same
move and the playtest said his back-air went wrong. That result is unexplained and it was measured
while Chrom's jab was shipping 51 Transform tracks for bones that do not exist, so the round was
confounded. If Cloud breaks again on an identity matrix, the rule is wrong and that is the single
most important thing to know.

**3. Chrom's jab and Mythra's dash attack / side tilt, facing left.** These are the weapons the rule
predicts it fixes. Chrom was "at the other hand's position" facing left -- a mirror across the
centreline -- and he is now on the same vanilla bone Corrin uses.

**4. Palutena's n-air, both directions.** Her staff is on the vanilla `Mouth`, parented to `HaveR`
with her own `Stick` bind transform and driven by her own 65-keyframe track renamed onto it. The
gates verify all four of those against her own skeleton. Watch the sweep as well as the placement.

**5. Corrin's up-air must STAY correct.** He is untouched this round and he is the control -- he was
already on the vanilla `HaveR`, which is what his own body model does, and that is the whole basis
for the rule.

Also: two `sys_smash_flash` effects the source ACMD anchors to `havel` should now appear at the
LEFT hand. They were firing at the right hand while `HaveL` was reparented.

### What is now known about the facing bug, and what the probe cannot tell you

**Hitbox placement is the game's own ground truth and it says the bones are fine.** The playtest
reports Chrom's and Mythra's hitboxes correct in BOTH directions while their visuals are wrong, and
their hitboxes ride `haver`. So `HaveR`'s runtime transform is correct facing left, and the fault is
in the mesh, not the skeleton.

That RETRACTS the probe reading that said otherwise. `mirror_probe` had `|handr - haver|` at 1.030
facing right and 9.14 .. 13.31 facing left, which would be impossible for a bone and its own parent
-- but hitboxes off `haver` land correctly, so the reading is wrong. It is the third time that call
has misled this project. Also established, from the same capture: it **cannot read an appended bone
at all** -- `sword`, `sword1` and `fusionsword1` return byte-identical positions across all 262
samples despite carrying three different bind transforms, and a name the skeleton lacks returns the
fighter origin rather than failing.

Treat `ModelModule::joint_global_position` as unreliable for anything but ordinary body bones, and
prefer hitbox placement as the instrument.

    bash tools/ftpq.sh get /rat-telemetry.log > work/pt46.log

**Note the `%20`**: `ftpq.sh` takes a URL path, so a space in the mod folder name must be encoded or
`get` returns 0 bytes with exit 0 -- an empty file that compares as DIFFER on every retry.

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
