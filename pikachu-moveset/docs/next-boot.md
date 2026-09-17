# What the next boot needs to check, and why I could not

Written 2026-09-17 at the end of an unattended session. The card holds a fully built, fully gated
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

## Boot once and look at four things

**1. Does it load at all?** Any Pikachu, training mode. This is the only question that gates the
rest, and the build has 525 more files and 226 more `config.json` declarations than the last build
anyone played. If it hangs on the loading screen, that is the shape `share-to-added` had, and the
rollback is one revert:

    git revert e13b231 && bash tools/build.sh && bash tools/deploy.sh

`deploy.sh` retires the 525 mirrored files from the card by itself now -- the retire list is
computed from the manifest rather than hand-written.

**2. Mythra's sword, and Corrin's.** These are the session's main fix. Mythra's blade was grafted
at subindex 0 only, which is 1,320 of its 10,044 vertices -- 13%, and the playtest description was
"very small slivers where the middle of the sword would be". All four subindices ship now, with all
nine of the bones they are weighted to instead of three. Corrin's Yato had no table row at all;
it is rigid on `HaveR`, a bone Pikachu already has. Expect a whole sword in each case:
Mythra on her dash attack and three tilts, Corrin on his up-air.

**3. Chrom's Falchion jabbing LEFT.** Still unexplained, and the instrument that was supposed to
explain it was reading the wrong bone: his MESH binds to `Sword` and his HITBOXES to `sword1`, two
separate bones under `HaveR` with different local rotations, and `mirror_probe` logged `sword1`.
It now logs both, every frame to 24, both facings, relative to `haver` in the fighter's own frame.
So: jab facing right, then facing left, then pull the log --

    bash tools/ftpq.sh get /rat-telemetry.log > work/pt42.log
    grep MIRROR work/pt42.log

-- and `sword` reading the same both ways means the graft follows the hand and the problem is
elsewhere; diverging means it does not. Ruled out already, offline: every grafted bone's local
basis is a proper rotation with unit scale (determinant +1, no reflection), so a mirrored basis is
not the cause.

**4. Any non-default Pikachu colour.** Previously the moveset simply did not exist on c01..c07 --
30 ported animations absent, 45 modified ones vanilla. All eight ship now. Expect every move to
work with the right animation and the right colour, and **no weapons**: the grafted model stays
c00-only on purpose, because each costume's body model carries its own textures and mirroring
c00's would turn every Pikachu default yellow.

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
