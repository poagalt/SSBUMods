# The article path: why the clone crashes, and which articles it crashes for

Written 2026-09-17 on the instruction to find out why the article path crashes and how to fix it,
because the port wants the game's own attachment rather than a merged mesh.

Everything here was read out of the dumps in `vendor/main/` with `tools/disasm` and the
`work/ghidra/main-all-100k.c` corpus. **No console was used.**

## The one-line answer

`WeaponSpecializer_PackunSpikeball`'s own native code reads
`FighterParamAccessor2::instance_->fighter_param[FIGHTER_KIND_PACKUN]` with the index **compiled
in**, and that entry is null in a match with no Piranha Plant in it. The crash is a property of
**the article that was chosen for the experiment**, not of `clone_weapon`: auditing all 114 vtable
slots of every article this port has wanted says five of the seven never touch their donor's entry
at all, including both of the ones it actually needs. Pointing the clone at one of those is the
whole fix, and no patching is involved — the engine runs the donor's own code throughout.

## What the faulting instruction reads

`work/crash-0917b.log` gives `PC = main + 0x3476e44`. Read flat:

    0x3476e20  sub  sp, sp, #0x40             <- function entry (udf padding above it)
    0x3476e30  adrp x8, $+0x1e46000
    0x3476e34  ldr  x8, [x8, #0x3b0]          <- x8 = the value at 0x52bc3b0
    0x3476e38  ldr  x20, [x8, #0x1228]
    0x3476e3c  ldr  x19, [x1, #0x20]
    0x3476e40  ldr  x0, [x19, #0xb0]
    0x3476e44  ldp  x8, x9, [x20, #0x138]     <- FAULTS. X[20] is 0000000000000000 in the dump

and `0x52bc3b0` is, at offset +0x0 exactly,
`_ZN3lib9SingletonIN3app21FighterParamAccessor2EE9instance_E`.

`the-weapons.md` got this far and stopped at "the crash is in the fighter PARAM accessor". That
sentence names the **data** that is missing. It does not name the **code**, and the code is what
makes this fixable.

### 0x1228 is not a field. It is a fighter kind.

`FighterParamAccessor2__hit_target_no_impl` (`0x20a8080`, 0x54 bytes, exported) spells the layout
out in nine instructions:

    cmp    w1, #0x4c            ; if the kind argument is NANA
    mov    w8, #0x4b            ;   substitute POPO
    csel   w8, w8, w1, eq       ;   (Ice Climbers share one entry)
    mov    w9, #0x38            ; stride 56
    smaddl x8, w8, w9, x0       ; x8 = this + kind*0x38
    ldr    x0, [x8, #0x70]      ; ARRAY BASE IS +0x70, entry member 0 is a pointer
    ldr    x8, [x0]             ; ... with a vtable
    ldr    x8, [x8, #0x10]
    mov    x1, #0xadb509366     ; ... called with a hash40 param name

So the accessor carries an array of 56-byte entries at `this+0x70`, indexed by **fighter kind**,
and each entry's first member points at that fighter's param object. The NANA->POPO `csel` is the
independent confirmation that `w1` really is a fighter kind and not an arbitrary index.

Therefore

    (0x1228 - 0x70) / 0x38 = 81 = 0x51 = FIGHTER_KIND_PACKUN      remainder 0

The faulting read is `fighter_param[FIGHTER_KIND_PACKUN]`, and the index is **constant-folded into
the instruction stream**. There is no runtime kind anywhere in the function.

### It is Packun's code, which is why the index is a constant

`--xrefs 0x3476e20` finds **zero** call sites. The function's address appears exactly once in
`.data`, at `.data+0x2e02b0`, inside a table whose address point is `.data+0x2e0298` — so it is
**vtable slot 3** of a class. Walking that table and resolving each entry to its nearest symbol
gives 16 `WeaponSpecializer_PackunSpikeball` functions and 12 `WeaponSpecializer_PackunBosspackun`
functions and nothing else.

`WeaponSpecializer_PackunSpikeball` is also, of every donor this port has considered, **the only
one with an exported `WeaponSpecializer` class at all**:

    grep -oE "WeaponSpecializer_(Chrom|Roy|Cloud|ELight|Kamui|Gamewatch|Gaogaen|Packun)[A-Za-z0-9]*" \
      vendor/main/symbols-merged.txt | sort -u
    WeaponSpecializer_PackunBosspackun8end
    WeaponSpecializer_PackunSpikeball16get
    WeaponSpecializer_PackunSpikeball21update
    WeaponSpecializer_PackunSpikeball23is

Four names, all Packun. Chrom's sword, Cloud's Fusion Sword, Mythra's blade, the dragon hand and
the parachute have no such class.

### What the engine does everywhere else, and what that says the invariant is

Every other reader of this array takes the kind at **runtime, off the object**. From
`FUN_00b63960` and `FUN_00b63d70`:

```c
iVar3 = *(int *)(*(long *)(lVar10 + 0xf1c0) + 0x40);   // the owner's own fighter kind
iVar2 = 0x4b; if (iVar3 != 0x4c) iVar2 = iVar3;        // the same NANA -> POPO fixup
lVar4 = *(long *)(*(long *)(FPA2::instance_ + (long)iVar2 * 0x38 + 0x70) + 0x138);
```

There are 46 such dynamically-indexed sites in the corpus and 1874 reads of the singleton in all.
Essentially none of them null-check the slot. The invariant the engine relies on is *the entry for
a fighter kind that is in the match is populated* — and nothing populates an entry for a fighter
who is not there.

**This part was inference and is no longer: Part 10 disassembles the faulting function and confirms
every element of it -- the table base, the 56-byte stride and kind 81 folded into a literal `0x1228`
offset. The paragraph below stands as written; only its caveat is withdrawn, and the search it
describes turns out to have been impossible rather than merely unsuccessful, for a reason Part 10
gives.** The
writer of the array was not located: there are no stores through the singleton symbol in the
corpus, and no `str` at `#0x70` near a 0x38 multiply that looks like the loader. What is
established is that the slot *is* null at the faulting instruction (`X[20]` in the register dump),
that the engine reads it unguarded for the owner's own kind, and that the boot record is 42/42 with
the clone off and 0/10 with it on.

## Both experiments `the-weapons.md` proposes are no-ops

Neither needs a boot to rule out. Both were read out of `vendor/`.

**1. `add_param_object("pikachu", "spikeball")` cannot matter.** `sl_params.rs:313`'s
`init_fighter_p_object` installs a `get_param` reimplementation on **Pikachu's** own
`FighterParamHolder`, and it is gated on

```rust
if whitelisted_params.get(&fighter_id).is_none()
&& new_articles.get(&fighter_id).is_none() { return; }
```

With the clone on, `NEW_ARTICLES[pikachu]` is already non-empty, so the reimplementation is
**already installed** and adding a whitelist entry changes nothing. More to the point, that path
writes Pikachu's param holder; nothing in it can populate PACKUN's entry in the accessor.

**2. `use_original_code: false` cannot matter either, because the field is never read.** It is
written into `NewAgent` at `sl_api.rs:369` and that is the last anything does with it:

    grep -rn "use_original_code" vendor/ --include="*.rs"
      sl_api.rs:309       sl_api.rs:369       sl_weapons.rs:18
      smashline_lib.rs:421, 464, 471

All six are the declaration, the FFI signature and the store. The donor's fighter module is loaded
by `get_weapon_code_dependency(object.kind)` (`sl_create_agent.rs:582` and `:1079`), keyed on the
**weapon kind**, unconditionally. And the crash is in `main`, reached through the descriptor
`get_static_fighter_data` copies verbatim, which the flag could not reach even if it were live.

So `the-weapons.md`'s "two single-variable experiments, in this order, neither yet run" should be
struck. They are both zero-variable.

## The spikeball is two bugs, not one

Three functions in Packun's weapon code read PACKUN's entry unguarded. The vtable table below
attributes them exactly, and only two are the spikeball's — the third belongs to the boss Piranha
Plant's class and never runs here:

| site | class, slot | what it does on a null entry |
|---|---|---|
| `FUN_03476e20` | `PACKUN_SPIKEBALL` slot 3 | `ldp` at `entry + 0x138` — this is the crash |
| `FUN_03476dd0` | `PACKUN_SPIKEBALL` slot 55 | `*out = entry + 0x108` — **stores 0x108 as a pointer** instead of faulting |
| `FUN_034722c0` | `PACKUN_BOSSPACKUN` slot 55 | reads `entry + 0x1e8`; not reachable from this article |

`FUN_03476dd0` is why patching only the faulting site would be worse than the crash: it hands a
bogus `0x108` pointer to whatever consumes `[obj+0x88]` and fails later, somewhere else. Both of
the spikeball's slots have to be dealt with together, which the fix below does.

## Which articles have the defect, and which do not

The first pass at this worked from nearest-below symbols and left two rows unresolved, because the
weapon region has one exported symbol per several thousand bytes. It is exact now, from the game's
own dispatch table.

### The weapon class table, which makes attribution exact rather than alphabetical

`.data+0x386400` is an array of 615 pointers — one per `WEAPON_KIND_*` — each to a singleton whose
first field is its class's vtable address point. It was found by structure, not guessed: slide a
615-entry window over `.data` and take the offset where the most entries resolve to "a `.data`
pointer whose target is also a `.data` pointer". `.data+0x386400` wins with 612 of 615, and the
runner-up is the same table off by one slot. Every vtable it yields has **114 slots**.

That turns "which class is this function in" into a lookup. The spikeball's crash function is at
`.data+0x2e02b0`, which is `vtable(WEAPON_KIND_PACKUN_SPIKEBALL) + 0x18` — slot 3. And the caller
proves the slot number independently:

    0x33a07a8  ldr x0, [x19, #0x3bb0]   ; the weapon object's specializer
    0x33a07ac  ldr x8, [x0]             ; its vtable
    0x33a07b0  ldr x8, [x8, #0x18]      ; slot 3
    0x33a07b4  mov x1, x19              ; arg1 = the weapon, which is param_2 in the decomp
    0x33a07b8  blr x8
    0x33a07bc  <- the LR in the crash report

### The audit

Every one of the 114 slots of each candidate's class, each distinct target function scanned to its
`ret` for a read of `instance_ + 0x70 + donor_kind*0x38`, and for whether a `cmp` against the
donor's kind precedes it. `tools/check-article-donor-params.sh` is that scan, and it runs it from
the raw instruction stream rather than the Ghidra corpus — an independent method that reproduces
the hand result exactly.

| article | weapon kind | class vtable | verdict |
|---|---|---|---|
| `CHROM_SWORD` | `0x1b6` | `.data+0x2fdda8` | **clean.** No slot reads Chrom's entry at all. |
| `CLOUD_FUSIONSWORD` | `0x16d` | `.data+0x29edc0` | **clean.** Both Cloud-slot reads are fighter-side — `0x8db3b0` (Limit) and `FighterSpecializer_Cloud::is_camera_off_final2`. |
| `ELIGHT_ESWORD` | `0x251` | `.data+0x2ad4d8` | **clean.** The unguarded ELIGHT reads belong to `ELIGHT_EXPROSIVESHOT` and `ELIGHT_SPREADBULLET`, not the esword. |
| `KAMUI_DRAGONHAND` | `0x175` | `.data+0x2bc708` | **clean.** The one Kamui read is fighter-side (`0xb5c1a0`). |
| `GAMEWATCH_PARACHUTE` | `0x79` | `.data+0x2b36e0` | **clean.** The six unguarded Gamewatch reads are `GAMEWATCH_BOMB` (slots 3, 7, 11), `GAMEWATCH_BREATH` (7) and `GAMEWATCH_OCTOPUS` (3, 13). None is the parachute's. |
| `GAOGAEN_ROPE` | `0x1ba` | `.data+0x2b5030` | **broken.** Slots 40 and 55. See below. |
| `PACKUN_SPIKEBALL` | `0x1e8` | `.data+0x2e0298` | **broken.** Slots 3 and 55. |

**Five of the seven are clean, including both prizes.** The parachute and the dragon hand — the two
articles that ship their own `model.nuanmb`, which is the one thing a grafted bone provably cannot
reproduce — have no donor-param dependency anywhere in their classes. The clone route was never
blocked for them. It was blocked for the one article the experiment happened to pick.

Chrom's sword is also where the fighter-side guarded read lives, and it is worth keeping visible
because it is the shape that makes an article portable:

```c
uStack_38 = 0; uStack_50 = 0; uStack_48 = 0; uStack_58 = 0;    // pre-zeroed
if (*(int *)(param_2 + 0xc) == 0x1b) {                         // owner == CHROM
    lVar3 = *(long *)(FPA2::instance_ + 0x658);                //   Chrom's entry
} else {
    if (*(int *)(param_2 + 0xc) != 0x1a) goto LAB_010bb54c;    // not ROY -> fall through
    lVar3 = *(long *)(FPA2::instance_ + 0x620);                //   Roy's entry
}
```

Roy and Chrom share the sword code and dispatch on owner kind, so an owner who is neither takes the
fall-through with the locals still zero. That read is in `FUN_010bb480`, which is fighter-side
rather than in the weapon class — the class itself reads nothing, which is why the audit reports
Chrom's sword clean with zero reads of either sort.

## The fix

### 1. Point the clone at a clean article. That is the whole fix for five of seven.

Nothing is patched and nothing is reimplemented: the engine runs the donor's own code for all 114
slots. Implemented as a candidate table in `src/articles/mod.rs` with `CLONE_TARGET` naming the
row that is wired up, gated by `tools/check-article-donor-params.sh`, which refuses to agree with
a `clean` column that the dumps contradict and fails outright if `CLONE_ENABLED` is true while the
target is dirty. `install()` also refuses at runtime, so pointing `CLONE_TARGET` at the rope logs
and does nothing rather than crashing a playtest.

`sword` is the target first because it is the cheapest possible test of the path, not because the
port needs it — the graft is already algebraically identical to native for Chrom. One entry in
Chrom's whole descriptor table, `max_count` 1, and the only donor whose article assets are complete
in `extracted/` for all eight costume slots (11 model files including `model.nusrcmdlb`, 3 motion).
The parachute and dragon hand need `tools/arc-fetch.sh` first: `extracted/` has two files each.

### 2. For the spikeball and the rope, the offending slots have an engine default that is a no-op

This is the part that makes even the dirty articles recoverable, and it is the game's own answer
rather than an invention:

| slot | default | how many of the 612 weapon kinds use the default |
|---|---|---|
| 3 | `0x33be810`, whose whole body is `return;` | **412** |
| 55 | `0x33bea70`, whose whole body is `return;` | **442** |

Both are optional hooks that most of the game's weapons decline. Chrom's sword, the parachute and
the dragon hand all take the default for both. So pointing a cloned article's slot 3 and slot 55 at
those two addresses selects a configuration the engine ships for the majority of its own weapons —
two 8-byte writes, with the other 112 slots still running the donor's real native code.

What it costs, read off the overrides:

    spikeball slot 3   copies a vector of 56-byte entries from param[PACKUN]+0x138 into a static,
                       hands it to HitModule (boma+0xb0) vtable[0x78], then pokes CameraModule
                       (boma+0x60) with vtable[0x50], [0x128](0,-1), [0x130](4,-1)
    spikeball slot 55  sets [obj+0x498] and publishes {param[PACKUN]+0x108, count 1} at [obj+0x88]
    rope      slot 55  the same publish, byte-for-byte the same shape

So the ball would exist and be driven by Piranha Plant's own code, minus whatever hurtbox list and
camera setup slot 3 installs. That omits a behaviour rather than substituting wrong data, which is
the right way round: pointing the donor's param entry at Pikachu's instead would make every read
succeed with the wrong fighter's numbers, and silent wrongness is worse than an absent hurtbox.

**The hazard, which is why this is not implemented yet.** The vtable is per weapon KIND and shared,
so nopping slot 3 globally would also degrade a real Piranha Plant in the same match. The correct
form is per-INSTANCE: the weapon object holds its specializer at `+0x3bb0` and reads the vtable
through it, so an article created for our owner can be given a private copy of the vtable with two
slots defaulted while every other Ptooie keeps the class's own. smashline already has the hook
point and the owner id — `CURRENT_OWNER_KIND` and the `weapon_owner_hook` family — so this is
designable, and it is not written.

#### Written, 2026-10-12: `src/articles/neutralize.rs`, off until a dirty row is enabled

The per-instance form, with every address read out of the game through the code index
([lookup.md](../../handbook/lookup.md)) and re-checked by `tools/check-article-donor-params.sh` on
every run:

- **Where.** A hook on `main+0x339edc0`, the weapon *initialize-with-kind* virtual. Its pointer sits
  in the weapon base class's vtable in `.data` (`0x51863c8`, beside the next virtual's at
  `0x51863d0`), so pooled objects pass through it every time they are given a kind. Its arguments
  are (object, battle_object_id, kind, entry_id, …), which it forwards to the base initialiser, and
  near its end it stores `class_table[kind]` at `object + 0x3bb0`. Slot 3 is first called in the
  *next* virtual (`0x33a0490`, where `0x33a07b8` is), so patching after the original returns is in
  time. The one slot used inside initialise is 85, which the rope shares with 611 kinds.
- **What.** A class "singleton" is a stateless 8-byte object whose word points 0x18 bytes on, at
  the vtable's address point; between them are offset-to-top and a null RTTI pointer, then 114
  slots (0x390, the exact spacing of neighbouring singletons). So the private copy is an 8-byte
  object pointing at a copied group with the listed slots replaced, taken at runtime from the
  parachute's vtable, which holds the majority default in all three (slot 3: 412 kinds, slot 40:
  552, slot 55: 442).
- **Which slots.** From the class table and the gate's own scan, independently: the rope's
  unguarded readers are slot 40 (`0x34042c0`) and slot 55 (`0x3403f90`); the spikeball's are slot 3
  (`0x3476e20`) and slot 55 (`0x3476dd0`). Of the rope's 14 overridden slots, the other 12 are kept.
- **Whose.** Only a weapon of a listed kind whose owner entry's current fighter is a Pikachu,
  asked of the engine inside the hook (`FighterManager::get_fighter_entry` →
  `FighterEntry::current_fighter_id` → `sv_battle_object::kind`), so it holds even if the engine
  initialises article objects at fighter load. A real Incineroar's rope keeps its class's vtable.

The gate fails unless the listed slots cover every unguarded slot (tested: dropping 40 from the
rope's list fails with "[40] uncovered"), notes any listed slot that reads nothing of the donor's,
checks that the parachute holds the default in each, and checks the hook's two addresses against
the raw memory dumps. **Nothing changes in a shipped build**: both rows stay `enabled: false`, and
with no listed row enabled the hook is not even installed.

**Arming one is a watched-boot, single-variable experiment.** Set its row `enabled: true`, run
`tools/gen-article-assets.sh` and `tools/gen-config.sh`, build, and load. The first boot logs
`NEUTRALISED <row> … for entry N`, once per kind; a crash after that line and before the slot's
first call narrows to the private vtable, one before it to the clone itself. The rope also needs
its use: the whip spawning it and the placement and bookkeeping `FUN_00abede0` does (above), which
is the next piece of work once the article is shown to load.

### 3. The rope needs both halves, and this is only one of them

`FighterSpecializer_Gaogaen::generate_rope` (`0xabe800`) — the function `README-rat.md` calls the
linchpin of the whole Alolan Whip, and which takes nothing but a module accessor — reads
`FPA2::instance_ + 0xff8` (GAOGAEN) unguarded in its own prologue. So calling `generate_rope` with
Pikachu's accessor faults whether or not a rope article exists, and that is independent of
ADR 0004. ADR 0004 says the victim side cannot run without a rope; this says the rope's own native
code cannot run without Incineroar. Two problems, both still open.

## The module map, which fell out of reading the overrides

Needed to say what slot 3 feeds, and worth keeping: every `app::lua_bind::<X>Module__*_impl` was
disassembled and the `ldr` offset it takes off its `BattleObjectModuleAccessor` argument recorded,
six impls per module agreeing in almost every case.

    0x38 Posture   0x40 Status    0x48 Control   0x50 Work      0x58 Ground    0x60 Camera
    0x68 Kinetic   0x70 ColorBlend 0x78 Model    0x80 Physics   0x88 Motion    0x90 Stop
    0x98 Article   0xa0 Attack    0xa8 Damage    0xb0 Hit       0xb8 Combo     0xc0 Area
    0xc8 Item      0xd0 Link      0xd8 Team      0xe0 Search    0xf0 Turn      0xf8 Reflect
    0x100 Shield   0x108 Reflector 0x110 Absorber 0x118 Jostle  0x120 Catch    0x128 Cancel
    0x138 Capture  0x140 Effect   0x148 Sound    0x150 Visibility 0x158 Grab   0x160 Slope
    0x168 Shake    0x170 Slow     0x180 Shadow   0x188 MotionAnimcmd 0x190 Lua 0x198 InkPaint

This confirms the offsets the port had already inferred one at a time — `boma+0x50` WorkModule and
`boma+0xa0` AttackModule in `the-weapons.md` — and supplies the rest.

### Chrom's sword guards the read, and that is the whole difference

`FUN_010bb480`, the only `FighterParamAccessor2` access anywhere in the Chrom/Roy article region:

```c
uStack_38 = 0; uStack_50 = 0; uStack_48 = 0; uStack_58 = 0;    // pre-zeroed
if (*(int *)(param_2 + 0xc) == 0x1b) {                         // owner == CHROM
    lVar3 = *(long *)(FPA2::instance_ + 0x658);                //   Chrom's entry
} else {
    if (*(int *)(param_2 + 0xc) != 0x1a) goto LAB_010bb54c;    // not ROY -> fall through
    lVar3 = *(long *)(FPA2::instance_ + 0x620);                //   Roy's entry
}
```

Roy and Chrom share the sword code and dispatch on owner kind, so an owner who is neither takes
the fall-through with the locals still zero and the function completes. Chrom's entry (`0x658`) is
read **exactly once in all of `main`**, and that is this. Across `0x10bb000`..`0x10bd400` — 21
corpus functions, ending in two `FighterSpecializer_Roy` symbols — no other function touches the
accessor at all.

Other reasons it is the right thing to point the experiment at:

| | spikeball | chrom_sword |
|---|---|---|
| unguarded donor-slot reads | 3 | 0 |
| exported `WeaponSpecializer` class | yes | none |
| entries in the donor's descriptor table | 4 | **1**, `max_count` 1 |
| `.data` constants its `on_init` touches | shared descriptor `0x4f62778` **+ a lazy static** (`0x4f62078`, `0x52b70f0/0x52b7100`) | shared descriptor only |
| assets | fetched off the console | `extracted/fighter/chrom/{model,motion}/sword/`, **c00-c07** |

That last row also retires "every clone is c00-only, deferred so the clone experiment stays one
variable". Chrom's sword ships `motion/sword/c00`..`c07` in the local extraction, and its
`model/sword/c00` is 11 files including `model.nusrcmdlb` — the file the George mod named.

### The rope is blocked twice, and one of them is new

`FighterSpecializer_Gaogaen::generate_rope` (`0xabe800`) — the function `README-rat.md` calls the
linchpin of the whole Alolan Whip, and which takes nothing but a module accessor — reads
`FPA2::instance_ + 0xff8` (GAOGAEN) **unguarded, in its own prologue**. So calling `generate_rope`
with Pikachu's accessor faults regardless of whether a rope article exists.

And the rope's weapon code repeats the spikeball's exact shape:

```c
void FUN_03403f90(undefined8 param_1,undefined8 param_2,long param_3) {
  plVar1 = *(long **)(param_3 + 0x88);
  *plVar1 = *(long *)(FPA2::instance_ + 0xff8) + 0x108;   // identical to FUN_03476dd0
  *(undefined4 *)(plVar1 + 1) = 1;
}
```

This is independent of ADR 0004. ADR 0004 says the victim side cannot run without a rope; this
says the rope's own native code cannot run without Incineroar. Both have to be answered.

### What `generate_rope` does after that read (REA, 2026-10-12)

Read in full through REA (Ghidra 12.1.4 over `work/rea/main.elf`; body and Evidence saved as
`work/rea/findings/main-generate_rope-0xabe800.{c,evidence.json}`). The vtable slots are named
from the game's own `lua_bind` wrappers, each of which is one indirect call through its module:
`ArticleModule__generate_article_impl` uses `+0x78`, `WorkModule__set_float_impl` `+0x60`,
`get_float` `+0x58`, `on_flag` `+0x110`, `off_flag` `+0x120`. Module offsets are the map above.

1. **It is a placement routine that ends in one article spawn.** It turns two work flags off
   (`0x2100000f`, `0x200000ea`), then ray-casts against the stage through GroundModule
   (`boma+0x58`, slots `+0x630`, `+0x5b8`, `+0x620`) to find where the rope posts go. It writes
   the answer into the **current status's** floats: `0x1000007` =
   `FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLOAT_ROPE_X`, `0x1000008`, and `0x1000009` =
   `..._FLOAT_GAOGAEN_TARGET_X`.
2. The geometry comes from Incineroar's param block: `FPA2 + 0xff8` holds a `std::vector` at
   `+0xf0/+0xf8` whose first element gives an angle tolerance (`+4`, compared against
   |atan2 − 90°| + 0.01) and a height (`+8`; `<= 0` returns early). Its scalars come from
   `WorkModule::get_float(0x4f)` (name not looked up) and from PostureModule (`+0x38`, slots
   `+0xb0`, `+0x118`, `+0x68`, not named). The `+0xff8` read is the unguarded one above.
3. **The spawn is `ArticleModule::generate_article(0, false, -1)`, and it is conditional.** It
   runs only if the computed position lies between two floats read from `*DAT_052b8f00 + 0xc30`
   and `+0xc34`, a global object not identified yet. Outside that range the function returns
   with the floats written and **no rope**.
4. **Article 0 is per fighter.** For Incineroar it is `FIGHTER_GAOGAEN_GENERATE_ARTICLE_ROPE`
   (`0x0`; `ROPE2` is `0x1`). For Pikachu it is `FIGHTER_PIKACHU_GENERATE_ARTICLE_DENGEKIDAMA`.
   So even with the `+0xff8` read solved, `generate_rope` run on Pikachu's accessor would try to
   spawn Thunder Jolt, not a rope.

What that means for "no rope model". The game's own rope spawn cannot be reused as-is: it reads
Incineroar's params, and it spawns "article 0", which on Pikachu is not a rope. A rope on Pikachu
therefore needs three things:

- a **rope article that Pikachu owns**, the same clone problem as the Chrom sword above, with
  its full `cNN` file set fetched and gate 3f3 (`tools/check-articlefiles.sh`) passing;
- the **placement** (steps 1–2) done by the port, writing the same three status floats. Who reads
  them afterwards was not traced; `xrefs`/callers on the float ids is the next REA question;
- a **spawn** of that article's id, gated the way step 3 gates it.

**The two flags, named** (`vendor/main/lua-const-table.tsv`, in the context the function runs in):
`0x200000ea` = `FIGHTER_GAOGAEN_INSTANCE_WORK_ID_FLAG_SPECIAL_S_GENERATED_ROPE` (an instance flag),
and `0x2100000f` = `FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_CHECK_WALL_MOVE` (a status flag, set again
on the wall branch). `generate_rope` *clears* `GENERATED_ROPE` on entry; it never sets it.

### The other half: the rope's spawn callback, `FUN_00abede0`

Of the 90 functions in `main` that touch `0x200000ea`, only two also read Incineroar's
`FPA2 + 0xff8`, and both only clear it. The flag is **set** in 18 places, and one of them,
`FUN_00abede0`, begins at the byte after `generate_rope` ends, in the same compilation unit. It has
no direct caller anywhere in the 100,508-function corpus, so it is reached through a pointer; it
takes `(generator, owner accessor)`. What it does, with every slot named the same way as above:

1. `ModelModule` (`+0x78`) slot `+0x150` with hash40 `0x31ed91fca` = **`top`** (`tools/hash40.sh`),
   i.e. the owner's `top` joint position.
2. `WorkModule::get_float(0x1000007)` and `get_float(0x1000008)`: **exactly the two floats
   `generate_rope` wrote**. This is who reads the placement.
3. Fills a spawn descriptor that starts with **`0x4f62778`**, the same shared descriptor the
   spikeball's `on_init` touches (table above), plus the owner's facing (`PostureModule +0xb0`,
   negated), a 32-bit field at `accessor + 8` (presumably the owner's battle object id; not
   checked), and the team (`TeamModule +0x48`).
4. Creates the object through the generator's own vtable (`+0x10`, then `+0x30` for its id),
   initialises it through `get_battle_object_from_id` (`+0x30`, `+0x38`).
5. `WorkModule::set_int(rope id, 0x100000bf)`. `0x100000bf` has no Incineroar name in the constant
   table; the next one up, `0x100000C0`, is `..._INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER`. So this
   is the rope's object id, inferred from use.
6. **`WorkModule::on_flag(0x200000ea)`, which is `SPECIAL_S_GENERATED_ROPE`.**

So the game's rope pipeline is three steps:
1. `generate_rope` computes the post positions into status floats 7–9.
2. `generate_article(0)`, behind the position gate, reaches this callback.
3. The callback builds the rope at `top` from floats 7–8, records its id in int `0x100000bf`, and
   raises `GENERATED_ROPE`.

A port that spawns its own rope has the full list of what to leave behind, and the instance work
ids are Incineroar's numbering, which collides on Pikachu ([traps.md](../../handbook/traps.md)).
`0x2100000f` on Pikachu is `FIGHTER_PIKACHU_STATUS_WORK_ID_FLAG_KAMINARI_ATTACK_HIT` and
`..._SKULL_BASH_ATTACK_POWER_MODIFY`. Pikachu has no named flag at `0x200000ea`.

**The gate is the blast zone.** `app::camera::get_dead_area()` (`0x15cb9e0`) is one line:
`return *(Vector4 *)(*DAT_052b8f00 + 0xc30)`. `app::camera::check_dead_area` opens with the same
`BattleObjectWorld + 0x5c` / `+ 0x59` test and `+ 0x10` subtraction as `generate_rope`'s gate. 73
functions read those fields, including `GroundUtility::check_dead_area`, `item::is_dead_area`, and
`sv_camera_manager::dead_range`. So step 3's gate is an inlined dead-area check: the rope spawns
only if its computed position lies between the first two components of the stage's dead area. In
ordinary play that is always true, so **the gate is not why the rope is missing**.

**How `generate_article(0)` reaches it: a weapon-kind table.** The corpus cannot show this, since it
was built from `.text` alone; `work/rea/main.elf` carries the relocated `.data`. Exactly one 8-byte
word in `main`'s data holds `0xabede0`, at `.data` `0x4fbeb40`, written there by the RELATIVE
relocation at `.rodata` `0x3b059d0`. It sits in a table of 32-byte entries
`{u32 weapon kind, u32 1, callback, 0}`:

    0x4fbeb38  0x1BA  -> 0xabede0   WEAPON_KIND_GAOGAEN_ROPE (BACKLOG) -> this callback
    0x4fbeb58  0x1BD  -> 0xabf1d0
    0x4fbeb78  0x1BB  -> 0xabf040
    0x4fbeb98  0x1BC  -> ...

The neighbouring Incineroar weapon kinds' callbacks sit right after it in `.text`. The entries before
`0x4fbeb38` carry other hash40s (`virtualhat`, `virtualbeetle`), so this looks like one global
weapon-kind → spawn-callback table, not one per fighter. Found by scanning the ELF's data for the
pointer (2 s), not by a REA session.

## Two corrections to `the-weapons.md` that fall out of this

**Palutena's staff has no article path to recover.** Her descriptor table is nine entries —
GODWING, EXPLOSIVEFLAME, EXPLOSIVEFLAME_RESERVE, AUTOAIMBULLET, AUTORETICLE, REFLECTIONBOARD,
BLACKHOLE, BEAM, GATE — and there is no staff among them. The staff is part of
`palutena/model/body`, bound to her own `HaveL`, which is exactly what the merge already does. For
that one weapon the mesh graft **is** the native mechanism, and the "distinct mechanism, expected
to behave differently" note is understating it: it is a different kind of object.

**For Chrom and Cloud the article route buys nothing geometrically.** `the-weapons.md` already
proves the graft is algebraically identical to the native `HaveR` attachment for both. So Chrom's
sword is the right **test vehicle** for the article path and not the prize. The prizes are the
parachute and the dragon hand, which ship their own `model.nuanmb`, and the rope.

## Where this is left, and how to arm it

`CLONE_ENABLED` is **false** and no article assets are in the tree. Everything else is in place and
was verified with the clone armed before being put back: `tools/build.sh` compiled clean, and
checks 3b, 3f, 3g and 4l all passed with `sword` shipping 28 files across eight costume slots, every
one byte-identical to the arc's copy.

    # arm it (three commands, in this order -- the last two are what check.sh 3f asserts)
    sed -i 's/^pub const CLONE_ENABLED: bool = false;$/pub const CLONE_ENABLED: bool = true;/' \
      pikachu-moveset/plugin/src/articles/mod.rs
    bash tools/gen-article-assets.sh
    bash tools/gen-config.sh

then `tools/build.sh`, `tools/deploy.sh`, and load training mode. **Somebody has to be watching the
console**: a crash on entry is not destructive but it leaves the card unable to play the mod, which
is the whole reason this constant defaults to false.

What a pass looks like: Pikachu loads, and Chrom's Falchion rides his right hand permanently, put
there by the engine aligning the sword model's own `HaveR` to his. Note that `src/sword/mod.rs`
still shows the grafted `chrom_sword` MESH on jab, so during jab there will be two swords — that is
expected, and retiring the graft move by move is the follow-on, not part of the test.

## How to redo any of this

    # the audit, from the dumps, with its own controls
    bash tools/check-article-donor-params.sh

    # the faulting function, flat
    RAT_SYMS=vendor/main/symbols-merged.txt tools/disasm/target/release/disasm.exe \
      --flat vendor/main/rat-main-text.bin 0x3476e14 90

    # who reads the singleton at all (1877 sites)
    RAT_SYMS=vendor/main/symbols-merged.txt tools/disasm/target/release/disasm.exe \
      --flat vendor/main/rat-main-text.bin --dataxref 0x52bc3b0

    # the layout proof
    RAT_SYMS=vendor/main/symbols-merged.txt tools/disasm/target/release/disasm.exe \
      --flat vendor/main/rat-main-text.bin 0x20a8080 25

The descriptor tables were walked statically rather than logged: `get_static_fighter_data`
(`0x64b730`) is a switch whose jump table is at `.rodata 0x44bd18c`, each case an `adrp`/`add`
returning a per-kind `StaticFighterData` in `.data`; `+0x8` is `static_article_info`, which is
`{descriptors, count}`, and each descriptor is
`{weapon_id:i32, max_count:i32, on_init:ptr, on_fini:ptr, extra:u64}` — the layout
`vendor/sl_weapons.rs` declares. Module-relative addressing for the dumps comes from
`vendor/main/rat-memserve.txt`: text `0x0`, rodata `0x39c9000`, data `0x4f17000`, bss `0x52aa000`,
and all four dump sizes match that run exactly.

One thing the dump proves in passing: `0x64b730` begins `ldr x17, [pc+8]; br x17` with a literal
that is in neither module base. That is smashline's own trampoline, so `rat-main-text.bin` is a
dump of the **patched** process, and the first four instructions of any hooked function in it are
not the game's.

---

# Part 2: the visuals, and what the game's own structure says to do

Written 2026-09-17 on the instruction to complete the mod's visuals, doubt every blocker, and take
nothing in the project as the best implementation. The starting facts taken as given: every hitbox
is accurate except Ptooie's, the sword moves and Palutena's n-air look correct only facing right,
Revenge uses a non-native visual, and G&W's parachute does not show.

## The data was incomplete, and that mattered

`extracted/` held no `model.xmb` and no `model.nusrcmdlb` for cloud, elight, gamewatch, kamui or
palutena, and only one or two files of several article model directories. 1764 files are now in
`work/arc-vis` -- complete `model/` and `motion/` sets for chrom/sword, cloud/fusionsword,
elight/esword, gamewatch/parachute, kamui/dragonhand, gaogaen/rope and packun/spikeball across
**all eight costume slots**, plus palutena's body c00 and pikachu's body c00-c07.

## Nine candidate causes of the facing bug, eliminated by measurement

Each was a live suspicion in the project's notes or a reasonable guess. None survives.

| suspect | measurement | verdict |
|---|---|---|
| `model.numdlb` / `model.nusrcmdlb` missing the new meshes | both list 51 of 51 mesh/subindex pairs | complete |
| `nusrcmdlb` being a distinct file that drifted | byte-identical to `numdlb` for pikachu, palutena and chrom | it is a duplicate |
| culling spheres in `model.numshexb` | recomputed from the shipped vertices: all five match the shipped sphere and **contain** every vertex | exact |
| `entry_flags` / per-entry data | all 51 entries `[3,0]`, `unk1 (0,1,0)`, weapons identical to vanilla | uniform |
| `billboard_type` on the bind bones | all 88 bones `Disabled` | not it |
| `model.xmb` not shipped for the new mesh names | byte-identical across pikachu/body, packun/spikeball, cloud/fusionsword, elight/esword | per-MODEL settings, not per-mesh |
| triangle winding reversed, which would backface-cull in one facing | geometric normal vs vertex normals: all 51 meshes CCW-consistent, weapons included | not it |
| `model.nuhlpb` helper constraints hijacking a bind bone | 11 orient constraints, every target an `H_*` bone | `Mouth`, `HaveR`, `HaveL` untouched |
| the 511-bone limit | 88 bones, 89 with a graft | irrelevant |

## Facing is a ROTATION, which makes the reported symptom impossible as a transform error

`PostureModule` exports `rot_y_lr`, `init_rot_y_lr`, `update_rot_y_lr`, `reverse_rot_y_lr` and
`is_rot_y_lr_different_inner_lr`. Facing is a rotation about Y, not a mirror -- and a rigid rotation
of the whole model **cannot** move one mesh relative to the others. So "the Falchion is at the other
hand's position facing left" cannot be a bad bake or a bad bind. It is either a visibility artefact
or a reading of the build that was simultaneously shipping 51 foreign animation tracks.

And the geometry of the CURRENT build is provably native. Chrom's article skeleton is two bones:

    HaveR  (root)   R
    Sword  (child)  R-transpose      =>  world(Sword) = R-inverse . R = IDENTITY

so the native attachment reduces to `v . local(Sword) . world_pika(HaveR)`, and the shipped
`chrom_sword` vertices equal `source . local(Sword)` with **max deviation 0.000001 over 444
vertices**. The bake is not approximately right, it is exact.

The current build has never been seen on a screen; `next-boot.md` records it as untested.

## What the articles' own skeletons say, and it is the whole answer

Pulled and read rather than inferred:

    chrom/model/sword         2 bones   HaveR -> Sword
    cloud/model/fusionsword   7 bones   HaveR -> fusionsword1..6
    elight/model/esword      11 bones   Root -> HaveR -> Sword1 -> Sword_lightA..F2
    gamewatch/model/parachute 7 bones   Have -> Rot -> All -> Parachutea1/a2, b1/b2
    kamui/model/dragonhand    7 bones   Attach -> Hand1..4, HandD, HandU

**Every article's ROOT bone is its attachment socket** -- `HaveR`, `Have`, `Attach` -- which is
exactly the mechanism described as "the engine aligns the weapon model's own HaveR to the fighter's
HaveR". The article carries its own skeleton, its own culling data and its own `model.nuanmb`, and
the engine owns the attachment, the facing and the culling.

Three of the five weapons this port grafts ARE these articles: `WEAPON_KIND_CHROM_SWORD`,
`WEAPON_KIND_CLOUD_FUSIONSWORD` and `WEAPON_KIND_ELIGHT_ESWORD` -- and Part 1 proves all three are
clean to clone, along with `GAMEWATCH_PARACHUTE` and `KAMUI_DRAGONHAND`.

So the facing bug does not need to be solved. It needs to be made someone else's problem, and the
someone else is the engine.

## Palutena's staff is the one that cannot be an article, and it is also the one hack

She has three model directories -- `body`, `gate`, `godwing` -- and no staff among them, and there
is no staff in her `WEAPON_KIND_PALUTENA_*` list. The staff is `staff_group_VIS_O_OBJShape`
subindices 0 and 1, single-bound to `Stick`, and `Stick`'s parent chain is

    Stick -> HaveR -> HandR -> ArmR -> ShoulderR -> ClavicleR -> ClavicleC -> Bust -> Waist -> Hip -> Rot -> Trans

structurally identical to Chrom's `Sword -> HaveR`. The difference is that the staff must be
DRIVEN: it sweeps on its own track during her n-air, and a mesh baked onto `HaveR` cannot rotate
independently of the hand. That is why the port repurposed Pikachu's vanilla `Mouth` bone --
reparented from `Head` to `HaveR`, given `Stick`'s bind transform, with her `Stick` track renamed
onto it in eight animations.

It works because `Mouth` is a real runtime bone. It is still a hijack of a facial bone that vanilla
animations drive, and the fix is to give the staff what every other weapon here gets natively: its
own article model, two bones `HaveR -> Stick`, the mesh single-bound to `Stick`, and her `Stick`
animation shipped as the article's motion. That is data work, not a new mechanism --
`ssbh_data_json` round-trips every file involved.

## Revenge: the native visual, decoded, and one hash recovered

`the-weapons.md` records the hashes in `revenge_off` as compile-time baked and unrecoverable. One is
recovered, and the mechanism is now read rather than guessed.

`FighterSpecializer_Gaogaen::revenge_off` (`main 0xab86f0`), every module resolved through the offset
map below:

    WorkModule       off_flag 0x200000e8, 0x200000e3, 0x200000e4
    WorkModule       set_int(-1, 0x100000c3) ; set_int(0, 0x100000be)
    WorkModule       set_float(0.0, 0x4d)    ; set_float(1.0, 0x4e)
    ModelModule      [0x258](1.0)                           <- set_emmisive_scale, back to 1.0
    VisibilityModule set_int64(0xc8e6a844f, 0x11a164aef7)   <- group(12 chars), state(17 chars)
    AttackModule     set_power_mul_5th(1.0) ; set_reaction_mul_3rd(1.0)
    MotionAnimcmdModule [0x78](3, 0x1a264dbb63, -1)         <- a 26-char script name
    SoundModule      is_playing(0x1658651362) -> stop_se(0x1658651362, 0)

**`0x1658651362` is `se_gaogaen_special_l04`.** hash40 is `(len << 32) | crc32(name)` -- the top byte
is the length and the rest is a plain zlib crc32, verified against `param_spikeball` ->
`0xf679aaf13`. The name was found by hashing every token in every dictionary under `vendor/`,
including `vendor/param-labels/soundlabelinfo/Labels.txt`, which the earlier search did not cover.

The other three are in none of 1,293,370 hashed tokens, and a prefix sweep over `[a-z0-9_]` with up
to four free characters against 45 plausible prefixes finds nothing. They stay open -- but they are
no longer needed, because the mechanism is known: Revenge's visual is an emissive-scale change plus
a visibility-group material variant, and `special_lw_revenge_emmisive_scale_min = 0.1` / `_max = 1`
in his `vl.prc` are the two ends of it. `ModelModule::set_emmisive_scale` is bound and callable.
That is strictly more faithful than the `sys_final_aura` plus orange `FLASH` this port uses now, and
it needs no recovered name.

## The BattleObjectModuleAccessor offset map

Recovered by disassembling every exported `app::lua_bind::<X>Module__*_impl` and recording which
offset it loads off its accessor argument, six impls per module agreeing in nearly every case. This
is what made the Revenge function readable at all.

    0x38 Posture    0x40 Status     0x48 Control   0x50 Work       0x58 Ground   0x60 Camera
    0x68 Kinetic    0x70 ColorBlend 0x78 Model     0x80 Physics    0x88 Motion   0x90 Stop
    0x98 Article    0xa0 Attack     0xa8 Damage    0xb0 Hit        0xb8 Combo    0xc0 Area
    0xc8 Item       0xd0 Link       0xd8 Team      0xe0 Search     0xf0 Turn     0xf8 Reflect
    0x100 Shield    0x108 Reflector 0x110 Absorber 0x118 Jostle    0x120 Catch   0x128 Cancel
    0x138 Capture   0x140 Effect    0x148 Sound    0x150 Visibility 0x158 Grab   0x160 Slope
    0x168 Shake     0x170 Slow      0x180 Shadow   0x188 MotionAnimcmd 0x190 Lua 0x198 InkPaint

---

# Part 3: what shipped, and the Revenge mapping recovered in full

## The mesh route is closed by experiment, not by argument

Part 2 predicted the vanilla-bone build would not fix facing, on the grounds that its geometry is
already exact and facing is a rigid rotation. **It was deployed and played, and it did not.**

The build was verified rather than assumed: the deployed `model.nusktb` is byte-identical to the
local one, `model.numshb` is 1475896 on both, `model.numshexb` 4704, the plugin `.nro` 245760, and
`model.nusrcmdlb` is present -- all pulled back off the card over FTP and compared. The artifact
timeline is coherent too: `add-bones.sh` edited 18:54:52, the merged model regenerated 18:55:13,
the `.nro` built 18:56:36, committed 19:06. So the change under test really was on the console.

That is the end of the mesh route as a thing to debug. The data is right and the result is wrong,
and no further measurement of the model files can distinguish anything.

## What is wired up now

Four real articles, ids assigned by position in the candidate table:

    sword        WEAPON_KIND_CHROM_SWORD          id 0x6   chrom's jab
    esword       WEAPON_KIND_ELIGHT_ESWORD        id 0x7   mythra's dash attack + three tilts
    fusionsword  WEAPON_KIND_CLOUD_FUSIONSWORD    id 0x8   cloud's back-air
    parachute    WEAPON_KIND_GAMEWATCH_PARACHUTE  id 0x9   g&w's up-B, all four motions

`sword` and `esword` have NO AGENT SCRIPTS AT ALL -- `lua2cpp_chrom` contains only `chrom`, and
`lua2cpp_elight` has no `elight_esword` -- so they are pure models with nothing to run, which is
why they are first in the table. `fusionsword` and `parachute` have their own agents; the
parachute's is what drives the canopy, through
`VisibilityModule::set_int64(para3/para4, on/off)`, and that comes free with the clone.

`src/sword/mod.rs` generates and removes them edge-triggered against `ArticleModule::is_exist`,
which is the native pattern -- `generate_article(id, false, -1)` and
`remove_exist(id, ARTICLE_OPE_TARGET_ALL)`, both taken from real scripts. A weapon whose article
row is disabled falls back to its merged mesh, so bisecting a bad boot by flipping one row off
degrades to the behaviour that already shipped rather than showing nothing.

Palutena's staff and Corrin's Yato stay on the mesh, because neither is an article in the game.
They need an article model authored from the donor's body model -- two bones, socket plus bind
bone, exactly Chrom's shape.

716 files across four articles and eight costume slots, every one asserted byte-identical to the
arc's own copy by `check.sh` 3f, which is also why 3g's size floor now exempts article paths: the
arc's copies are legitimately tiny (`motion_list.bin` 96 bytes, the parachute's `model.nuanmb` 104,
the esword's `update.prc` 96) and byte equality rules out the truncation the floor exists to catch.

## Revenge: the mapping, not just the mechanism

Part 2 said the three unrecovered hashes were "no longer needed because the mechanism is known".
That undersold it. The mapping itself is now read out of the game.

`ModelModule::set_emmisive_scale` is vtable `0x258` on the module at `boma+0x78`, checked rather
than assumed -- `ModelModule__set_emmisive_scale_impl` (`main 0x20577a0`) is four instructions:

    ldr x0, [x0, #0x78]      ; ModelModule, matching the offset map
    ldr x8, [x0]
    ldr x1, [x8, #0x258]
    br  x1

That slot is called from **six places in the entire game**, five of them Incineroar's, and two of
those five are

    set_emmisive_scale(min + (rate / rate_max) * (max - min))

Every hash in it confirmed by hashing the name back:

| hash | length | name |
|---|---|---|
| `0x1018dfb2f4` | 16 | `param_special_lw` |
| `0x25f3eddc2e` | 37 | `special_lw_revenge_emmisive_scale_min` |
| `0x25cfe0e377` | 37 | `special_lw_revenge_emmisive_scale_max` |
| `0x1b3f8ea8af` | 27 | `special_lw_revenge_rate_max` |
| `0x207cff7d55` | 32 | `special_lw_revenge_rate_base_add` |
| `0x2642e370ac` | 38 | `special_lw_revenge_rate_attack_mul_add` |
| `0x23f8c156af` | 35 | `special_lw_revenge_rate_correct_max` |
| `0x205018f31f` | 32 | `special_lw_revenge_rate_max_team` |
| `0x1925263a0d` | 25 | `special_lw_hit_damage_mul` |
| `0x1658651362` | 22 | `se_gaogaen_special_l04` |

and `revenge_off` ends with `set_emmisive_scale(1.0)`, which is the inactive value that
`the-weapons.md` recorded as unknown.

**So the glow is not a fixed pulse: it tracks how much damage is banked**, ramping from `min` to
`max` as the stored multiplier climbs toward its cap. That is a real cue -- a maxed Revenge should
read differently from a freshly banked one -- and this port was flattening it to a constant
`FLASH(..., 0.35)`.

Implemented in `src/specials/speciallw/mod.rs`: the native call with the native value on every
active frame, 1.0 on teardown beside the `set_power_mul_5th`/`set_reaction_mul_3rd` resets it
already mirrored, and the orange tint's alpha carrying the same ramp so the cue is visible at all.
Pikachu's body cannot show emissive -- `skin_pikachu_001` carries Texture0/4/6/7 and no emissive
slot -- but the call is not inert on this model: the merged Mythra blade ships `def_light_004_emi`
and `def_light_005_emi`, so her sword really does glow with the banked rate while it is out.

Two hashes remain open, both `MotionAnimcmdModule` script names (`0x1a264dbb63`, 26 chars, and
`0x177d111458`, 23), and neither is needed for the visual.

## What still needs a boot

Nothing here has been seen on a screen. `CLONE_ENABLED` is **true** and four articles install, so
the next boot answers the question the whole of Part 1 and Part 2 was for: does a fighter load with
appended articles, and does the engine put them in his hand facing both ways.

If it crashes, the log names the last article `install()` reached and every row is bisectable by
flipping `enabled` -- turn them off from the bottom of the table up, since `sword` and `esword` are
the two with no code behind them.

---

# Part 4: the attachment mechanism, found

The articles installed and rendered, and did not follow. `ArticleModule::have` is why.

## What the probe measured

Four clones installed, ids `0x6`-`0x9`, on the verified build. Then:

    ARTICLE sword       f0..f7 lr+1/-1  id=0x6 exist=false           (all 16 samples)
    ARTICLE parachute   f1..f7 lr-1     all=(4.647,-2.699,-3.409)    identical every frame
    ARTICLE parachute   f1..f7 lr+1     all=(4.647,-2.699,-1.489)    identical every frame

A constant offset, frozen at spawn, differing only by the facing at the moment it appeared. That
is a detached object placed once -- which is exactly what `generate_article` alone produces, and
exactly what the playtest reported: "skewed to the left/right of pikachu permanently based on the
direction pikachu was facing on initial up-b instead of changing position with the drifting
animation".

One regression of ours was in that report too. `article_id` is derived from the candidate table at
COMPILE time, so it returns `Some` whether or not the clone worked -- and `src/sword/mod.rs` hid
the merged mesh on that basis. Chrom's sword answered `exist=false` on all 16 samples, so the
graft was suppressed and nothing replaced it: "visually gone from both left and right", strictly
worse than the bug it replaced. The mesh now falls back unless the article is actually live.

## The mechanism

    ArticleModule::have(boma, article_id, Hash40 joint, ArticleOperationTarget, u32, bool)

Snake's cardboard box is a held article, and all four of his appeal scripts attach it with,
verbatim from `lua2cpp_snake/snake/AppealHiL.txt`:

```rust
ArticleModule::have(agent.module_accessor, *FIGHTER_SNAKE_GENERATE_ARTICLE_CBOX,
                    Hash40::new("haver"), ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
                    *BATTLE_OBJECT_ID_INVALID as u32, false);
```

A joint name, and the joint is `haver`.

**It hid because it is a call the OWNER makes.** For Chrom it lives in his fighter code, which this
port does not run, so nothing was ever going to make it happen by itself. Every place it was
hunted for is a place it could not be:

| looked in | result |
|---|---|
| the article descriptor | `extra` is `0x0` on all 35 articles across 7 fighters |
| the article's own scripts | `lua2cpp_chrom` contains only `chrom`; `chrom_sword` has no agent |
| a `WeaponSpecializer` class | no `WeaponSpecializer_Chrom*` symbol exists |
| a kinetic type | 202 `WEAPON_KINETIC_TYPE_*` constants, none of them Chrom's |
| `param_<article>` in the donor's vl.prc | Chrom's has 13 objects, none of length 11 |
| `on_init` | a one-shot placement from the owner's position, `lr`, team and `scale` |
| `PostureModule::set_sync_constraint_joint` | 3 callers game-wide; the real one passes `hash40("top")` |

And the measurement that ruled out the article's own code entirely: **Chrom's sword class overrides
exactly 1 of its 114 vtable slots**, with slots 6/7/8's defaults all empty `return;` stubs. That
one override sets `lr`, a rotation and a visibility flag. There was never a per-frame follow in it
to find.

## What is implemented

`hold()` calls `ArticleModule::have` on the frame the article comes into existence, with a
per-weapon `socket` (currently `haver` for all four). `ATTACH` selects between that and a fallback:

    Attach::Have    (default) the engine's own call, and the engine does the whole transform
    Attach::Manual  drive the article's own PostureModule from the socket every frame
    Attach::Both    both, if `have` turns out to half-work

The fallback is worth keeping but is known to be incomplete, and the reason is worth writing down:
natively the blade sits at `v . local(bind_bone) . world(socket)`, and setting the article's
rotation to the socket's world rotation omits `local(bind_bone)` -- for Chrom that is the whole
orientation, since `HaveR -> Sword` has `Sword`'s matrix as `HaveR`'s transposed and
`world_article(Sword)` is therefore the identity. A weapon attached that way tracks the hand at a
fixed wrong angle, which is the shape of "cloud bair is newly broken with the sword straight down
through pikachu's character model in both directions". `have` should not have that problem, because
the engine is composing the transform rather than us.

## Still open

**Chrom's sword never generates.** `exist=false` on all 16 samples while the other three came up
on f1 or f2. `have` cannot help an article that does not exist, so his jab will fall back to the
merged mesh until that is understood. `max_count` is 1 for all four, so it is not a pool limit,
and his `on_init` reads only `scale`, which Pikachu has.

**The two article-id hashes** in `revenge_off`/`revenge_on` (`0x1a264dbb63`, 26 chars, and
`0x177d111458`, 23) are `MotionAnimcmdModule` script names and remain unresolved. Neither is
needed for the visual.

---

# Part 5: `ArticleModule::have`, and the two articles the game never shipped

Written 2026-09-18 in an unattended session, so nothing below has been seen on a screen except
where it says a playtest reported it.

## `have` is the attachment, and it is measured

    ArticleModule::have(boma, article_id, Hash40 joint, ArticleOperationTarget, u32, bool)

Fourteen call sites in the dumped scripts, **all fourteen passing `hash40("haver")`** -- Snake's
cardboard box, Diddy's gun, DK's barrel. It is specifically the "put this article in the hand"
call, which is why it was in none of the seven places Part 4 hunted: it is a call the OWNER makes,
so for Chrom it lives in his fighter code, and this port runs Pikachu's.

The probe measured the difference the moment it was added. Lateral offset relative to `haver`,
with the sign folded out by `lr`, so a correctly-following article reads the same both ways:

| article | before `have` | after `have` |
|---|---|---|
| parachute | 4.647 / 4.647 | **0.000 / -0.000** |
| fusionsword | -4.997 / -4.997 | **0.000 / -0.000** |

And the playtest agreed: Cloud's back-air went from "straight down through the character model in
both directions" to fully working, and the parachute from static to "animated correctly".

**`rot` is not a valid socket.** Moving the parachute's socket to `Rot` to centre it stopped it
following at all -- the playtest went from "animated correctly" back to "not animated" -- which
also tells us that "animated" in these reports means "the attachment took". Given that every one
of the game's own fourteen calls passes `haver`, the honest reading is that `have` is a hand call
and other joints are not supported. `haver` is therefore the best known socket, and the parachute
sitting out to one side is the hand being off-centre, not the attachment being wrong.

Two things not yet tried for that, in order: an ORDINARY central bone near the same height
(`bust` at y 5.865 or `hip` at 3.607 against `haver`'s 4.591), which tests whether the refusal was
about `Rot` being bone index 1 and special or about central bones generally; and `ArticleModule::
set_pos` per frame on top of `have`, which risks fighting the attachment and so is second.

## Two articles the game never shipped, authored

Palutena's staff and Corrin's Yato are meshes inside their owner's body model, with no
`WEAPON_KIND_*` and no model directory -- hers are `body`, `gate`, `godwing` and that is all. They
were the last two weapons still on the merged mesh, which is the thing that does not work facing
left. So `tools/author-article.py` builds the article they never had.

**The skeleton is two bones, and it is the native formula written as data.** Natively the staff
renders at `v . local(Stick) . world_palutena(HaveR)`, because the mesh is single-bound to `Stick`
and `Stick`'s parent is `HaveR`. An article whose skeleton is

    HaveR   identity, no parent        <- the socket `have` attaches
    Stick   local(Stick), parent 0     <- the bind bone, mesh single-bound to it

renders at `v . local(Stick) . W`. Point `W` at Pikachu's `haver` and it is the same expression
with his hand substituted for hers. Nothing is baked and nothing is recomputed.

Palutena's `Stick` turns out to be a pure translation of **(0, -2.2, 0)** from `HaveR`, with no
rotation at all, which is why this was worth doing rather than fearing: there is no orientation to
get wrong. Corrin's Yato is the same shape one level down -- his weapon mesh binds to HIS `HaveR`,
so the article is `HandR`(identity) -> `HaveR`(his own local transform) and the socket is `handr`.

| article | source | host kind | bones | meshes | vertices |
|---|---|---|---|---|---|
| `staff` | `palutena/model/body` | `WEAPON_KIND_GANON_SWORD` | 2 | 2 | 862 |
| `yato` | `kamui/model/body` | `WEAPON_KIND_GAMEWATCH_NORMAL_WEAPON` | 2 | 3 | 3634 |

**Hosted on a kind that already exists**, because the article still has to BE something. Ganon's
sword and G&W's `normal_weapon` are both script-less -- `lua2cpp_ganon` has no `ganon_sword`,
`lua2cpp_gamewatch` no `gamewatch_normal_weapon` -- and both audit CLEAN across all 114 vtable
slots, so the host contributes a working article and no behaviour at all.

The Yato's host was Steve's sword first and was moved, for a reason worth recording: smashline
loads the DONOR's fighter module for any cloned weapon kind whose agent is absent
(`get_weapon_code_dependency`, unconditional), so each distinct donor is another module loaded at
article creation -- and Steve is the heaviest fighter in the game. `GAMEWATCH_NORMAL_WEAPON` puts
the Yato on the same donor as the parachute, so six articles now load five modules instead of six,
and the one they share is one that has already loaded successfully. Their `motion/` directory and their
`model.xmb`/`model.nuhlpb` are copied verbatim so the loader finds what it expects; only the
geometry is ours.

Six articles now install: `sword=0x6 esword=0x7 fusionsword=0x8 parachute=0x9 staff=0xa yato=0xb`.

### Why this is safe to have in the tree

Round-tripping was verified before anything was written: dump `model.nusktb`, rewrite it from the
JSON, dump again, and the two JSONs are **identical**. So `ssbh_data_json` is semantically lossless
and authoring is surgery, not reconstruction.

And it is gated. `check.sh` 3f2 runs `author-article.py --verify`, which re-derives each recipe and
checks the SHIPPED bytes against it: the skeleton is exactly socket(identity) -> bind bone with the
donor's own transform, every vertex is bit-identical to the donor's body model, the bounding sphere
contains every vertex, every mesh/subindex has a modl entry naming a material that is in the
numatb, and every texture those materials name is either shipped or engine-common. 3f's
byte-identity check skips authored rows and says so, because half their files have no arc copy to
be identical to.

Engine-common textures are deliberately NOT shipped: Corrin's Yato material references
`default_Normal`, which lives at `common/shader/sfxpbs/default_normal.nutexb` -- 308 paths in the
arc index reference that name and only a handful of fighters carry a local copy.

## Chrom's sword: still refusing, and three doors now tried

`generate_article(0x6)` has answered `exist=false` on **32 consecutive samples across two boots**
while the other three come up on f1 or f2. The id mapping is not the problem: if it were off by
one, up-B would have generated a sword instead of a parachute, and a parachute is what appeared.

What is now known about it, and it is suggestive:

Mapping PostureModule's and VisibilityModule's vtable slots back to their lua_bind names -- 28 and
19 of them, recovered from the four-instruction thunks -- makes his sword's ONE override readable:

    PostureModule::set_scale(...)       slot 0x120
    PostureModule::set_rot(...)         slot 0x0e0
    VisibilityModule::set_whole(0)      slot 0x0c0   <- hides the whole model

**His sword's article hides itself**, and something in Chrom's own fighter code shows it again.
Nothing in this port would. So `hold()` now also calls `ArticleModule::set_visibility_whole(id,
true, ALL)` after `have` -- which is the game's own idiom, driven by Diddy and DK for barreljets,
gun and dkbarrel, and harmless on an article that was already visible.

And three generate paths are now tried in order, each only if the previous left nothing behind:

    generate_article(id, false, -1)
    generate_article_enable(id, false, -1)
    generate_article_have_item(id, -1, socket)       <- generates AND holds; the name is the use case

The log line says which door opened (`via=`), plus `is_generatable` and a one-shot
`ARTICLE-CENSUS` of ids 0x0..0xB with Pikachu's own six as controls. One boot distinguishes "the
engine refuses this article" from "the engine accepted it and it died immediately", which two boots
of `exist=false` could not.

## The build and the deploy, both faster

Two phases were per-file `cp` loops, and a process spawn costs about 100ms under Git Bash, so the
spawns WERE the time:

| phase | before | after |
|---|---|---|
| `mirror-costumes.sh` (525 copies) | 67s | **1-2s** |
| `gen-article-assets.sh` | 27s | **3s** |

`ARTICLE_SLOTS` defaults to `c00` for iterating -- 87 files a deploy instead of 716, because the
eight costume slots are almost entirely the same bytes (chrom/sword is 14 of 14 model files
byte-identical across all eight, elight/esword 23 of 23). `ARTICLE_SLOTS=all` for a release.
`deploy.sh --no-retire` skips the retirement pass, which is what makes shrinking the slot set
affordable: 626 files at one DELE per second is ten minutes, and an un-retired file is
shipped-but-undeclared and never loads.

One measurement worth recording because it nearly misled: the first A/B of the copy-loop fix looked
like a 249s -> 472s REGRESSION. Cargo went 24s -> 62s on byte-identical code in that run, which no
script change can cause, so it was system load. Timed in isolation the two phases are as above.
`deploy.sh`'s header already says every guess about where these scripts spend their time has been
wrong; add this one to the list.

---

# Part 6: there are TWO native attachment mechanisms, and the parachute was using the wrong one

Written 2026-09-18 in the same unattended session as Part 5, after finding a source this project had
not been reading. `vendor/ssbu-status-all/ghidra_scripts/lua2cpp_*.nro/` is not 284 NRO files, it is
284 DIRECTORIES of fully symbolised decompiled status code -- every fighter's C++ status functions
with `app::lua_bind::*_impl` call names intact. `vendor/SSBU-Dumped-Scripts` only ever had the acmd
half, which is why four parts of this document could not find calls that were sitting in plain text
on disk. Everything below is read out of that tree.

## The correction that matters: `have` is not the mechanism, it is *a* mechanism

Part 5 recorded "fourteen call sites in the dumped scripts, all fourteen passing `haver`". That
count was occurrences, not sites. Deduplicated, the entire script corpus contains **one** `have`
call:

    ArticleModule::have(BOMA, *FIGHTER_SNAKE_GENERATE_ARTICLE_CBOX, Hash40::new("haver"),
                        ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL),
                        *BATTLE_OBJECT_ID_INVALID as u32, false);

Snake's cardboard box, and nothing else. Not Ganon's sword, not Link's parasail, not Meta Knight's
mantle, not Samus's gun, not Peach's parasol -- none of the held props use it. So `have` cannot be
the general attachment, and the question Part 4 asked has a second answer nobody had looked for.

## Which mechanism an article uses is decided by whether it ships animations

Count the `.nuanmb` files in each donor's article motion directory. The answer is in the file list:

| article | motions it ships | owns an animation for the move? |
|---|---|---|
| `chrom/motion/sword` | `entryl`, `entryr` | no -- entry only |
| `cloud/motion/fusionsword` | **none at all**, just `motion_list.bin` | no |
| `elight/motion/esword` | `specials1/s2`, `specialsmovel/r`, `close/open/toclose/toopen` | no |
| `gamewatch/motion/parachute` | `specialhi`, `specialairhi`, `specialhiopen`, `specialhiclose` | **yes, all four** |

The first three have no animation for the move that shows them. They are RIGID: the engine keeps
them in the hand, and `have` is how you ask for that. This is not a guess -- it is why adding `have`
took Cloud's back-air from "sword straight down through the character model in both directions" to
fully working, in both facings, in one round.

The parachute owns an animation for **every one of its four trigger motions**, and it is the one
`have` was wrong for.

## What G&W actually does, verbatim

`lua2cpp_gamewatch/gamewatch/status/SpecialHi/main.c`:

    generate_article(PARACHUTE, false, -1);
    set_visibility_whole(PARACHUTE, false, 0);
    ...
    change_motion(PARACHUTE, H, false, -1.0);                             <- the ARTICLE's motion
    MotionModule::change_motion(H, 0.0, 1.0, false, 0.0, false, false);   <- the FIGHTER's motion

and `SpecialHiFall/main.c`, the drifting status, does `is_exist` -> generate if absent -> the same
pair again with a new `H`. The hashes are literals in the decompilation, and all three resolve by
recomputing hash40 over the name:

| literal | name | length | where |
|---|---|---|---|
| `0xa28f17495` | `special_hi` | 10 = `0xa` | up-B from the ground |
| `0xed8a31e01` | `special_air_hi` | 14 = `0xe` | up-B from the air |
| `0xfedb5db59` | `special_hi_open` | 15 = `0xf` | the drift status |

**The article and the fighter are handed the same motion hash on the same frame.** That is the
attachment: both play the same-named animation from the same frame, and the canopy's position above
the body lives in the animation's own root track, authored in the owner's local space. Nothing is
constrained to a joint -- which is why moving the socket around never worked, and why `rot` was not
"a joint `have` refuses" so much as a wrong answer to a question about the wrong mechanism.

It also explains the offset exactly. `have` glued the root to `haver`, and `haver` is ~4.65 units
FORWARD along the facing axis. The playtest's "offset to the right when facing right and to the left
when facing left" is a forward offset seen from two sides, not a lateral one.

And the flag was never the problem: the native parachute is generated with **`false, -1`**, which is
what this port already passed. The 22 script call sites that pass `true` -- Ganon's sword, Link's
parasail, Meta Knight's mantle, both Samus guns, Peach's parasol, Fox's blaster, both slingshots,
Toon Link's baton, Young Link's milk -- are all held props against 303 `false` projectiles, so the
flag plainly means something and is worth remembering. It is just not what holds the parachute,
because the parachute does not use it.

### What shipped for it

`src/sword/mod.rs` gains `Weapon::own_motion`, true on the parachute row and nothing else. That row
now gets `set_visibility_whole(true)` on generate and then, **edge-triggered on the fighter's motion
changing**, `ArticleModule::change_motion(id, Hash40::new_raw(motion), false, -1.0)`. It gets no
`have` at all.

The edge matters as much as the call. Native changes the article's motion once per status entry;
calling it every frame would restart the animation every frame and leave the canopy frozen on frame
0, which is indistinguishable from "not animated" -- the exact symptom the last two rounds produced
by two other means. `ARTICLE_MOTION[wi]` holds the last hash applied and is zeroed when the article
stops being live.

Pikachu's own up-B motions are named `special_hi` and `special_air_hi`, the same as G&W's, so the
fighter's own hash resolves in the article's motion list. That is the same coincidence the native
code relies on, not a mapping this port invents.

## Chrom's sword: the lifetime, which is the other thing `entryl`/`entryr` is telling us

`entryl` and `entryr` are the ENTRY motions. Chrom's sword is created during his entry sequence and
then simply exists, held, for every frame of the match -- which is why no `generate_article` for it
appears in any script OR anywhere in `lua2cpp_chrom`, whose whole decompiled status tree contains not
one article reference. There is nothing to generate at move time because it was never gone.

This port has been doing the opposite: `generate_article` on the frame the jab starts, `remove_exist`
on the frame it ends. The correlation is hard to ignore -- the three articles whose donors DO
generate per-move come up on frame 1, and the one whose donor never does is the one that has answered
`exist=false` on 32 consecutive samples across two boots.

So `sword` now gets `Weapon::persist`: generated whenever it does not exist regardless of whether the
move is running, never removed, and shown and hidden with `set_visibility_whole(want)` -- G&W's own
idiom for the parachute, target `ARTICLE_OPE_TARGET_ALL`.

### And the same test, run across all four, makes it a controlled comparison

The decompiled status trees answer "does the donor generate this article per-move" for each of them,
and the answer lines up with what the playtests reported without a single exception:

| article | what its donor's status code does | our per-move generate |
|---|---|---|
| `cloud/fusionsword` | 4 sites by `generate_article`/`remove_exist` | **works** |
| `gamewatch/parachute` | generated in `SpecialHi` and again in `SpecialHiFall` | **works** |
| `elight/esword` | **one** site in her entire tree, and it is an `is_exist` QUERY | broken |
| `chrom/sword` | **zero** article references anywhere in his entire tree | never generates |

Four for four, with a positive control: where the donor generates per-move, this port's per-move
generate works; where the donor never does, it fails. That is a prediction rather than a story
fitted after the fact, so `esword` gets `persist` too.

`fusionsword` deliberately does NOT, even though every rigid held article in the game is persistent
and it therefore should be eventually. It currently works, and changing the lifetime of a weapon that
works, unobserved, to fix one that does not is how this project has lost rounds before. `staff` and
`yato` are hosted on kinds that ARE generated per-move -- `GANON_SWORD` at 18 sites and
`GAMEWATCH_NORMAL_WEAPON` at 26 -- so by the same correlation they should come up on the per-move
path, and they are left on it. That hosts chosen for being script-less and audit-clean turn out to
also be per-move generatable is luck, but it is checkable luck.

All three outcomes are informative and at most one is a regression: generation still fails and
nothing changes; it succeeds and the move is fixed; or it succeeds and visibility does not hold, in
which case Pikachu carries Falchion and Mythra's sword all match -- ugly, but proof that generation
works outside the move, which two boots of `exist=false` could not give. The generate log is
throttled to the first four attempts per weapon, because a persistent article that keeps failing
retries every frame.

## Two things ruled out on the way, both worth not re-testing

**The cloned descriptor is complete.** `vendor/sl_weapons.rs`'s `get_static_fighter_data` hook copies
the donor's whole `ArticleDescriptor` -- `{weapon_id, max_count, on_init, on_fini, extra}` -- by
value, and `panic!`s if the donor does not have the entry. So `max_count` and `on_init` do carry, and
since nothing panics the entry IS being appended. A truncated descriptor was a good theory and it is
dead.

**The mesh graft cannot be facing-dependent, so the fallback's bug is not in the binding.** Dumping
`pikachu/model/body/c00/model.numshb` shows every grafted weapon is SINGLE-BOUND -- zero bone
influences and a `parent_bone_name` -- and all five parents resolve in Pikachu's 88-bone skeleton
(`chrom_sword`, `cloud_sword`, `mythra_sword` and `corrin_sword` on `HaveR`, `palutena_staff` on
`Mouth`). A single-bound mesh renders at `v . world(parent)`, and facing is a rigid Y rotation of the
whole skeleton, so the result is identical relative to the body in both facings by construction.
Whatever "only correct facing right" is, it is not the bind -- and the article path makes it moot,
which Cloud's back-air working in both directions already demonstrates.

----

# Part 7: Revenge's native visual, recovered verbatim -- and why it stays unused

The instruction listed four defects and this is the fourth: "Revenge uses a different visual than
the native version to show that he has a revenge stack". It was worth re-opening under "doubt every
blocker", and the honest outcome is that the blocker held. What is new is that the native visual is
now written down exactly, and that the blocker is now PROVEN rather than assumed.

## What the game does, verbatim

`lua2cpp_gaogaen/gaogaen/SpecialLwRevenge.txt`, the whole function:

    EFFECT_OFF_KIND(agent, Hash40::new("gaogaen_revenge_aura"), false, false);
    EFFECT_FOLLOW(agent, Hash40::new("gaogaen_revenge_aura"), Hash40::new("shoulderr"), 0, 0, 0, 0, 0, 0, 1, false);
    EFFECT_FOLLOW(agent, Hash40::new("gaogaen_revenge_aura"), Hash40::new("shoulderl"), 0, 0, 0, 0, 0, 0, 1, false);
    EFFECT_FOLLOW(agent, Hash40::new("gaogaen_revenge_aura"), Hash40::new("kneer"),     0, 0, 0, 0, 0, 0, 1, false);
    EFFECT_FOLLOW(agent, Hash40::new("gaogaen_revenge_aura"), Hash40::new("kneel"),     0, 0, 0, 0, 0, 0, 1, false);
    EFFECT_FOLLOW(agent, Hash40::new("gaogaen_revenge_aura"), Hash40::new("feelerca"),  0, 0, 3, 0, 0, 0, 1, false);

and `SpecialLwRevengeEnd.txt` is one line, the same `EFFECT_OFF_KIND`. So the native indicator is
**one effect followed on five joints**, plus the emissive scale on the model.

## What this port already ships, and it is closer than the report suggests

Three of the four pieces are already the game's own, and none of this is new work from this session
-- it is recorded here because Part 3 described the emissive and not the rest:

| piece | shipped | native? |
|---|---|---|
| emissive scale | `min + rate/rate_max * (max - min)` from Incineroar's `param_special_lw`, 1.0 on release | **yes, exactly** |
| the joint list | `shoulderr`, `shoulderl`, `kneer`, `kneel` | **yes** -- 4 of the native 5 |
| the aura itself | `sys_final_aura` substituted for `gaogaen_revenge_aura` | no, and cannot be |
| the colour cue | `FLASH(1.0, 0.45, 0.0, emissive)` orange | no -- an addition |

`feelerca` is the only joint that does not transfer; it is Incineroar's flame whiskers and Pikachu
has no counterpart. So the structure of the native effect is already reproduced, on the native
bones, with one substituted archive.

## The blocker is ADR 0002, and it is a decision rather than a limitation

`gaogaen_revenge_aura` lives in `effect/fighter/gaogaen/ef_gaogaen.eff` and nowhere else. The arc
index has 329 `.eff` files: one per fighter, plus `effect/system/common/ef_common.eff` and
`effect/stage/stgcommon/ef_stgcommon.eff`. A fighter loads its own bank by name, so referencing
Incineroar's from Pikachu's code is exactly what creates a dependency on the CSK Collection --
which the brief forbids, and which `docs/adr/0002-sys-effects-only.md` decided against on purpose.

ADR 0002 also predicts this investigation almost word for word: "a future reader will be tempted to
'fix' it by wiring up the real effect names. Doing so silently reintroduces the CSK dependency the
brief forbids." That reader was this session. The ADR was right and nothing here overrides it; a
CSK-requiring variant with full source effects is already tracked in `BACKLOG.md` as a separate
deliverable, and that is where these five calls belong.

**What this session adds is the proof that there is no runtime escape.** All 61 `EffectModule__*`
entry points were enumerated from the symbol table -- `req`, `req_follow`, `req_on_joint`,
`req_continual`, `req_common`, `req_time_follow`, `kill_kind`, `detach_kind` and the rest. Every one
of them requests or cancels an effect. Not one loads a bank: there is no `preload`, no `load`, and
no bank argument anywhere in the API. So the constraint cannot be worked around from the plugin at
all, and the only route is shipping a merged `ef_pikachu.eff` -- which is precisely the CSK-shaped
deliverable ADR 0002 excludes, not a fix to this one.

That is worth having measured. "Fighter-prefixed archives are not resident" was the ADR's premise;
it is now also a closed question about the API surface, so the next reader tempted to re-open it can
stop here.

## One thing deliberately not changed

Pikachu's own bank contains `pikachu_rocket_aura` and `pikachu_rocket_aura_max`, which are following
auras from his Final Smash and would need no substitution at all. They are Volt Tackle's electric
yellow rather than Revenge's orange, so against `sys_final_aura` plus an orange FLASH they trade one
wrong axis for another, and the `_max` variant's obvious appeal -- a second tier keyed to the stack
-- is not a thing the native move has. Left alone; this is an aesthetic call on a move whose
mechanical half is already exact, and it is the user's to make.

---

# Part 8: the other three script categories, audited across all 55 ported moves

The instruction was to complete the visuals ENTIRELY, and the four reported defects are only the
ones that were noticed. A motion entry names four scripts -- `game_`, `effect_`, `expression_`,
`sound_` -- and this port installs two of them. This is what is in the other two, measured rather
than assumed.

    game_        73 installed
    effect_      67 installed
    expression_   0
    sound_        0

Zero of each is inherited from the reference recipe rather than decided: `docs/reference-recipe.md`
records that the George moveset has "any `sound_*` or `expression_*` script" among its notable
absences, and this port followed it. That is a description of a different mod, so it was worth
checking what the absence actually costs here.

## The good news, and it was the thing worth being afraid of

Gate 3k exists because a synthesised motion once ran Pikachu's own `expression_specialairhi1`,
which calls `VisibilityModule::set_whole(false)` -- the up-B was logically correct and measured
working while being hidden by his own script. 3k guards synthesised motions and deliberately
EXEMPTS repointed ones, so every ported move that lands on a motion Pikachu already has still runs
his expression script, unguarded.

All 77 ported motions were checked against Pikachu's own 335 script files for
`VisibilityModule::*`, `set_mesh_visibility`, `set_attach_item_visibility` and
`ModelModule::set_scale`. **33 resolve to a real Pikachu `expression_` function and all 33 are
clean.** The other 44 have no such function, and those are the synthesised ones 3k already covers.
So the bug class that hid the up-B is not live anywhere else, and the expression gap is not costing
a visible defect today.

## What the gap does cost, by call count across the 50 donor scripts

| call | count | what dropping it costs |
|---|---|---|
| `ControlModule::set_rumble` | 59 | controller rumble, invisible |
| `RUMBLE_HIT` | 38 | same |
| `ItemModule::set_have_item_visibility` | 32 | a held item stays visible through the move |
| `WorkModule::on_flag` / `off_flag` / `is_flag` | 22 | covered by gate 4f, which already checks that a flag the status code reads has an ACMD that sets it |
| `AttackModule::set_attack_reference_joint_id` | 7 | the joint and axis a hitbox's DIRECTION is judged from |
| `VisibilityModule::set_int64` | 3 | donor-specific mesh groups, e.g. G&W's `head` -> `head_close`; not portable, Pikachu has no such group |

97 of the 161 calls are rumble. The port is not missing visuals there, it is missing haptics.

## Two findings deliberately not acted on

**`set_attack_reference_joint_id`, on six moves.** `jab_chrom`, `utilt_falco` (twice, `handr` and
`handl`), `usmash_fox` (`toer`), `getup_up_bowser` (`armr`), `getup_down_bowser` (`arml`) and
`trip_bowser` (`armr`). This is not cosmetic -- it sets the reference joint and
`AttackDirectionAxis` a hitbox's direction is resolved against, so it can move knockback angles.

It is left alone for two reasons. Hitbox behaviour is the part of this mod the brief says is already
accurate, and changing it unobserved to fix something nobody has reported is the same mistake as
changing `fusionsword`'s lifetime would be. And `jab_chrom`'s reference joint is
`Hash40::new("sword1")` -- a bone in CHROM's body model. Pikachu has no `Sword1`, so that one could
not be transcribed as written even if it were wanted; it would need a decision about which of
Pikachu's bones stands in, which is exactly the kind of guess this document exists to avoid.

**`ItemModule::set_have_item_visibility`, 32 calls.** Genuinely visible: hold an item, use a ported
move, and the donor would hide it while this port does not. It is also the riskiest thing in the
table to transcribe, because the call is a PAIR -- hidden on one frame and restored on another --
and the restore only happens if the move runs to completion the way the donor's status flow
guarantees. A ported move that is interrupted between the two would leave the item permanently
invisible, which is a worse bug than the one being fixed, and it is not a bug a single playtest
would reliably surface. Transcribing it wants the interrupt paths worked out first.

## What this means for "entirely"

Within ADR 0002, the visual work that remains is the article work in Parts 6 and 7, all of which
needs a boot. Nothing else in the four script categories is producing a visible defect: the effect
scripts are installed for 67 of 73 moves, the six without are four project-authored landing-lag
scripts that correctly fall through to Pikachu's own generic landing smoke plus the two Revenge
stance starts whose visual is driven from the status side in `src/specials/speciallw` instead, and
the expression scripts are clean where they run at all.

---

# Part 9: why a right-hand weapon cannot look the same in both facings on Pikachu's rig

This is the most important thing in this document for reading the next playtest, because it predicts
that part of the original complaint will NOT be fixed by the article work, and says why.

## The measurement

Pikachu's bind-pose world positions, accumulated through the parent chain from his shipped
`model.nusktb`:

| bone | x | y | z |
|---|---|---|---|
| `HaveR` | **-3.958** | 4.591 | 2.778 |
| `HaveL` | **+3.959** | 4.578 | 2.772 |
| `ShoulderR` | -2.190 | 5.317 | 1.405 |
| `ShoulderL` | +2.190 | 5.317 | 1.406 |
| `Bust` | 0.000 | 5.865 | -0.694 |
| `Hip` | 0.000 | 3.607 | -0.878 |
| `Trans` | 0.000 | 0.000 | 0.000 |

The L/R pairs mirror in **x** and share their **z**, and the body bones sit at x = 0. So **x is the
lateral axis** -- the one the side-on camera reads as DEPTH -- and z is forward/back.

Facing is a rigid 180-degree rotation about Y (`PostureModule::rot_y_lr`), which maps
(x, z) -> (-x, -z). So `HaveR` moves from x = -3.958 to x = +3.958 between facings: **an eight-unit
swing in camera depth, on a character whose body is about two units deep.**

A weapon held in the right hand is therefore roughly four units in FRONT of Pikachu's silhouette in
one facing and four units BEHIND it in the other. Not mirrored -- occluded. That is a complete
mechanical explanation for "the sword moves only look correct when Pikachu is facing right", and it
is geometry, not a bug.

## Why the article path cannot fix it

`ArticleModule::have` attaches the article to the socket it is given, and every rigid row gives it
`haver` -- which is the same joint the merged mesh is single-bound to. Part 6 established that a
single-bound mesh renders at `v . world(parent)` and is facing-independent by construction; an
article held at the same joint renders at the same place. **So the articles and the merged meshes
are the same expression, and the articles will look the same as what is on screen today.**

The staff makes this concrete and checkable. Its merged mesh is single-bound to `Mouth`, an
otherwise unused bone the port RE-PARENTED to `HaveR` and loaded with Palutena's own `Stick` bind
transform -- a graft that adds no bone, and one `check.sh` already verifies in both halves ("Mouth
is parented to HaveR, as Stick is in palutena" / "Mouth carries palutena's Stick bind transform").
Pikachu's face is untouched: his mouth geometry binds to `Mouth_group` under `Face`, and `Mouth`
has no children and no other mesh. So the merged staff already renders at
`v . local(Stick) . world(HaveR)`, and `tools/author-article.py` builds an article that renders at
`v . local(Stick) . W` with `W` pointed at `haver`. Those are the same expression written two ways.

**Cloud's back-air is not a counterexample.** It went from "sword straight down through the
character model in both directions" to "fully working" when `have` was added, but that was fixing a
DETACHED article -- an object placed once and left in world space. It is the difference between
attached and not, not between one facing and the other.

## What that leaves

Against the four defects in the original instruction:

- **The parachute** is a real mechanism bug and Part 6 fixes it natively.
- **Chrom's and Mythra's swords not generating** is a real bug and Part 6 predicts the cause.
- **Revenge** is ADR 0002 and Part 7 closes it.
- **"Sword moves only look correct facing right"** is, on this evidence, the rig. Nothing in the
  attachment path addresses it, and the next boot should show the swords looking as they do now
  once they generate. If they do, that is the articles SUCCEEDING.

## The fix that looks obvious and is wrong

Mirror the socket with facing -- `haver` when facing one way, `havel` the other -- and the weapon
would sit identically against the silhouette both ways. `HaveL` is an exact mirror (x +3.959 against
-3.958, y and z within 0.013), so the geometry would work perfectly.

**It would look worse.** The ported animation swings his RIGHT arm, because it is the donor's own
animation for a right-handed swing. Attaching the weapon to `havel` would hang it off the hand that
is not moving while the empty right hand performed the attack. So this is not a knob worth adding,
and it is written down here so it does not get tried twice.

If the asymmetry is to be addressed at all, the remaining lever is the animation rather than the
attachment: the swing itself would have to be authored to keep the weapon clear of the silhouette in
both facings, which is a retarget and a much larger piece of work than anything in this document.
That is a decision, not a fix, and it wants the next boot's screenshots first -- the articles have
never all been on screen at once, and "how bad is it actually, now that the weapon is the real
model" is not a question this document can answer offline.

---

# Part 10: the spikeball crash, disassembled -- and the one-match experiment nobody has run

Part 1 called the null slot "the one load-bearing claim here that is not decompiled". It is
decompiled now, and it was right. What is new is what the faulting code wants, and that the cheapest
test of the whole theory is a playtest with no code change at all.

## The crash, instruction by instruction

`FUN_03476e20`, `WEAPON_KIND_PACKUN_SPIKEBALL` slot 3:

    adrp x8, ...            \
    ldr  x8, [x8, #0x3b0]    > x8 = *(FighterParamAccessor2 singleton), bss 0x52bc3b0
    ldr  x20, [x8, #0x1228]  > x20 = the per-kind entry
    ldr  x19, [x1, #0x20]
    ldr  x0,  [x19, #0xb0]
    ldp  x8, x9, [x20, #0x138]   <- FAULTS, x20 is null

`0x1228` is `0x70 + 81 * 0x38` exactly, so the table base, the 56-byte stride and **fighter kind 81
constant-folded into the offset** are all confirmed as Part 1 described them. Pikachu's kind never
enters the instruction; there is no register to redirect.

A wrong turn worth recording, because the next reader will take it too. Ghidra renders these reads
as `*(long **)(__ZN3lib9Singleton...instance_E + iVar1 * 0x38 + 0x70)`, which reads as though the
table were a static bss array adjacent to the singleton POINTER -- and the bss there really is a
dense run of singleton pointers, with `FighterBayonettaFinalModule` eight bytes later, so the
reading is plausible and it is wrong. The disassembly shows `ldr x8, [x8, #0x3b0]` DEREFERENCING the
singleton first. The decompiler folded the load away. Searching for stores "through the singleton
symbol" therefore could not have found the writer even if it existed, and neither can searching the
bss range; the writer stores through an instance pointer held in a register.

## What the faulting code actually wants, and why a null-guard is not enough

`entry + 0x138` is a `std::vector` of 56-byte records -- the code takes `begin`/`end` with one `ldp`,
computes `(end - begin) >> 3` and multiplies by `0x6db6db6db6db6db7`, the modular inverse of 7, which
is division by 56. It then runs a **do/while** that copies every record field by field (offsets 0x14,
0x18, 0x20, 0x28, 0x30, 0x34) into a buffer, and hands that buffer to a virtual call.

So handing it an empty vector does not help: `subs x9, x9, #1; b.ne` executes at least once, and a
count of zero underflows to 2^64 iterations. Fabricating the entry means supplying **Piranha Plant's
real per-kind param records**, not a zero-filled stub. That is a materially harder fix than the
"populate one slot" it looks like from Part 1, and it is worth knowing before anyone starts.

## The experiment: put Piranha Plant in the match

The entry is null because nothing populated it, and the thing that populates it is Piranha Plant
being loaded. **So with a real Piranha Plant in the match, that slot should hold the real data and
this code should run as designed.**

Nothing in the boot record tests this. The 0-for-10 crash record was ten matches against whatever
the CPU happened to be; there is no reason to think Piranha Plant was ever one of them, and the
theory predicts the crash is conditional on his absence.

This costs one playtest and no code change:

1. Set `enabled: true` on a Ptooie row in `src/articles/mod.rs` (there is none today -- the audit
   refuses it, so this needs the row plus a bypass of `tools/check-article-donor-params.sh`, which
   is the one deliberate step).
2. Pikachu versus **Piranha Plant**, and use Ptooie.

Three outcomes, all worth having:

- **It works.** Then the article is fine and the blocker is entirely "the donor must be loaded",
  which reframes the fix completely -- the question becomes how to get that data resident, not how
  to dodge a null read.
- **It crashes identically.** Then the entry is not populated by the fighter merely being present,
  the null-slot theory is incomplete, and the register dump from that boot says where it actually
  came from.
- **It crashes somewhere else.** Then slot 3 is fixed by the donor's presence and the SECOND site
  (`FUN_03476dd0`, slot 55, which stores `entry + 0x108` as a pointer rather than faulting) is next,
  exactly as Part 1 predicted it would be.

This is deliberately not wired up in this build. It needs a row that the donor-param audit is
designed to refuse, and switching that audit off is not something to do to a build the user is about
to test for six other things -- it would put a known crasher in the same boot as the article work
Parts 6 and 9 are waiting on, and a crash on entry would tell us nothing about either.

## And the authored-article route does not rescue it

Parts 5's technique -- host geometry on a clean weapon kind and let the port drive it -- was the
obvious thing to try here, since `fighter/packun/model/spikeball` is a real article model and is
already in `work/arc-vis`. It does not work, for a reason worth stating: hosting on a clean kind
gets the BALL and none of its behaviour. The staff and the Yato are static props held in a hand, so
"no behaviour" is the correct behaviour for them. Ptooie's ball is launched, held aloft on the
breath, falls, bounces and carries a moving hitbox, and all of that lives in the class that crashes.
Hosting it elsewhere would mean re-implementing the move by hand, which is the opposite of this
project's whole premise.

## The up-B crash of 2026-09-18, root-caused from the crash report

The build stamped `2026-09-18T09:01:11Z` (md5 `59551cc1`) crashed the game on G&W's up-B. Two things
were new in it -- `change_motion` on the parachute (Part 6) and `persist` on two sword rows (also
Part 6) -- and the obvious suspect was the wrong one.

### What the report says

`/atmosphere/crash_reports/01789743093_01006a800016e000.log`:

    Type:            Data Abort
    Fault Address:   0000000000000000
    Crashed Thread:  MainThread
    Address:         00000078d47430d0        <- in NO loaded module
    X[08]:           00000078d47430d0        <- and the same value is in x8

    Stack Trace:
      [00] cross2_Release.nss + 0x33a17b8
      [01] cross2_Release.nss + 0xa804dc
      [02] cross2_Release.nss + 0x3a7000
      [03] cross2_Release.nss + 0x3d4114
      [04] cross2_Release.nss + 0x3d41a8
      [05] rat_of_doom + 0x37f4              <- our plugin

`cross2_Release.nss` offsets are module-relative and this project's dumps are based at zero, so
every frame can be disassembled directly. Two of them settle it:

    0x3d4128    bl  GenerateArticleFromCopyAbilities      (reached from frame [03])

    0x33a17b8   ldr x0, [x8, #0x58]      ; x0 = obj->0x58
                ldr x8, [x0]             ; x8 = its vtable
                ldr x8, [x8, #0x6e0]     ; slot 0x6e0
                blr x8                   ; -> 0x78d47430d0, which is in no module

So the call went from our plugin into **article generation**, and died dispatching through a vtable
that was not a vtable. It is not `change_motion`; that function is nowhere in the trace.

It is also not the signature `tools/pull-crash.sh`'s header describes. That header says a
`TaskWorker2` thread with fault address 0 means a null deref in smashline's fighter init -- the
article-clone signature. This is `MainThread` with fault address 0, a third combination, and it is
generation at MOVE time rather than init time.

### Why `persist` is the cause and not the generate calls themselves

`persist` moved generation from "on the frame the move starts" to "every frame the article does not
exist, for the whole match, including entry and respawn". Chrom's sword never succeeds -- 32
consecutive `exist=false` samples across two boots -- so with `persist` set, all THREE doors
(`generate_article`, `generate_article_enable`, `generate_article_have_item`) fire on every frame of
the match for that row, and Mythra's does the same.

Those same three failing calls ran for many boots without crashing when they only ran during a jab.
The difference is exposure, not the calls: generation is being attempted in states where it is not
safe, and the object whose vtable is garbage at `0x33a17b8` is the result.

`persist` is `false` on both rows again. The reading that motivated it still stands --
`ELIGHT_GENERATE_ARTICLE_ESWORD` appears once in Mythra's whole status tree and it is an `is_exist`,
and Chrom's tree has no article reference at all, so the native lifetime really is "created at
entry, held for the match". Reproducing that by retrying a failing generate every frame was the
wrong way to get there, and the crash report says so.

### What this means for `change_motion`

Nothing, either way. It was reverted before the report was read, on the reasoning that up-B is the
only place it runs; the report shows it was never in the trace. Part 6's reading of the game is
unaffected, and `own_motion` stays `false` for now only because the next boot should change one
thing at a time -- with `persist` gone, re-enabling it is the obvious next experiment rather than a
risk.

The one-frame deferral added while waiting for the report (gate the call on `exists`, read BEFORE
the generate block, so the motion is applied the frame AFTER the article appears) is kept. It was a
guess at the wrong problem, but it is a strictly safer shape for when `own_motion` comes back on and
it costs one frame of default motion.

## The Yato on Pikachu's feet, and the third socket `have` has refused

The same boot put Corrin's sword at Pikachu's feet during up-air. That is what an article that
EXISTS but never attached looks like -- it sits at the owner's origin -- and it is progress, because
it means the authored `yato` article is now generating where it used to fall back to the merged
mesh.

`have` had refused the socket. The row asked for `handr`, and the authored model was
`HandR`(identity) -> `HaveR`(Corrin's own local transform) so that the two-bone form would hold:

    v . local_kamui(HaveR) . world_pika(HandR)

which only equals Corrin's own `v . world_kamui(HaveR)` if Pikachu's HandR-to-HaveR step matches
Corrin's. It does not; they are different rigs. But the geometry was moot, because the attachment
never happened at all.

**`haver` is now treated as the only socket `have` accepts.** Three have been tried and two refused
-- `rot` ("back to not animated") and now `handr` (the feet) -- against fourteen native call sites
that every one of them passes `haver`. That is no longer a hypothesis worth re-testing.

So the Yato's socket is `haver`, and `tools/author-article.py` grew a **one-bone form** to match:
when the donor's mesh binds directly to the joint the engine will attach, there is no intermediate
transform to carry and a single identity bone reproduces `v . world(HaveR)` exactly. The staff keeps
the two-bone form, because hers really does bind to `Stick`, a child of `HaveR`, and that
intermediate transform is the whole point of it. `--verify` and gate 3f2 check whichever form the
recipe declares:

    ok  staff  2 bone(s), 2 mesh object(s),  862 vertices, all bit-identical to palutena/model/body
    ok  yato   1 bone(s), 3 mesh object(s), 3634 vertices, all bit-identical to kamui/model/body

## And the costume slots, which were a real defect either way

`ARTICLE_SLOTS` defaulted to `c00`, and a cloned article resolves under the OWNER's costume slot, so
on any Pikachu but the first the articles had no model and no motion list at all. This was briefly
suspected for the crash and is not -- the playtests SAW the parachute, so the c00 model resolved and
the test was on c00 -- but it is a release defect on its own and it is fixed: all eight slots ship,
1684 mod files against 768.

One trap that surfaced with it. `check.sh` compares the tree against `ARTICLE_SLOTS`, and
`deploy.sh` runs `check.sh` internally, so **both must be run with the variable exported** or the
deploy fails its own gate with twelve FAILs that are not defects:

    ARTICLE_SLOTS=all bash tools/deploy.sh --no-retire


# The development loop, and where it actually loses time

Written 2026-09-18 on the instruction to find a way to retrieve visual data without the user
describing it, and to find the other weak links.

## The loop as it stands

    write -> build (~1.5 min) -> check (~4 min) -> deploy (~1 min, or ~13 min when assets move)
          -> USER: reboot console, get into a match, perform the move
          -> USER: describe what they saw, in prose
          -> quit Smash, hbmenu, relaunch ftpd, pull the log, relaunch Smash
          -> read -> write

Two of those steps are the user's and one of them is a person translating three dimensions into a
sentence. Everything below is ranked by how much of the loop it removes.

## 1. The log required quitting the game. It does not. -- FIXED

The worst link, and it was free to fix. Every log this project has ever read came over FTP through
`tools/pull-crash.sh`, and **sys-ftpd exits the moment a game launches**. So reading a log cost:
crash or quit Smash, hbmenu, relaunch ftpd, pull, relaunch Smash, get back into a match, reproduce.
Minutes of the user's time per look -- which quietly made "add a log line and see" an expensive
move instead of a free one, and that is why so much of this document is offline inference where a
measurement would have done.

Skyline mirrors every `println!` to a **TCP listener on port 6969**, and `src/telemetry/sink.rs`
already prints every line it writes to the card. So the entire telemetry stream has been available
live, during play, the whole time. Confirmed by connecting while Smash was running and FTP was
refusing: 6969 accepted, 43022 (arc-network) too.

`tools/logcat.sh` streams it to `work/logcat.txt`:

    bash tools/logcat.sh            follow until interrupted
    bash tools/logcat.sh 90         follow for 90 seconds
    bash tools/logcat.sh 90 ARTICLE only lines containing ARTICLE

The listener is push-only and silent until something logs, so an idle menu produces nothing; that
is not a failure. **No plugin change was needed.**

## 2. Every article bug has been reported as a SHAPE, and the probe only measured a POSITION -- FIXED

"In the wrong hand." "Straight down through the model in both directions." "Stuck inside his body
facing up." Not one of those is distinguishable from correct placement by position alone, and the
probe logged position alone. So each one had to be seen by a human and turned into prose, and every
round the prose was the only evidence.

The probe now logs orientation next to it, and -- the part that matters -- the fighter's `haver`
rotation beside it, because the informative number is the difference:

    ARTICLE yato f1 lr+1 id=0xb exist=true haver=(x,y,z) rel-haver
        rot=(rx,ry,rz) haver-rot=(hx,hy,hz) d=(dx,dy,dz)

`d` separates the three live hypotheses about `ArticleModule::have` in one boot: **d ~ 0** means it
aligns the article to the joint's full frame, **d = the owner's facing** means it carries position
and body rotation only, and **rot ~ 0** means it carries no orientation at all. That question has
been open since Part 5 and has cost three playtests.

One implementation note worth keeping: `ArticleModule::get_joint_rotate` is bound as returning
`u64` in this smash-rs -- the C++ returns a Vector3f by value and the signature did not survive --
so it is unusable without reinterpreting bytes. The probe goes through `article_boma()` and calls
`ModelModule::joint_global_rotation` on the article itself, which gives the same type and the same
call as the fighter's and makes the two directly comparable.

## 3. Screenshots, which the console will hand over for one button press

The Switch's Capture button writes a JPG to `/Nintendo/Album/YYYY/MM/DD/`. sys-ftpd serves the SD
card root, so `tools/pull-shots.sh` can fetch the newest ones -- and a JPG can be looked at
directly rather than described. One button press replaces a paragraph, and it is the ground truth
for anything the numbers cannot settle (silhouette, clipping, "does it read as a sword").

The catch is the same one as the log: **ftpd is dead while the game runs**, so shots can only be
pulled after quitting. That still beats prose, and it composes well -- play a set, capture the
moments that look wrong, quit once, pull all of them.

Holding Capture records the last 30 seconds as MP4. `ffmpeg` is already installed on this machine,
so a clip can be split into frames and stepped through -- which is what a moving article needs,
because a single frame cannot show whether the sword is tracking the hand or lagging it.

## 3b. Live VIDEO is blocked, and not by the thing I expected

SysDVR was installed on 2026-09-18 to stream the console's framebuffer over RTSP. It **runs on
23.0.0** -- the firmware gap that looked like the risk was not the problem -- and ffmpeg reads its
SDP cleanly:

    title: SysDVR - https://github.com/exelix11/sysdvr
    Stream #0:0: Video: h264      Stream #0:1: Audio: pcm_s16be, 48000 Hz

But **zero bytes arrive after PLAY**, video and audio alike; a six-second raw capture produced a
0-byte file. SysDVR's own wiki gives the reason in one line:

> "Only games that support video recording (long press of the capture button) are compatible with
> SysDVR"

**Smash Ultimate disables video recording.** Screenshots work, thirty-second clips do not, and that
is Nintendo's restriction on the title, not a bug and not a firmware issue. The capture backend had
nothing to hand over, which is exactly the signature observed: a clean userspace negotiation over
an empty pipe.

The wiki names the escape -- `dvr-patches`, which removes the per-title recording restriction --
and that is where the firmware finally does bite. The latest dvr-patches release is tagged
**`fw-22.5`** (2026-06-21) against this console's **23.0.0**. Atmosphere matches exefs patches by
the build ID of the system module they patch, so a 22.5 patch is silently ignored rather than
misapplied: it cannot brick anything and it also cannot work.

Two further reasons not to push on this, both from the same wiki page:

- "After copying the sysmodule to the sd atmosphere won't boot anymore -- this happens if you have
  too many sysmodules or ones **particularly heavy like sysftpd**." This card runs sys-ftpd, and
  21.0 and 20.0 each cut the memory available to sysmodules further.
- USB mode takes exclusive ownership of the USB interface, which would break `haze.nro`/MTP.

The console did soft-hang once on a scene transition with SysDVR resident, and between "asked to
capture a title that forbids capture" and "memory pressure alongside sys-ftpd" there is no shortage
of candidates. It was recoverable by closing the game; no reboot was needed.

**So: stills, not video.** The Capture button works in Smash, `tools/pull-shots.sh` already fetches
from `/Nintendo/Album/`, and a still is enough to answer "which hand is it in and which way is it
pointing". Building dvr-patches for 23.0.0 by hand is possible in principle -- find the build IDs
and offsets in the 23.0.0 system modules -- but that is patching system firmware on the only dev
console, to watch a game Nintendo blocked from recording, for a marginal gain over a screenshot.
Not worth it. Revisit if dvr-patches ships a 23.x release.

## 4. The real ceiling: I cannot drive the game, so every experiment costs a human round trip

This is the biggest remaining cost and the fixes above do not touch it. Each boot answers whatever
the build was instrumented to answer and nothing else, so a wrong guess about WHICH question to ask
costs a full cycle. Two ways to attack it, in order of payoff:

**Make each boot answer more.** Cheap and already partly done -- the three generate doors, the
one-shot census and now the rotation delta all ride along in the same boot. The discipline is to
add the measurement that would distinguish the next three hypotheses, not just the current one.

**Run the game on this machine.** A Switch emulator with Skyline and ARCropolis would collapse the
loop entirely: launch, screenshot, read the log off the local filesystem, iterate without the
console or the user. This is a genuinely large setup task with real uncertainty (Skyline support,
performance, whether the mod behaves identically) and it changes the machine's software
environment, so it is written down as a proposal rather than started. It is the only change on this
list that removes the human from the loop rather than shortening their part of it.

## 5. Offline prediction, which keeps catching things after the fact

Three times now the answer was sitting in files on disk and was only looked for after a playtest
reported a symptom:

- The merged weapon meshes are all SINGLE-BOUND, so they cannot be facing-dependent (Part 9).
- `haver` is 3.958 off Pikachu's centreline in the lateral axis, so a right-hand weapon changes
  side with facing (Part 9).
- The game's own held articles do NOT have identity root bones -- `chrom/sword`'s is a full
  rotation, `cloud/fusionsword`'s is a 90-degree axis permutation -- while the authored Yato's is
  identity. That is a complete explanation for "facing up", and it was one `ssbh_data_json` dump
  away at any point.

A gate that dumps each article's root-bone transform next to the socket it is attached to, and
flags an identity root on a `have`-attached row, would have caught the Yato before it shipped.
`check.sh` 3f2 already reads these files; it checks geometry against the donor and says nothing
about orientation.

## 5b. Three boots were spent testing changes stacked on an untested foundation

The worst process failure of 2026-09-18, and it cost more than any single bug in this document.

`ARTICLE_SLOTS=all` landed mid-session while chasing the up-B crash: 768 -> 1684 files, and
`config.json` from roughly 400 declared paths to roughly 1300 across the same 8 costume keys. It
was introduced as a correctness fix (articles resolve under the OWNER's costume slot, so a cNN
Pikachu had no article at all) and it is a real one. It was never booted on its own.

What followed was three consecutive stalls on the match loading screen, and for each one a
different recent change was blamed:

| build | delta from the last build that loaded | blamed | verdict |
|---|---|---|---|
| `befc6df9` | the rotation probe | SysDVR | wrong -- SysDVR was not running for the second stall |
| `b12363df` | sink fix + article root bones | the rotation probe | wrong -- removing it did not help |
| `63693b34` | probe removed | root bones | unproven |

The arithmetic that should have been done on the first stall: `befc6df9` differs from the working
build by ONLY the probe, and `63693b34` differs by ONLY the sink fix and the root bones. No single
one of those can explain all three stalls. The common factor is the foundation all of them were
stacked on, and the one boot where it appeared to work was a single success against three
subsequent failures -- which is not evidence of determinism, and was treated as though it were.

Both stall logs are byte-identical in shape, which is itself the tell: 17 lines, the install phase
completing with all six `clone_weapon` calls returning, and then nothing. The hang is in the
engine's asset load AFTER the plugin is done, which is precisely where a change to the declared
file set would bite and precisely where the probe cannot reach.

**The rule this earns:** a change that alters the mod's ASSET FOOTPRINT -- file count, declared
paths, costume slots, anything `config.json` grows by -- gets a boot of its own before anything is
stacked on it. It is not a code change and it does not fail like one; it fails somewhere the plugin
never runs and the logs cannot see. Cheap to isolate, expensive to confound, and three boots were
spent learning that.

The corollary is about evidence, not assets: when a build stalls, the FIRST question is whether the
same build stalls again. That distinguishes a broken build from a flaky load in one boot and it was
never asked.

## 6. Smaller things, measured

- **`check.sh` runs inside `deploy.sh`**, adding ~4 minutes to every deploy even when the identical
  check passed a minute earlier. A `--no-check` for the case where nothing changed since a green
  run would pay for itself several times a session.
- **`ARTICLE_SLOTS` must be exported for both `check.sh` and `deploy.sh`** or the deploy fails its
  own gate with twelve FAILs that are not defects. That cost a cycle today. The gate compares the
  tree against the variable; it could compare against `config.json`, which is generated from the
  tree and cannot disagree with it.
- **An unbounded `until` poller hung for 9h50m** waiting for a `CHECK_EXIT=` sentinel that a
  different invocation was responsible for writing. Waits should be bounded and keyed on something
  the waited-for command actually produces.

---

# Part 11: a held article's root bone carries its owner's socket bind rotation

Derived 2026-09-18 from files that had been on disk the whole time, after the playtest reported
Corrin's Yato "stuck inside pikachu's body facing up". It is the third mechanism this document has
had to find by measurement rather than by reading code, and the first one that was a single
`ssbh_data_json` dump away from the start.

## The measurement

Dump the ROOT BONE of each article the game actually holds in a hand, and compare it against that
article's OWNER's `HaveR` bind rotation, accumulated up the parent chain from the fighter's own
`model.nusktb`:

| | root bone rotation | owner's `HaveR` world bind rotation |
|---|---|---|
| `chrom/model/sword` | `[-0.185,-0.983,0.014]` `[-0.494,0.106,0.863]` `[-0.85,0.152,-0.505]` | **the same, to 3 dp** |
| `cloud/model/fusionsword` | `[0,-1,0]` `[0,0,1]` `[-1,0,0]` | **the same** |

Cloud's is a clean 90-degree axis permutation and could be a convention. Chrom's is not axis
aligned, and reproducing three rows of arbitrary trigonometry by coincidence is not a thing that
happens. So:

> **A held article's root bone carries its owner's socket-joint bind rotation.**

`gamewatch/model/parachute` has an identity root and does not contradict this -- it is the
motion-driven article from Part 6 and is never `have`-attached.

## What it implies about `have`, and why the symptom fits

If the root bone is where the hand's orientation comes from, then `ArticleModule::have` supplies
POSITION and the owner's FACING but **not the joint's orientation**. Everything the playtests have
reported lines up with that reading:

- Socket `handr`, refused entirely: the Yato sat at Pikachu's FEET -- the owner's origin, no
  attachment at all.
- Socket `haver`, identity root: it moved to his BODY and pointed UP -- attached in position,
  oriented by nothing, so the mesh sat in raw model space.
- Cloud's `fusionsword`, a real article with a real root bone, has worked in both facings since
  `have` was added.

The middle case is the informative one, and it was the user's own observation that the sword had
moved from the feet to the body that made it readable: position was fixed, orientation was not.

## What shipped

`tools/author-article.py` derives the root from **Pikachu's own shipped `model.nusktb`** -- the
file the game loads, and the one the graft reparented `Mouth` in -- rather than hardcoding a
matrix, so it cannot drift from the skeleton it has to agree with. Translation stays zero; `have`
provides the position.

It applies to **both** authored forms, and the argument does not depend on bone count. The staff
renders at `v . local(Stick) . W(HaveR)` on Palutena, so reproducing its orientation RELATIVE TO
THE HAND on Pikachu means the root must play the part of `W(HaveR)` -- his, not hers:

    staff  2 bone(s), 2 mesh object(s),  862 vertices   HaveR(host bind rotation) -> Stick
    yato   1 bone(s), 3 mesh object(s), 3634 vertices   HaveR(host bind rotation), single bind

Fixing only the Yato and leaving the staff on identity would have been arbitrary. If the rule is
right both are now right; if it is wrong, the next boot says so about two moves instead of one.

Three mistakes on the way, all mine and all cheap: `find_src` searches only the donor roots and has
no Pikachu body model to find (it now reads the mod's own); `--verify` compared the matrix with `!=`
and failed on f32 round-trip noise (now 1e-5, far tighter than anything visible and far looser than
rounding); and the log label still said `(identity)` after it had stopped being true.

## The thing this should have been

Part 9 already established that the merged meshes are single-bound and cannot be facing-dependent,
and Part 5 built the authored articles without ever asking what the game puts in a real article's
root bone. The dump that answers it takes one command and no console. That is now the fourth item
on the list in the loop analysis of findings that were sitting in files on disk and were only
looked for after a playtest reported a symptom -- and the strongest argument yet for the gate
proposed there: **flag an identity root on any `have`-attached article**, which would have caught
this before it ever shipped.

---

# Part 12: the article system is the cause, and the crash is a null article the engine never checks

2026-09-19, after four consecutive failed boots -- three stalls on the match loading screen and one
crash -- and four eliminated theories.

## The bisect that settled it

`CLONE_ENABLED = false` **loads and plays a full three-stock match.** Every article disappears, the
merged weapon meshes come back, and the game is completely stable. So the fault is in the article
system and nowhere else.

That is the first unambiguous result in five boots, and it cost one build because the switch was
already there. It should have been the FIRST thing tried rather than the fifth -- see Part 5b of
the loop analysis, which is about exactly this mistake and was written before it was repeated.

## What the crash report says

Two reports, byte-identical frame offsets, so deterministic rather than the flakiness that had
started to look likely. Faulting instruction at `cross2 + 0x33bb330`:

    0x33bb32c  mov x20, xzr      <- a "not found" branch sets the pointer to NULL
    0x33bb330  ldr x8, [x20]     <- FAULT, and X[20] is 0 in the register dump
    0x33bb334  ldr x1, [x19, #0x50]
    0x33bb33c  ldr x8, [x8, #0x10]
    0x33bb340  blr x8            <- a virtual call, on a null object

Frame [04] of the crashed MainThread sits in a function that calls `RemoveArticleByIndex`, and the
telemetry log ends at

    [rat] pika0 status NONE(0xffffffff) -> DEMO(0x107) on frame 0

So: **the engine asks for an article during the entry sequence, gets nothing back, and calls a
virtual method on it without checking.** Our install phase always completes first -- all six
`clone_weapon` calls return in every log -- so nothing the plugin does is on the stack.

The thing to look for is therefore an article that is IN THE TABLE but never materialises, and
exactly one row has that paper trail: Chrom's sword, `exist=false` on 32 consecutive samples across
two boots while `esword`, `fusionsword` and `parachute` come up on frame 1 or 2. That row is
disabled in the build now on the card.

**The caveat that keeps this a hypothesis:** that row has been enabled for many boots that loaded
fine. "It is null" cannot be the whole story; something must also have changed about WHEN the
engine walks that table, and that part is unknown.

## Four theories, each killed by the next data point

| theory | killed by |
|---|---|
| SysDVR | the second stall happened with SysDVR not running (6666 closed) |
| the rotation probe | removing it changed nothing |
| the costume-slot expansion | the crash happened on the c00 build |
| a `CHROM_SWORD` / `GANON_SWORD` asset-name collision | the Yato is hosted on `GAMEWATCH_NORMAL_WEAPON`, ships as `yato`, and was seen on screen twice -- so the engine resolves by the DECLARED name, not the canonical one |

The collision was worth the ten minutes: both really do have the canonical asset name `sword`, and
the host was chosen without checking. It is just not what broke anything.

## Two things about the card that were believed wrong

**FTP serves fine while the game is running.** For most of this project the rule was "sys-ftpd
exits when a game launches, so quit Smash to read anything". That came from anonymous requests
failing with `430 Access denied` -- an AUTHENTICATION error that reads like a transport fault.
`deploy.sh` has always read `.ftp-credentials.local`; `pull-shots.sh` and `pull-crash.sh` never
did, and now do. Logs, screenshots and crash reports can all be pulled mid-match.

**Writing during a match is fine too, and a guard that said otherwise was wrong.** One mid-match
deploy left three files absent after three retries each, and that was read as proof that uploads
need the game closed. It is not: the very next deploy, with Smash CLOSED, needed FIVE repairs and
could not list one directory at all. Failures happen in both states at roughly the measured rate of
one bad upload in eighty, which is exactly why `deploy.sh` has a verify-and-repair pass. The guard
was added and removed the same day; `deploy.sh`'s own header had said "FTP works REGARDLESS of what
the console is running" the whole time.

The lesson is about evidence, not FTP: one failure was generalised into a rule while the
disconfirming case sat unread in the next log file.

Worth stating plainly: every deploy log from today was audited and only the mid-match one failed,
so bad uploads do NOT retroactively explain the stalls. But the margin is thinner than assumed, and
`deploy.sh`'s verify-and-repair pass is load-bearing rather than belt-and-braces.

## A retire bug that orphans files permanently

`--no-retire` on a SHRINKING tree orphans files forever. The retire pass diffs against the
manifest, and the deploy rewrites the manifest to the new smaller set -- so the files it skipped
become invisible to every future run. Going from `ARTICLE_SLOTS=all` to `c00` this way left 916
article files on the card that a subsequent full `deploy.sh` did not touch: it deleted exactly one
file, `param/vl.prc`. They are undeclared and should never load, but the tooling cannot clean them
and that should not be true.

## Where the video went

Smash replays live in the game's SAVE DATA, not on the SD card, so a saved replay is not a file
that can be fetched. Exporting one to video puts an MP4 under
`Nintendo/Album/Extra/<title-id>/YYYY/MM/DD/` -- confirmed, there is one there from 2024 -- but
nothing from today reached the card at all, screenshots included, while captures from 09/17 are
present. That points at the console's capture destination being System Memory rather than the
microSD card.

---

# Part 13: dual-wielding, which works, and the six replaced moves

2026-09-19. Playtested: **"dual-wielding works for all implementations."**

## The fix, and why it is a workaround rather than the answer

Part 9 established that `haver` sits 3.958 units off Pikachu's centreline in the lateral axis, so
facing swings the hand through about eight units of camera depth on a body roughly two deep -- one
sword is in front of his silhouette and the other way it is behind. No attachment mechanism fixes
that, because it is the rig.

`tools/dual-wield.py` appends a left-hand copy of each sword to the shipped model: same vertices,
same material, bound to `HaveL` instead of `HaveR`, with matching `numdlb`, `nusrcmdlb` and
`numshexb` entries. Nine mesh objects across four swords, each vertex-identical to its right-hand
copy and checked by `--verify`. `src/sword/mod.rs` toggles the pair together through `mesh_l`.

It APPENDS rather than regenerating, because `tools/merge-weapons.sh` and the shipped model
disagree -- its table binds chrom to `Sword`, cloud to `fusionsword1` and mythra to `Sword1` while
the shipped model has all three on `HaveR`, and `build.sh` never runs the merge. Regenerating would
have reverted to a configuration that tool's own header records the playtest rejecting twice.

**It is not what the game does.** The donor swings one sword and Pikachu now carries two. The
open follow-up, at the user's suggestion, is to show only the NEAR copy and hide the far one --
the facing is known every frame from `PostureModule::lr` and the two meshes already toggle
independently, so the change is small. What needs a boot is whether the swap reads cleanly at the
moment he turns.

## Articles are now down to two, and the mesh is the preferred path

Chrom's jab worked the moment his article was disabled and the dual-wielded mesh took over. So
wherever a merged mesh exists it is the better of the two routes, and the article is only needed
where there is none:

    esword      OFF   Mythra's dash attack -- was still wrong facing left
    staff       OFF   Palutena's n-air and grab -- never once confirmed correct
    yato        OFF   Corrin's up-air -- "straight up in pikachu's body"
    fusionsword ON    the one article a playtest called fully working
    parachute   ON    no mesh to fall back to; the canopy is four animated bones

## The six replaced moves, and what the pipeline actually wanted

    jab         chrom    -> littlemac      f-tilt  elight   -> sheik
    d-tilt      robot    -> gaogaen        n-air   palutena -> master (Byleth)
    f-air       demon    -> falco          d-air   krool    -> eflame (Pyra)

The first attempt hand-transcribed the donor scripts into the plugin and was entirely wasted:
`tools/gen-moves.sh` GENERATES those modules from the ACMD dumps -- "copying them verbatim gives
frame-data parity by construction, there is no transcription step" -- and `build.sh` runs it, so
the hand edits were regenerated over on the next build. Effect scripts are generated from the
source's `sys_*` spawns only, which makes ADR 0002 automatic rather than a thing to hand-filter.

A move's source fighter lives in FIVE hand-maintained places, and all five must agree:

    tools/gen-moves.sh              the generator's own table -- the actual source of truth
    tools/moves.tsv                 the port list
    tools/ported-motions.txt        the motion list
    tools/motion-patch MAP          which source's FAF/flags/intangibility to copy
    tools/reach topscale MAP        which source's scale to compensate hitbox offsets by

plus `deforeign`'s `SOURCES`, which `build.sh` refuses the build without. Missing the motion-patch
map gave "6 field(s) wrong across 385 checked"; missing topscale's gave hitboxes 2.23 world units
off. Neither is discoverable from the failure message alone.

## Three latent tool bugs this surfaced

**`rebase` corrupted non-hitbox calls.** It counts commas from the bone assuming
`ATTACK(.., BONE, damage, angle, kbg, fkb, bkb, size, X, Y, Z, ..)`, and applied that to any line
containing the bone hash. Pyra's d-air is the first ported move whose EFFECT rides a grafted bone,
and the result was `EFFECT(.., 0, 0.0, 0.9, 0.0, 0, ..)` against a source reading `.., 0, 0.9, 0,
0, ..` -- a mangled call that still compiled. It now rewrites the bone for any call but only moves
offsets for ATTACK-shaped ones.

**`acmdparity --subset` ignored `--rebased`.** The effect-script path compares arguments exactly
and `exit`s before the skip logic runs, so a bone the rebase pass legitimately rewrote read as an
altered call. Latent for the same reason: no ported EFFECT had ever ridden a grafted bone.

**A vacuous assertion became a failure.** `check-weapons.sh` required something to anchor to the
realised `mouth` bone. Byleth's n-air replaced the only move whose hitboxes did, so a correct tree
failed. The load-bearing half -- nothing left on the un-realised source bone -- is still a hard
failure; the other half is now a note.

## What the next boot decides, and how to read it

Six articles install. Two mechanisms are now in play, one per article, and nothing below has been
seen on a screen.

**First, check the banner.** It must read

    [rat] telemetry installed; plugin BUILD 57faec4+dirty 2026-09-18T18:45:12Z

which is md5 `4c2d5ae0`, read back off the card and compared byte for byte at the end of the deploy.
(`59551cc1`, stamped 09:01:11Z, is the build that crashed on up-B; if that stamp appears, the old
plugin is still resident.)
An earlier stamp means the old plugin is still resident and the console needs a full restart rather
than a match reload -- that mistake has already cost two rounds of "behaviour is the same" in this
project, and `tools/deploy.sh` now does the read-back on every run precisely so that the card can
never again be a silent variable.

### The parachute -- SUPERSEDED, read the crash section above first

The build described below crashed the game on up-B and `own_motion` is `false` again. What follows
is what to expect WHEN it is re-enabled, which should not happen before the crash report is read.

#### The parachute, when own_motion comes back on

It now plays the fighter's own motion and is not `have`-attached at all. Expected: the canopy sits
above Pikachu, centred, moving with him as he drifts, in both facings.

    ARTICLE parachute f1 lr+1 id=0x9 exist=true all=(x,y,z) rel-haver

The number to read is no longer "the same in both facings" -- that was the test for `have`. With a
motion-driven article the offset from `haver` SHOULD be nonzero and SHOULD differ between facings,
because the canopy is above the body and `haver` is forward of it. The test is now the screen.

Three ways this can fail, and what each means:

- **Frozen on frame 0** -- `change_motion` is firing every frame, so the edge trigger is not
  holding. `ARTICLE_MOTION[wi]` is zeroed whenever the article is not live; if the article flickers
  in and out, that zeroing re-fires it.
- **Not animated, sitting where it spawned** -- `change_motion` is not resolving, i.e. the article's
  `motion_list.bin` does not have an entry for the hash we passed. The four names are
  `special_hi`, `special_air_hi`, `special_hi_open`, `special_hi_close`; the shipped files are
  `d02specialhi`, `d02specialairhi`, `d02specialhiopen`, `d02specialhiclose`.
- **Nothing at all** -- generation failed; read the `GENERATE` line.

### Chrom's sword and Mythra's -- `persist` is OFF, generation is back to move-time

`persist` caused the up-B crash (see the root-cause section above) and is `false` on both rows, so
generation happens on the frame the move starts, as it did for every boot before that one. The
three doors and the logging below are unchanged and still run -- they are the diagnostic, and they
were never the risk.

    ARTICLE sword GENERATE id=0x6 first_returned=? via=? exist_after=? generatable=?

Throttled to the first four attempts, and fired during the jab only now that `persist` is off.
`via=` names which of the three doors opened: `generate_article`, `generate_article_enable`, or
`generate_article_have_item`. If all three leave `exist_after=false` the engine is refusing the
article rather than failing to build it, and `generatable=` says whether it will admit that.

`esword` is on the same path and logs the same line.

The "Pikachu carries Falchion all match" failure mode described here previously does not apply any
more: it was a consequence of `persist` keeping the article alive outside the move, and with the
per-move lifetime back, an article that generates is removed when the move ends exactly as
`fusionsword`'s is.

### The one-shot census, logged the first time any ported weapon's motion runs

    ARTICLE-CENSUS id=0x0 pikachu-own generatable=? exist=? num=? active=?
    ...
    ARTICLE-CENSUS id=0xb cloned      generatable=? exist=? num=? active=?

Pikachu's own six are the controls. Whatever a healthy article looks like, `0x0`..`0x5` look like
it, and any cloned id that differs from that shape differs for a reason.

### The swords' facing asymmetry will probably still be there

Read Part 9 before concluding the sword articles failed. `haver` is 3.958 units off Pikachu's
centreline in the lateral axis and facing swings it to +3.958, so a right-hand weapon is in front
of his silhouette one way and behind it the other -- and the articles attach to the same joint the
merged meshes bind to, so they render in the same place. If the swords generate and still look
asymmetric, **that is the articles working**; the asymmetry is the rig, and the lever for it is the
animation, not the attachment.

### The four rigid ones

`fusionsword`, `staff` and `yato` are unchanged from the build the last playtest saw, except that
`staff` and `yato` exist at all now. `fusionsword` is the control: it was reported fully
working and must stay that way. `esword`'s earlier "unmoved by `have`" reading came from passing
`TARGET_ALL` to a getter and should be disregarded; the offsets are now read with
`ARTICLE_OPE_TARGET_FIRST`, so the first honest numbers for it arrive with this boot.

### If it crashes on entry

Flip `enabled: false` in `src/articles/mod.rs` **from the bottom of the table up** and rebuild. The
order is deliberate: `sword` and `esword` have no donor code at all, `fusionsword` and `parachute`
bring their own agents, and `staff` and `yato` are the authored pair whose geometry is ours. A
weapon whose row is off falls back to its merged mesh, which is the behaviour that already shipped,
so bisecting never leaves a move showing nothing.

    bash tools/build.sh && bash tools/deploy.sh --no-retire

### What is deliberately still unfixed

**`fusionsword`, `staff` and `yato` still have the port's per-move lifetime**, not the game's. Every
rigid held article in the game is persistent, but all three sit on per-move-generatable kinds and
`fusionsword` demonstrably works, so the flag is set only where the donor never generates -- `sword`
and `esword`. If those two come up with it, extending it is the next thing to try; if they still do
not, the lifetime theory is dead and the census numbers are the next lead.

**Ptooie and the rope stay blocked**, and now for a reason with a shape: their weapon classes read
their donor's `FighterParamAccessor2` entry with the fighter kind compiled in, and
`tools/check-article-donor-params.sh` refuses to let either install. The escape is the engine's own
default for the two offending vtable slots -- `0x33be810` and `0x33bea70`, both bare `return;` stubs
used by 412 and 442 of the 612 weapon kinds -- applied per-INSTANCE through the specializer pointer
at `weapon+0x3bb0` so a real Piranha Plant in the same match keeps its own. Designed, not written,
and it is the one remaining item that patches engine DATA rather than selecting engine BEHAVIOUR, so
it wants a decision rather than a commit.

**Before any of that, run Part 10's experiment.** The slot is null because Piranha Plant is not
loaded, so a match against a real Piranha Plant should populate it and the faulting code should run
as designed. That costs one playtest and no code change, and all three of its outcomes are
informative. It is not wired into this build on purpose: it needs a row the donor-param audit is
built to refuse, and putting a known crasher in the same boot as the article work would mean a crash
on entry tells us nothing about either.

**`ARTICLE_SLOTS` is `c00`.** A release needs `ARTICLE_SLOTS=all`; `tools/author-article.py` honours
the same variable now, so the authored pair extends with the cloned four instead of silently staying
at one slot. Two retirements are pending on the card.

---

# Part 14 — the back throw, and what it is not

Playtest, 2026-09-19: *"trying to back throw makes pikachu let go of the opponent and immediately
run backwards, not actually throw."*

Everything that can be settled without running the game has been, and **all of it is right**. The
list matters as much as any fix would, because each item is a repair somebody would otherwise try:

* **The script is Ness's and is structurally identical to Pikachu's own working back throw.**
  `game_throwb` is copied verbatim by `tools/gen-moves.sh`: two `ATTACK_ABS` on frame 0 (one
  `KIND_THROW`, one `KIND_CATCH`), `CHECK_FINISH_CAMERA`, then one `ATK_HIT_ABS` on the `throw`
  node reading the three `FIGHTER_STATUS_THROW_WORK_INT_TARGET_*` work ids. Pikachu's own fires on
  frame 26, Ness's on 27. Same mechanism, same node, same work ids.
* **The animation is Ness's.** `e01throwb.nuanmb` is dropped in, 68 frames, and it still carries a
  `Throw` node track after the drop-in strips the 36 bones Pikachu does not have.
* **The motion entry is Ness's, field for field** — `turn` set, `move` clear, faf 53, blend 0.
  Vanilla Pikachu's own `throw_b` also has `turn` (and `move`, which Ness does not need), so
  turning is not the difference, and `move` going away is correct: Ness's back throw has no root
  motion to apply.
* **`Throw` is in the shipped skeleton**, so the node `ATK_HIT_ABS` names resolves.
* **The cargo carry cannot be stealing it.** `retarget_forward_throw` writes `globalTable[0x45]`
  and nothing else; `L2CFighterCommon::CatchCont` sends a BACK throw to `globalTable[0x46]`, read
  out of `vendor/decomp` rather than assumed. `pikachu_throw_main` delegates any throw whose
  `CATCH_WAIT_WORK_INT_MOTION_KIND` is not `throw_f`, and `status_Throw_Sub` proves that work id
  really is the chosen throw's motion hash.

## Ness's `Throw` override was the standing suspect, and it is cosmetic

Ness is the only donor here who replaces the common `Throw` init, which made it the obvious place
to look. His installs a per-frame helper into `globalTable[0x15]` — the same slot Donkey Kong uses
for his own per-frame helpers, established twice already in this project. Disassembled (ness
`0x7100006190`, via `tools/disasm`) that helper is, in full:

```
if arg and AttackModule::is_hit_abs(boma) then
    ModelModule::set_joint_translate(boma, hash40("throw"), {0.0, 2.0, 0.0}, false, true)
end
```

One joint lift, two units up, and only **after** the throw has already connected. It cannot be
what stops the throw connecting.

It is deliberately **not** ported. `set_joint_translate` has no matching clear anywhere in Ness's
tree, so a faithful copy risks leaving the node the victim rides permanently displaced — and ADR
0005 does not allow shipping that to fix a bug it cannot cause.

## So the next step is a probe, not a fix

`telemetry::probe::throw_frame` ships two lines per throw attempt:

    in   the motion the engine chose, the three TARGET work ids the ACMD will read on frame 27,
         and whether the grab is still standing on the way IN
    out  the frame reached against the animation's length, and whether `AttackModule::is_hit_abs`
         ever became true

Between them those separate the three remaining stories — wrong motion selected, grab already gone
on entry, or the status ending before frame 27 — and no two of them want the same fix.

---

# Part 15 — complete is not the same as present, and present is not the same as real

Two load failures on 2026-09-20, one crash and one stall, both from articles, and neither from
the article system. Both were the FILES.

## The crash: Byleth's bow shipped 1 of its 18 model files

Data Abort, fault address 0, thread `TaskWorker2`, frame 6
`smashline + 0x2a68c (fighter_initialize_object_data + 0x2c)`. In `vendor/sl_mod.rs` that
function is nothing but the trampoline into `call_original!`, so the null dereference is the
game's own fighter object construction, frames 0-5 all in `cross2`. The same shape as Part 12.

Two articles had gone in at once — Samus's `cshot` and Byleth's `bow` — which is the mistake this
document keeps warning about. Both off loaded; the bisect was one build.

The cause was counting, not code. The archive has **eighteen** files for
`fighter/master/model/bow/c00`. `extracted/` had **one**. `tools/gen-article-assets.sh` searches
`work/arc-vis` first and falls back to `extracted/`; the two articles that had always worked
(`cloud/fusionsword` 14 of 14, `gamewatch/parachute` 16 of 16) were pulled off the running console
into `work/arc-vis` complete, and nothing anywhere said that that was *why* they worked. The bow
had no `arc-vis` copy, so it shipped a model directory with a single file in it.

**`check.sh` gate 3f could not have seen it, by construction.** It asserts the shipped bytes are
identical to the source they were copied from — and they were, because the source was partial
too. A check that compares a copy against its own origin is blind to an incomplete origin.

`tools/check-articlefiles.sh` (gate `3f3`) counts every installed article's shipped files per
costume slot against `work/acquire/arc-paths.txt`, the 590,711-path archive map. Without the map
it refuses to judge rather than passing.

The first suspect was the `cshot`, and it was the wrong one: it is the only article this project
has cloned that has **no model in the archive at all**, so "the novel configuration" looked like
the answer. It ships one motion file of the archive's one. Novelty is not evidence.

## The stall: an article model that is every file and no model

With both articles complete, the `cshot` still needed something to be seen — Samus's shot is made
entirely of her effects, which ADR 0002 excludes. It was lent Pikachu's own electro ball, and the
next boot hung on the training-arena load.

`fighter/pikachu/model/dengekidama` has all eleven of its archive paths, so the count passed —
and every one is a shell. A 40-byte `model.numatb` with **zero material entries**, a 144-byte
`model.numdlb` with **zero mesh rows**. His jolt is a pure-effect article exactly like her shot,
and handing the engine an empty mesh to build a weapon from stalled the load.

So gate `3f3` now also parses every shipped article model and requires at least one mesh object
and one material. All four installed articles clear it:

    fusionsword  6 mesh objects, 1 material      bow     3 mesh objects, 3 materials
    parachute    6 mesh objects, 3 materials     cshot   1 mesh object,  1 material

The charge shot now wears **Samus's supermissile** — a 24,720-byte mesh, populated materials,
its own `numshexb` spheres and four textures. It is her own hardware.

## `model_from`, and why a lent model is safe

The substitution is a column in the Candidate table, not a special case in a generator:
`model_from: "samus/supermissile"`. It is GEOMETRY ONLY. The weapon is still
`WEAPON_KIND_SAMUS_CSHOT`, so its physics, its hitboxes and its straight-line flight are hers; her
`motion_list.bin` declares two motions, `charge` and `shoot`, that name no animation files at all,
so nothing in the lent model has to line up with her motion list. The motion side is never lent.

Four consumers had to learn the column, and one of them failing is instructive: the donor-param
audit's regex ended at `authored: ... \}`, so the new field turned the whole table into ZERO rows
and the audit into a vacuous pass until its own "no Candidate rows found" line said so.

## deforeign retargets cloned-article calls instead of deleting them

Byleth's n-air IS the bow: `generate_article`, `change_motion("attack_air_n")`, `remove_exist`,
all three on `FIGHTER_MASTER_GENERATE_ARTICLE_BOW`, which is 0 — Pikachu's electro ball — so all
three were stripped and the move shipped hitboxes with nothing to look at.

Stripping is right when the article is not cloned and wrong when it is. `deforeign` now rewrites
`ArticleModule::<op>(.., *FIGHTER_<DONOR>_GENERATE_ARTICLE_<ART>, ..)` to
`crate::articles::<op>(.., "<host>", ..)` for any enabled, clean row, reading the table from
`src/articles/mod.rs` so it cannot drift. The wrappers resolve the id at the moment of the call
and are no-ops when the row is off. Only the three ops that actually occur are retargeted; any
other op on a cloned article is reported rather than silently stripped.

## One thing still open

The deploy verify reports `fighter/pikachu/model/cshot/c00/model.xmb` wrong on every run. Every
upload returns 226, `RETR` reads back all 1108 bytes byte-identical, `LIST` shows 1108 — and
`SIZE`, which the verify trusts, answers 0. Deleting first made it worse. Sent by hand with a bare
`curl -T`, it lands every time; the deploy's command differs only by `--ftp-create-dirs`. The
repair loop's retries now drop that flag and log which variant landed the file, so the next deploy
with the console up settles it. All 42 files of both new articles were verified out of band.
