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

Two defects were found and fixed this round, both by measurement rather than by guessing, and each
lands on exactly one weapon -- so the result is attributable per weapon instead of being one more
"almost every sword changed" report. The full derivation is in `the-weapons.md`.

**1. Does it load at all?** Any Pikachu, training mode. Gates everything else. The rollback is one
revert of this session's commit, then rebuild and redeploy:

    git log --oneline -1          # note the hash
    git revert <hash> && bash tools/merge-weapons.sh && bash tools/build.sh
    bash tools/deploy.sh

`tools/merge-weapons.sh` is NOT a build step -- it reads weapon models pulled off the console into
`work/arc-weapons` -- so a revert that touches it needs that line run by hand before `build.sh`.

**2. Chrom's Falchion, jabbing LEFT.** This is the round's likeliest real fix and the one to look at
first. His jab was shipping `c00attack11.nuanmb` with 96 Transform tracks against an 88-bone
skeleton, **51 of them naming bones Pikachu does not have.** He was the only source fighter whose
`scale` param (0.9600) exactly equals Pikachu's, which put him on an `anim-dropin.sh` fast path that
copied the animation verbatim and skipped the foreign-track trim entirely. All 74 shipped animations
were audited; his and Ganon's ledge attack were the only two affected, and both are 0.9600 fighters.

**3. Mythra's dash attack and side tilt.** Her sword was the only SKINNED graft, and skinning
substitutes Pikachu's bind pose for the weapon model's hand frame -- so the mesh followed the bone's
DEVIATION from bind pose instead of its absolute pose. Her blade tip was **3.951 world units** from
where the native attachment puts it, at bind pose, and further off in motion. It is now baked into
`HaveR`'s frame and single-bound there, verified at 0.0004 units from the native prediction and
10.874 from `HaveR`, which is the blade's own length. Expect the blade in the hand.

Incidentally this also puts her blade on local **+Y** in `HaveR`'s frame, the same axis as Cloud's
and Corrin's -- the two weapons that have never been reported broken. See the axis table in
`the-weapons.md`; she is now the clean test of that correlation.

**4. Cloud's back-air and Corrin's up-air must still be CORRECT, both directions.** Nothing about
either was changed, deliberately: their grafts are algebraically identical to the native attachment
(the proof is in `the-weapons.md`) and they are the controls. If either regressed, something global
went wrong and the per-weapon reasoning above is void -- say so before anything else.

**5. Palutena's n-air is EXPECTED to still be wrong facing left.** No fix was attempted for her this
round, because nothing measurable pointed at one: her hitboxes trace her own staff's circle at
deviation 0.00 across all 96 active frames, and her mesh is single-bound to the same bone. She is
the only weapon whose bind bone was replaced outright rather than grafted.

### Then pull the log, because the probe was rebuilt and has never run

Play the six sword moves in both directions, then:

    bash tools/ftpq.sh get /rat-telemetry.log > work/pt43.log
    grep MIRRORROT work/pt43.log
    grep 'MIRROR ' work/pt43.log

Three things changed in `telemetry::mirror_probe`, and the first two mean the last round's Palutena
and Cloud readings were worthless:

- **It was reading a bone that does not exist.** It logged `stick`, and `add-bones.sh` stopped
  creating `Stick` when it began repurposing the vanilla `HaveL` instead. Every Palutena sample was
  a read of a name no skeleton contains -- and a missing bone returns the fighter origin rather than
  failing, so it returned plausible-looking numbers. It reads `havel` now. **This is the measurement
  that decides where the bug can still live:** her staff is the only weapon bound to a bone Pikachu
  already owned, so if `havel` tracks `handr` across both facings -- and `handr` is a fixed offset
  from `haver` by construction, so it must -- the skeleton is carrying the staff correctly and no
  further skeleton work can fix any of these.
- **`attack_air_b` was never sampled.** Cloud's back-air is the only weapon ever called correct in
  both facings, and a broken sample is only interpretable beside a working one from the same
  instrument. All six ported sword motions are sampled now.
- **It only ever logged POSITIONS**, and a position cannot tell a 180-degree turn from a mirror --
  both put the hand in the same place and only one turns the blade inside out.
  `ModelModule::joint_global_rotation` is in the binary's lua_bind surface at `0x2057680` with a
  clean `Vector3f` out-param and had never been called. The new `MIRRORROT` line carries it raw,
  with no `lr` applied, because dividing by the answer would erase it.

What to read out of `MIRRORROT`: compare each bone's two facings against each other. `handr` and
`haver` are vanilla and define what "correct" looks like. If `fusionsword1` (works) changes with
facing the way they do, while `sword`, `sword1` and `havel` (all three wrong facing left) do not,
that is the mechanism, measured, and it is the first direct evidence about facing this project will
have had.

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
