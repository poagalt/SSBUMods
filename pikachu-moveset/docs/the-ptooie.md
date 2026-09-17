# Ptooie: the transcription source

Everything needed to transcribe Piranha Plant's neutral special onto Pikachu, recovered rather
than inferred. Nothing here is a plan; it is the reference sheet the Rust gets written from, in
the same form `specials/whip/mod.rs` cites Incineroar.

**THIS IS BLOCKED, AND NOT BY ANYTHING BELOW.** The experiment in `src/articles/mod.rs` -- step 2,
does Pikachu load with the cloned article appended? -- has now run, and it fails: Data Abort at
`0x138` off a null base, thread `TaskWorker2`, inside the game's own weapon-creation path
(`work/crash-0917b.log`). Clone off loads 42 of 42; clone on, 0 of 10. All three previously
suspected confounders are eliminated and verified, so the clone itself is the cause. See "The
clone, and the three articles waiting behind it" in `the-weapons.md` for the full narrowing and
the next step, which needs no console.

Nothing in this file is wasted by that -- it is the source, and the source does not change. But
none of it can become a shipping move until an article can exist, so do not start writing the
Rust: `clone_weapon` installs and then the fighter never builds.

## How the source was recovered

    tools/disasm/target/release/disasm.exe \
      vendor/nro/prebuilt/nro/release/lua2cpp_packun.nro --statusmap 0x7100000480

recovers all of Piranha Plant's status functions with their kind and line (full output saved to
`work/packun-statusmap.txt`), then

    bash tools/decomp.sh packun <addr>...          the L2C status scripts
    bash tools/decomp.sh main <addr>...            the native FighterSpecializer helpers

Twenty-eight functions, all decompiled: `work/ghidra/packun-7114c7.annotated.c` (one),
`work/ghidra/packun-711514.annotated.c` (twenty), `work/ghidra/packun-7131dd.annotated.c` (the
seven main loops).

**Run `decomp.sh` through the normal tool path, not detached.** It drives Ghidra through
`cmd //c analyzeHeadless.bat`, and a `nohup`-ed copy with no console dies silently leaving a
zero-byte log, which reads exactly like a tool that is not installed. That cost one wasted batch
here.

## The status tree

Seven statuses, one tree for BOTH ground and air -- the situation is a branch inside each, not a
parallel set. The ACMD side is the opposite (`SpecialN*` and `SpecialAirN*` are separate scripts),
so the two do not line up one-to-one and should not be made to.

| status | value | PRE | MAIN | END | main loop |
|---|---|---|---|---|---|
| `FIGHTER_STATUS_KIND_SPECIAL_N` | `0x1DC` | `0x7100014c70` | `0x7100015140` | `0x71000154c0` | `0x7100033ce0` |
| `..PACKUN.._SPECIAL_N_LOOP` | `0x1E1` | `0x7100015600` | `0x71000159e0` | `0x7100015da0` | `0x71000336a0` |
| `..PACKUN.._SPECIAL_N_WAIT` | `0x1E2` | `0x7100015f20` | `0x71000162f0` | `0x7100016680` | `0x7100033190` |
| `..PACKUN.._SPECIAL_N_SHOOT` | `0x1E3` | `0x7100016700` | `0x7100016ae0` | `0x7100017550` | `0x7100032bb0` |
| `..PACKUN.._SPECIAL_N_END` | `0x1E4` | `0x71000175d0` | `0x71000179a0` | `0x7100017e30` | `0x7100032710` |
| `..PACKUN.._SPECIAL_N_HIT_END` | `0x1E5` | `0x7100017f20` | `0x71000182f0` | `0x7100018540` | `0x7100032270` |
| `..PACKUN.._SPECIAL_N_FAILURE` | `0x1E6` | `0x7100018550` | `0x7100018920` | `0x7100018bd0` | `0x7100031dd0` |

Values from `vendor/main/consts-1305.csv`, which is the game's own registration emulated, not a
third-party table. `FIGHTER_STATUS_KIND_SPECIAL_N` is `0x1DC` and is also
`FIGHTER_STATUS_KIND_COMMON_NUM` -- it is the last COMMON status, so that one number is portable
to Pikachu unchanged. `0x1E1` upward are Piranha Plant's own and are NOT.

## FIGHTER_STATUS_KIND_SPECIAL_N, PRE, 0x7100014c70

    sub_status_pre_SpecialNCommon()
    if ArticleModule::is_generatable(FIGHTER_PACKUN_GENERATE_ARTICLE_SPIKEBALL):
        StatusModule::init_settings(SITUATION_KIND_NONE, FIGHTER_KINETIC_TYPE_UNIQ,
            GROUND_CORRECT_KIND_KEEP, GROUND_CLIFF_CHECK_KIND_NONE, true,
            *_WORK_KEEP_FLAG_NONE_FLAG, *_NONE_INT, *_NONE_FLOAT, 0)
        FighterStatusModuleImpl::set_fighter_status_data(false,
            FIGHTER_TREADED_KIND_NO_REAC, false, false, false,
            FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_N | .._ACTION_CATEGORY_ATTACK
              | .._ACTION_TRIGGER_ON | FIGHTER_LOG_MASK_FLAG_SHOOT,
            FIGHTER_STATUS_ATTR_DISABLE_TURN_DAMAGE | FIGHTER_STATUS_ATTR_START_TURN,
            FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_N, 0)
        return false
    else:
        StatusModule::set_status_kind_interrupt(SPECIAL_N_FAILURE)
        return true

The two mask arguments are ORs of four and two `lua_const`s that the decompiler leaves as raw
offsets into the const table. They were resolved through `vendor/ssbu-status/lua_dict.txt`, which
is **hex-keyed with uppercase letters** -- searching for `96c:` finds nothing and `96C:` finds
`FIGHTER_LOG_MASK_FLAG_ACTION_TRIGGER_ON`. Worth knowing before concluding a constant is unknown.

`FIGHTER_PACKUN_GENERATE_ARTICLE_SPIKEBALL` is **`0x0`**, not the `0x3` that the table in
`the-weapons.md` claimed; `0x3` is `..._BOSSPACKUN`. Ours is
`FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL = 0x6`, appended after Pikachu's six vanilla articles
-- a different number for a different fighter, both correct.

## FIGHTER_STATUS_KIND_SPECIAL_N, MAIN, 0x7100015140

    if globalTable[0x16] == SITUATION_KIND_GROUND:
        GroundModule::correct(GROUND_CORRECT_KIND_GROUND_CLIFF_STOP)
        MotionModule::change_motion("special_n_start", 0.0, 1.0, false, 0.0, false, false)
    else:
        GroundModule::correct(GROUND_CORRECT_KIND_AIR)
        MotionModule::change_motion("special_air_n_start", ...)
    WorkModule::set_float(PostureModule::scale(), .._SPECIAL_N_WORK_FLOAT_START_SCALE)
    sub_shift_status_main(0x7100033ce0)

Both motion hashes were CONFIRMED by hashing the candidate, not read off a list:
`special_n_start` is `0xf3a6aace3` (in the name list) and `special_air_n_start` is `0x1331f32137`
(in no list -- its length byte `0x13` is 19, `special_air_n_start` is 19 characters, and
`tools/hash40.sh special_air_n_start` returns exactly the value in the decompile).

## The main loop, 0x7100033ce0

    if WorkModule::is_flag(.._SPECIAL_N_FLAG_GENERATE_ARTICLE_SPIKEBALL):
        ArticleModule::generate_article(SPIKEBALL, false, -1)
        WorkModule::off_flag(.._SPECIAL_N_FLAG_GENERATE_ARTICLE_SPIKEBALL)

    if ArticleModule::is_exist(SPIKEBALL) and not <ball status in sync>:
        change_status(SPECIAL_N_HIT_END)
    elif MotionModule::is_end():
        change_status(SPECIAL_N_LOOP)
    elif <situation changed since last frame>:          globalTable[0x17] vs [0x16]
        ground: GroundModule::correct(GROUND_CORRECT_KIND_GROUND_CLIFF_STOP)
                KineticModule::change_kinetic(FIGHTER_KINETIC_TYPE_GROUND_STOP)
                MotionModule::change_motion_inherit_frame("special_n_start", -1.0, 1.0, ...)
                if LinkModule::is_linked(WEAPON_LINK_NO_CONSTRAINT):
                    ArticleModule::change_motion(SPIKEBALL, "start", true, -1.0)
        air:    GroundModule::correct(GROUND_CORRECT_KIND_AIR)
                KineticModule::change_kinetic(FIGHTER_KINETIC_TYPE_AIR_STOP)
                MotionModule::change_motion_inherit_frame("special_air_n_start", -1.0, 1.0, ...)
                if LinkModule::is_linked(WEAPON_LINK_NO_CONSTRAINT):
                    ArticleModule::change_motion(SPIKEBALL, "start_air", true, -1.0)

The article's own motion names are short: `start` is `0x59f79558f` (in the name list) and
`start_air` is `0x9b00ba75d`. `air_start` was the obvious guess for that second one and is WRONG
(`0x9bdc1e36b`); `start_air` matches exactly. Both were settled by hashing, which is the only way
a name outside all three lists ever gets confirmed.

## The one thing that cannot be transcribed literally

The sync test above is a call to a native helper:

    app::FighterSpecializer_Packun::is_special_n_spikeball_changed_status(Fighter&)
    main 0xe0e720, 0x228 bytes

It takes the FIGHTER, and the standing rule is that a source fighter's `FighterSpecializer_*`
helpers are never called on Pikachu -- the rule that exists because `retarget_forward_throw` wrote
Incineroar's `0x1F7` into native Donkey Kong, where `0x1F7` is `THROW_F_HI`, and killed the game.
Decompiled, the helper is exactly that hazard:

| fighter status | expects the ball in |
|---|---|
| `FIGHTER_STATUS_KIND_SPECIAL_N` `0x1DC` / `FIGHTER_KIRBY_STATUS_KIND_PACKUN_SPECIAL_N` `0x328` | `WEAPON_PACKUN_SPIKEBALL_STATUS_KIND_START` `0x0` |
| `..PACKUN.._SPECIAL_N_LOOP` `0x1E1` / `..KIRBY.._PACKUN_SPECIAL_N_LOOP` `0x329` | `.._LOOP` `0x1` |
| `..PACKUN.._SPECIAL_N_WAIT` `0x1E2` / `..KIRBY.._PACKUN_SPECIAL_N_WAIT` `0x32A` | `.._WAIT` `0x2` |

anything else, false. It is "is the ball in the status that goes with mine", and the parallel
`0x328..0x32A` family is Kirby's -- the game already writes this check twice, once per fighter
that owns the move, which is the clearest possible statement that the numbers are per-fighter.

On Pikachu, `0x1DC` still means `SPECIAL_N` because it is a common status. `0x1E1` and `0x1E2` do
not mean anything like LOOP and WAIT -- they are whatever Pikachu's own status table has there --
so the helper would compare Pikachu's status against Piranha Plant's numbers and report "out of
sync" essentially at random, sending the move to `HIT_END` on frames where nothing was hit.

**So it gets reproduced, not called**, and reproducing it is three status compares against OUR
status numbers plus the ball's own: the logic transcribed, only the fighter-specific constants
rebased. `RAT_STATUS_KIND_NUM` is `0x207` today, so the Ptooie statuses append above the whip's.

Two guard calls at the top of the helper are NOT yet identified, and are recorded rather than
guessed: a vtable `+0x1c8` bool and `+0x208` pointer on the module at fighter `+0x20` then
`+0x98`, and a vtable `+0x78` bool taking **3** on the module at `+0xd0`. That `3` is not the
spikeball's article id, which is `0x0`, so the obvious reading of it as `is_exist` is wrong. They
gate the whole body, so they matter; the status table above is independent of them.

## Also available, and likely needed

    app::FighterSpecializer_Packun::get_special_n_spikeball_speed_mul(Fighter&)   main 0xe0e950
    app::FighterSpecializer_Packun::update_special_n_spikeball_pos(Fighter&)      main 0xe0ebb0

Both decompiled to `work/ghidra/main-ee72.annotated.c` alongside the first. Both take `Fighter&`
and carry the same hazard; both will need the same treatment.

## What the clone does give for free

`clone_weapon(.., use_original_code: true)` makes smashline load Piranha Plant's fighter module
and run the spikeball's OWN code (`get_weapon_code_dependency`, `vendor/sl_create_agent.rs`), so
the ball's arc, its juggling, its damage and its own status machine are the game's, exactly right,
and none of it is in the table above. That is the whole reason this route is worth the restarts:
the fighter side is seven statuses, and the article side is zero.

The spikeball's assets are also self-contained, which matters after the `share-to-added` hang --
`extracted/fighter/packun/model/spikeball/c00/` carries its own
`metal_spikeball_001_{col,nor,prm}.nutexb`, so shipping the directory needs no alias and no
texture resolves through the vanilla arc.
