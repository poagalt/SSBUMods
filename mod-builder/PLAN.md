# A moveset builder — plan

Status: **proposal, not accepted.** Nothing here is settled until the open questions at the end
are answered.

## The goal, restated

Today `pikachu-moveset/` puts 25 specific moves from ~20 specific fighters onto one specific host.
The goal is a tool that takes **(source fighter, slot, host fighter)** — or **(source fighter,
attribute, host fighter)** — and produces a working, parity-checked mod for any such triple.

The honest restatement, which the rest of this plan is built on: **most of that job is already
mechanical, a minority of it cannot be mechanised at all, and the value of the tool is in knowing
which is which before any work starts.** The Pikachu project spent most of its calendar on 12 of
its 52 moves, and it did not know which 12 until it had failed at them.

## What is measurable today

Not estimates. These come from the local corpus, measured while writing this.

| Quantity | Value | Why it matters |
|---|---|---|
| Fighters with a decompiled status tree | **95** | the source side is covered |
| Fighter-body statuses across the roster | **3,515** (mean 37, max 415 for Kirby) | the size of the class-3 population |
| Statuses with an `init` line | **1,269** | |
| Statuses with an `exec` line | **1,205** | |
| Statuses with an `exit` line | **831** | |
| Statuses with `check_dmg` / `check_atk` | **65** / **46** | rare, and each is a one-time unlock |
| Modal line count per status | **4** (2,105 statuses); 326 have 10+ | the tail is real |
| Non-body article model directories | **734**, across **89 of 97** fighters | articles are the common case, not an edge case |
| Engine functions decompiled | **100,539** | the engine side is covered |
| Lua constants with names and values | **39,213**; const table 19,310/19,310 | every constant in a disassembly is now readable |
| Pikachu project moves with their own source status tree | **12 of 52 (23%)** | the class ratio, on one sample |

Two of those numbers reframe the whole problem.

**`init`/`exec`/`exit` occur in 20–30% of statuses.** The most expensive single bug in the Pikachu
project — a cargo carry that entered its walk status and moved `+0.00` units while looking alive,
because DK's `SHOULDER_WALK` has an `Exec` line the port never registered — was not bad luck. It
is a ~28% risk on every status a port touches, and it is **detectable by `ls`**. Any builder that
does not enumerate lines from the source's own directory listing will reproduce that bug at that
rate.

**89 of 97 fighters own articles.** ADR 0004 currently forbids them, because article ids are
per-fighter enumerations and `clone_weapon` crashes inside the game's own
`fighter_initialize_object_data`. For a one-host project that is a liveable carve-out — it costs
Ptooie and the Alolan Whip's rope visual. For a general builder it is a wall across most of the
roster, and it has to be either solved or declared.

## The core idea: derive, never author

Every hand-maintained list in the Pikachu project has drifted at least once, and one class of bug
came from having **three** parallel lists — `gen-moves.sh`'s inline groups, `tools/moves.tsv`, and
`topscale`'s `MAP` — that had to be kept in step by hand. `tools/check-lists.sh` exists solely to
detect that drift.

A builder makes that structural rather than policed: **one manifest naming the triples, and
everything else computed from the game.** If a fact can be read out of the corpus, reading it is
not optional.

## Architecture

Five layers. Each is useful on its own, which is also the phasing.

### Layer 0 — the Game Database

One queryable, versioned store built from the corpus already on disk. Not a new acquisition
effort; an index over what `sources-of-truth.md` already documents.

Per fighter: statuses (each with **the set of lines it actually has**), motions (animation, flags,
`cancel_frame`, `xlu_*`, `blend_frames`, the four script names), ACMD scripts per category, params
by (object, key, storage type), skeleton (bones and bind transforms), articles, `scale`, and the
status-kind enumeration with its `NUM`.

Cross-cutting: the constant table (offset → name → value), hash40 both directions, the common vs
fighter-scoped split for statuses, work ids, and params, the 100,508 entry points with their
decompiled bodies addressable by address, and the engine symbol table.

**It records its provenance and refuses to mix versions.** Everything is 13.0.5 except the status
decomp, which is 13.0.1 and matches because the fighter NROs are `patch-13.0.1` builds. A game
update invalidates addresses but not method, and a store that silently mixes two dumps is worse
than no store.

### Layer 1 — the requirements manifest

This is the central artifact and the main new idea.

Given `(source, slot)`, compute — by analysis, not by hand — everything that move *needs*:

- **ACMD**: which of the four script categories exist, and what macros they use.
- **Motions**: which motion entries, their FAF and intangibility, their `move` flag, and their
  animation files.
- **Params**: every `(object, key)` the move reads, each tagged `common` (free on any host),
  `fighter_param` (patchable), or `vl.prc` (needs providing), with its storage type — because the
  engine coerces `get_param_int`/`get_param_float` and a provider that doesn't will silently
  return 0, which cost five whip values including a motion rate of zero.
- **Statuses**: every status the move enters, each with its line set and its scope (engine-common,
  source-scoped, or new).
- **Foreign constants**: every source-namespaced constant, classified as article id, instance work
  id, status-scoped work id, or status kind — because the first three collide across fighters and
  the correct handling differs per kind.
- **Other objects**: does this move put another battle object into a status, link to it, or send it
  events? If so, which victim-side statuses, and which params do *they* read off the captor?
- **Articles**: which it generates, and whether the host has a free id.
- **Bones**: which bones its hitboxes anchor to, and whether the host's skeleton has them.
- **Effects**: `sys_*` versus fighter-prefixed.

From that manifest the **feasibility class falls out mechanically** — no judgement call, no
failing at a move to discover it was hard. And the per-move parity checks are *generated from the
same manifest*, so a check cannot go stale against the thing it checks.

### Layer 2 — resolvers

One resolver per requirement kind, each with an explicit fallback ladder and each recording which
rung it used. That record is the port's honest provenance: what is native, what is transcribed,
what is substituted, what is absent. `what-the-grabs-need.md` is that table, hand-built for two
moves; here it is an output.

| Requirement | Ladder |
|---|---|
| bone the host lacks | graft the source's bone under `HaveR` with its bind transform → rebase hitboxes onto `haver` → substitute nearest host bone → **fail** |
| foreign instance work id | allocate a port-owned state slot named after the source constant |
| status-scoped work id | keep (self-consistent when the port owns the status) |
| status kind | allocate past the host's `NUM`, **deterministically, from a lockfile**, so ids are stable across builds |
| param, `common` scope | nothing to do |
| param, `fighter_param` | patch the host's row |
| param, `vl.prc` | generate a provider entry answering both storage types |
| motion the host lacks | drop in the source animation + synth the entry (repointing **all four** script names, not just `game_script`) → repoint to a host animation whose length and root motion suit → **fail** |
| victim-side status | route through the engine (write the destination into `globalTable[0x45..0x48]`) → request the common status directly → substitute |
| article | host's own free article id → `clone_weapon` → absent, with the visual gap recorded |
| effect | `sys_*` only, or opt-in CSK mode |

### Layer 3 — codegen and the mod

Emit the plugin crate, `config.json` (derived from what the mod tree actually contains), the
`fighter_param`/`fighter_param_motion` patches, the provider tables, the motion list, and the
generated telemetry. Every emitted file carries an `@generated` banner and nothing hand-edits
them — the Pikachu project lost a commit's worth of work to a hand-edit the build regenerated
over, and `topscale` had to learn to refuse files lacking that banner.

### Layer 4 — verification, generated per move

Every existing check becomes a generic rule over the manifest: bones exist, hitboxes land where
the source's land, non-geometric ACMD arguments match, every engine call the source makes is made,
every param resolves to the source's value, every status line the source has is registered, no
foreign namespaced constant survives, the three lists agree (vacuously — there is one list now).

Plus the rule the Pikachu project arrived at the hard way: **a check is not finished until it has
been made to fail on purpose.** Five false cleans in one round, and one of them would have
reported no gap when there was one. Generated checks get generated negative tests.

### Layer 5 — the loop

Telemetry generated to instrument exactly the requirements that were resolved by *substitution* —
the rungs below the top of each ladder, which is where the bugs are. Plus native-recording
support: trace the source fighter on hardware, trace the port, diff them line by line. That diff
is what settled the capture, the kinetics, and the throw input after static reading had failed.

## The hard class, specifically

Cargo throw and Alolan Whip are the test of this plan, so here is the direct answer.

**They cannot be fully automated, and any plan that claims otherwise is wrong.** The reasons are
structural, not effort-shaped:

1. **Branch selection.** The whip's status tree is 748 lines of decompiled C of which most branches
   never run. Which ones do is not deducible from the source; it was settled by recording the
   source fighter on hardware and reading the executed path.
2. **Invisible calls.** `sv_kinetic_energy::*` go through the lua stack, so no function hook sees
   them. A runtime recording cannot show them and only the disassembly can — the two sources are
   complementary, and neither alone is sufficient.
3. **Same-frame overrides.** `init_settings` declares `UNIQ` and the status's own `main` overrides
   it to `GROUND_STOP` on the same frame. A transcription of the first call is "correct and still
   wrong".
4. **The other fighter.** A grab is two fighters running two pieces of code, and the victim's half
   reads its numbers off the captor by param object and by instance-work index. Pikachu's
   instance-work array ends at 232 and the victim reads a rope flag at 234.

So the builder's job for class 3 is **not** to generate the implementation. It is to make the
implementation cheap to write and impossible to write incompletely:

- **Generate a scaffold, one module per status, one function per line the source actually has** —
  from the directory listing, which is the mechanical answer to the ~28% missing-line risk. Each
  function pre-populated with the source's decompiled C as a comment, the ordered list of engine
  calls it makes (`disasm --trace`, which drops the L2CValue marshalling and yields 45 lines of
  signal instead of 400 of noise), and every constant resolved to its name **and now its value**.
- **Generate the state shim**: one named slot per foreign instance-work id the source uses, named
  after the constant it stands in for. Every round of the whip rewrite picked a different ad-hoc
  substitute for that same missing state and none agreed with the source.
- **Generate the victim-side requirement as a first-class item**, with the params it reads off the
  captor listed, so "the whipped opponent goes nowhere" is a pre-build warning rather than a
  playtest report.
- **Gate on completeness, not correctness**: every line registered, every engine call the source
  makes present somewhere in the port, every param resolving. Correctness still needs a playtest;
  completeness should never need one.
- **Prefer an engine route wherever one exists.** The cargo's entire entry problem collapsed to
  writing one status id into `globalTable[0x45]` and letting the engine's own `CatchCont` make the
  transition. Any resolver that can route instead of reimplement must.

Expected outcome, stated honestly: class 1 and 2 fully generated; class 3 scaffolded, with the
human or model writing bodies against a checklist that cannot be short. That reduces the hard
moves from "seven rounds each" to "one careful pass plus a playtest", which is the real win. It
does not reduce them to zero.

## Attributes and stats

Much easier, with one trap worth building in. Parameters are **coupled**: Falco's `jump_y` only
produces Falco's jump height at Falco's gravity, because the stored value is an initial velocity;
and changing `air_accel_y` or `air_speed_y_stable` invalidates the air-dodge FAF, which is
computed from them plus `escape_air_slide_speed` (HewDraw has a CI job whose only purpose is
reminding people of this).

So attributes are transferred as **dependency groups**, not keys: a group either moves together or
the builder refuses and says which key is missing. And the landing-lag and dodge families live in
different files from the movement ones — `landing_attack_air_frame_*` in `fighter_param`, the
`escape_*` family in `fighter_param_motion` — so a "port the dodges" request that only patches
motion entries is inert, which it silently was for a while.

## The feasibility oracle

The deliverable that makes this a builder rather than a project: **precompute the manifest for
every (source, slot) pair and publish the matrix.**

Most requirements are host-independent, so the expensive half is computed once — roughly 95
fighters × ~40 portable slots. Only bones, motions, articles, and status-id allocation depend on
the host, and those are cheap to evaluate on demand. The output is a table saying, for any triple:
its class, what it needs, which resolver rungs it would use, and what would be absent.

That answers "can I put Steve's blocks on Jigglypuff" before anyone writes code, and it turns the
project's hardest historical question — *why is this move fighting me?* — into a lookup.

## Phasing

Each phase ends in something usable, and each is testable against the existing Pikachu port as its
regression suite — a builder that cannot reproduce the 52 moves already shipped is not ready.

1. **Layer 0 + the manifest, read-only.** No codegen. Deliverable: the feasibility matrix, and a
   manifest for all 52 existing Pikachu moves that agrees with what was actually needed. If it
   disagrees, the manifest is wrong and that is cheap to learn now.
2. **Class 1 end to end** for an arbitrary triple, with generated checks. Regression: reproduce
   the Pikachu project's 40 class-1 moves byte-for-byte.
3. **Class 2**: animations, motion synthesis, the variant audit in both directions.
4. **Attributes**, with dependency groups.
5. **Class 3 scaffolding**, validated by regenerating the scaffold for the whip and the cargo and
   diffing against the hand-written modules that now work.
6. **Articles** — or a documented refusal. This is the one phase that may be a research project
   rather than an engineering one.

## Non-goals

- Not a GUI, and not a tool for end users. The class-3 path requires judgement; the operator is a
  model-plus-human loop with the corpus at hand.
- Not a balance tool. It ports what the game does; it does not tune.
- Not CSK-independent by assumption. `sys_*`-only is a *mode*, because for a general builder the
  fighter-prefixed effect archive is often the only faithful answer.
- Not version-agnostic. 13.0.5, pinned, declared, and re-derivable.

## Decisions — settled 2026-09-14

Nine are recorded permanently in [docs/adr/](docs/adr/), because each is a thing a future reader
would otherwise undo: **0001** a tool anyone can use (decision 1) · **0002** derive, never author
(5) · **0003** declared support tiers (8) · **0004** transpile, do not transcribe (11) · **0005** a
cloned article stays faithful to its owner (13) · **0006** a scoped slot is the default output
(27) · **0007** the CSS entry is data, not code (28) · **0008** source effects are a priced option,
not a CSK build (4, 29) · **0009** ship the facts, build the data (10). The rest live in this
table.

Three decisions are accepted on a **reading** rather than an observation, and
[experiments/](experiments/) holds one small test for each. ADR 0007 is explicitly marked
*accepted pending* its result.


| # | Decision |
|---|---|
| 1 | **A tool anyone can use.** Not an audience of one. This is the answer that reshaped the rest: a transcription scaffold is not a deliverable, so class 3 must be solved. |
| 2 | **One tool, two commands** — moves and attributes share the Game Database and the manifest, and have separate resolvers. |
| 3 | **`pikachu-moveset/` is the regression suite, untouched.** The builder must reproduce it; it never rebuilds the shipped mod. |
| 4 | **`sys_*`-only is the default, with a source-effects build as an explicit option.** Renamed from "CSK build", which was factually wrong: CSK does not touch effects at all. The option depends on the separate **One-Slot Effects** plugin and costs a rebuilt host effect archive per slot — ~21 MB each, measured from a shipped mod. |
| 5 | **Derive, never author — except a `measured/` file** whose every entry cites the recording it came from, and which a check rejects once the value turns out to be derivable. This retires the *parity table* concept entirely. |
| 6 | **One move at a time, no opinion about the resulting character.** |
| 7 | **Rust core, SQLite Game Database**, queryable by the existing bash checks. |
| 8 | **Declared support tiers**, per fighter and per role — refused as host: the three Miis, `ptrainer`, `ptrainer_low`, `element`; with caveats: Kirby, the Pokémon and Aegis bodies, Ice Climbers, `koopag`; the 12 echo fighters usable as class-1/2 sources only until their enumeration mapping is verified rather than inferred. |
| 9 | **Articles are in scope.** The `clone_weapon` crash is root-caused (see `handbook/open-questions.md` §3b) and is being solved separately. Plan for article support everywhere. |
| 10 | **Ship the facts, build the data** — plus a private corpus repo for development. See the evaluation below. |
| 11 | **Transpile, don't transcribe.** A Ghidra-C → smashline-Rust transpiler for status functions, validated against the twelve hand-written modules that already work. |
| 12 | **A GUI is the end goal**; whatever serves development until then. So the core is a library and every front end is a client of it. |
| 13 | **A cloned article is faithful to its original owner** — packun's spikeball on Pikachu keeps packun's scale and params, because the article *is* the source's object. Recorded in the output, not compensated. |
| 14 | **Fighter-wide traits do not port, by construction, and the output says so.** A trait is not part of a move; naming the ones it declined to port is fidelity information, not an opinion. |
| 15 | **The CSK build differs in exactly one axis: effects.** One animation strategy for both. The cross-fighter sharing investigation came back — see below; it does **not** replace the animation pass, and it raises a live question about whether the CSK build is needed at all. |
| 16 | **A missing helper body refuses the move**, naming the helper and the command that fetches it — escalating to auto-acquisition once the pull is scripted. Never a stub that compiles: a silently partial port is the bug class the completeness gates exist to kill. |
| 17 | **Completeness offline plus an in-game self-test** whose report the user can hand back. The tool cannot playtest for its users, and a curated-only "supported" list would undo decision 11. |
| 18 | **13.0.5 only; refuse anything else.** So the Game Database carries a version fingerprint, and its job is *refusal* rather than adaptation. (This overrides the recommendation to derive per-user version-sensitively — the simpler contract was chosen.) |
| 19 | **A curated slot list plus an escape hatch** to name a raw motion entry. The curated list is what a user wants; the hatch is what made a counter, a command grab and a cargo carry possible here. |
| 20 | **GUI-drives-everything as the end state, CLI bootstrap in the interim**, with the corpus optional — the feasibility matrix is shippable, so a user can see what they are in for before committing a console session. |
| 21 | **The transpiler emits generated Rust on disk**, regenerated by the build and never hand-edited. The entire completeness-gate layer works by reading source text, so an IR would mean rewriting every gate. |
| 22 | **Collisions between moves sharing a host are detected and resolved deterministically where a rule exists, refused where none does.** Status ids from a stable lockfile; article slots from the host's free indices; motion variants by repointing; and the param collision by **scoping the provider's answer to the status that is asking** — the mechanism already proven here after answering one status-scoped work id globally made Pikachu run at absurd speed. |
| 23 | **The self-test verdict is local**, with an option to write it to a file the user may keep or share. Nothing is reported back automatically. |
| 24 | **Ship the derived facts so no user needs Unicorn**, and **handle Ghidra's absence gracefully by denying class 3** rather than failing. Capability-gated refusal, using the same machinery as the roster tiers in decision 8 and the missing-helper refusal in decision 16 — one refusal path, three reasons. |
| 25 | **Share instead of shipping wherever the predicate holds**, and run the two-file experiment (Chrom's jab, Ganondorf's ledge attack) to exercise ARCropolis's untested `reshare_file` branch. |
| 26 | **Keep two builds for now** — but the CSK question is not settled, because CSK is also about **adding a fighter slot rather than replacing a roster member**, and if that is achievable without it, CSK can be dropped entirely. The slot axis is now answered: achievable without CSK, as a new costume slot. Only the effects axis remains. |
| 28 | **Character registration is declarative** — emit `mods:/database/<char>.json` plus `flags/reslot.toml`, no plugin for this part — **gated on one experiment**, because no shipped mod exercises that path. Fall back to the ~13 CSK API calls if the experiment fails. |
| 29 | **The effects option is renamed** (see decision 4) and its real cost recorded. |
| 27 | **A scoped slot is the DEFAULT output.** Revised the same day it was taken: the first version made it a per-project mode with a refusal, because attributes were believed not to scope. They do — `lib_paramconfig` re-keys the param getter on the costume slot — so the reason for the refusal evaporated and the safer default became the better one. A generated mod adds a character to a host's spare slot rather than deleting the host. Replacement stays available as an opt-in. |

## The output question, which this plan had not asked

Decision 26 exposed something the plan assumed without stating: **does the builder produce a mod
that REPLACES a roster member, or one that ADDS a slot?** Everything above — and the whole Pikachu
project — silently assumed replacement. `pikachu-moveset` installs agent-wide on `pikachu`, so all
eight costumes are the custom character and vanilla Pikachu is gone while the mod is enabled. For a
tool other people use to build characters, that is very likely the wrong default.

Three different things get called "a separate slot", and they have different costs. One bound is
already established and it is hard:

The investigation reported, and all three answers are now known.

### C. A new fighter kind — not possible, and CSK cannot do it either

The 94-kind bound is corroborated three independent ways: the constant table
(`FIGHTER_KIND_TRAIL = 0x5D`, `FIGHTER_KIND_TERM = 0x5E`), the clearing loop at `main 0x709d60`
whose bounds are exactly `0x60 + 0x38 × 94 = 0x14F0`, and a 94-arm dense jump table with every
entry read. It is not one site: a sweep of all 15.1 M instructions of `.text` found **22 separate
94-arm dispatch tables** bounded by `#0x5d`/`#0x5e`, 150 comparison sites in total, and two more
stride-indexed per-kind arrays. A 95th kind means growing a fixed array *and* extending 22
compiled jump tables *and* patching a hardcoded 94-way name→id decision tree. None of it is data.

**The `TABLE_EX` pseudo-kinds (`0x6E`–`0x75`) are not a hole to grow into.** They are CSS
identities *and* weapon-owner ids — which is how **Pokémon Trainer himself turns out to be an
article**: `WEAPON_KIND_PTRAINER_PTRAINER = 0x1CA`, owned by pseudo-kind `ptrainer` = `0x72`. The
sixteen slots at `0x5E`–`0x6D` are all literally named `"none"`, sharing one string pointer.

**Correction to what this plan said an hour ago:** the creator does *not* raise an
undefined-instruction exception for an out-of-range kind. That reading came from a Skyline hook
trampoline at `0x70c580` (smashline hooks that offset; the `udf` is the high word of a pointer).
The real out-of-range branch **reads out of bounds and tail-calls an initializer with garbage** —
silent corruption rather than a clean crash.

### A. Costume-slot scoping — possible today, with one trap and one hard limit

`smashline::Agent::set_costume(vec![7])` is a single call that scopes an entire agent, covering all
four ACMD categories, all 17 status lines, and both callback kinds. The reference mod's agent-wide
behaviour is simply the absence of that call. Per-slot **assets** need no API at all — the archive
is natively per-`cNN` and the path is built by `c%02d`, which this project already relies on
("Costume 0 only" in `BACKLOG.md`).

**The trap:** `ACMD_SCRIPTS` is keyed by `(agent, costume_set)` and the installer does exactly one
map lookup, whereas status scripts and callbacks are filtered per item. So mixing agent-wide and
costume-scoped ACMD on one agent **silently drops the agent-wide scripts** while that costume is
played. The rule for generated mods: if any ACMD on an agent is scoped, scope all of them.
Two lesser ones: wildcard agents (`"fighter"`, `"weapon"`) cannot be scoped at all, and
overlapping costume groups are undefined in the source — there is a literal `TODO` where the
collision would be handled.

**A hard limit was asserted here and it is FALSE — corrected 2026-09-14 by reading shipped mods.**
The claim was that `fighter_param` has no costume dimension, so a scoped-slot character cannot
have its own weight, speeds, gravity, or jumps. The first half is true and the conclusion does not
follow, because nobody uses `fighter_param.prc` for this. **`lib_paramconfig` hooks the engine's
param getter and re-keys the lookup on the costume slot:**

```
get_float_param_hook:
    blr  <original get_float_param>              ; keep the vanilla value
    bl   WorkModule__get_int_impl  [0x10000042]  ; FIGHTER_INSTANCE_WORK_ID_INT_COLOR
    bl   utility::get_kind
    bl   FighterParamModule__get_float_param     ; (kind, colour_slot, param, subparam)
    cmp  w0, #1                                  ; Option::Some -> override
```

`0x10000042` is `FIGHTER_INSTANCE_WORK_ID_INT_COLOR`, verified against the constant table. The
override table is keyed on **(fighter kind, costume slot, param, subparam)**, and weapons resolve
their owner's accessor first and pass a negated kind. So **a slotted character can have its own
attributes** — not through the prc, through a runtime hook on the one function every reader of
that table goes through.

Two of the four mods drive it declaratively and two from code, which is itself informative:

```toml
# Sonic Re-Imagined [SL2 Universal]/…/config_param.toml
kind = "sonic"
slots = [-1]                 # -1 = every slot
[[param_float]]
param = "walk_speed_max"
value = 1.5
```

`slots = [-1]` means all slots; the **Slotted** build ships no TOML and hardcodes the same twelve
values in Rust so it can pass its own narrow slot list. That is the cleanest signal available that
the declarative path exists but the slot list has to come from code when you want it narrow.

**What genuinely does not scope is `vl.prc`**, and none of these mods solves it. Waluigi's
`vl.prcxml` adds `param_diceblock` and Herobrine's adds `param_pearl`, both for cloned articles,
and both apply to **every costume of the base fighter including the vanilla ones**. Thwomp and
Sonic Slotted duck it by shipping no `vl.prc` at all.

### B — SETTLED EMPIRICALLY, 2026-09-14, by reading shipped mods

Three character mods extracted from GameBanana — Waluigi (on `dolly`), Thwomp (on `ike`), and
Herobrine (on `mewtwo`) — all do the same thing, and it is not a ninth colour slot. They create
**costume slots numbered `c120`, `c121`, `c122`**, as brand-new dir-info nodes:

```json
"new-dir-infos": [ "fighter/dolly/c120", "fighter/dolly/result/c120",
                   "fighter/dolly/movie/c120", "fighter/dolly/camera/c120",
                   "fighter/dolly/kirbycopy/c120", … 40 entries ],
"new-dir-infos-base": { "fighter/dolly/c120/camera": "fighter/dolly/c00/camera",
                        "fighter/dolly/c120/cmn":    "fighter/dolly/c00/cmn",
                        "fighter/dolly/kirbycopy/c120/cmn": "fighter/dolly/kirbycopy/c00/cmn", … }
```

Three things follow, and each retires something this plan had listed as unknown:

- **`new-dir-infos` and `new-dir-infos-base` work.** This plan recorded them as "asserted in this
  project's notes and verified nowhere". Three shipped mods use both, so the capability to create a
  dir-info node that vanilla does not have is real, and it needs no CSK.
- **Slot indices are not capped near 8.** `c120` is consistent with what was read from the engine
  rather than a surprise: the path is formatted `c%02d`, where `02` is a *minimum* width, and the
  selected slot is stored as a byte, so up to 255 is expressible. The high numbers are presumably
  a convention for avoiding collisions between slot mods.
- **So "add a character to the roster" means a new costume slot on an existing fighter.** The
  fighter kind is still `dolly`; Waluigi is `dolly` slot 120. That is fully consistent with the
  94-kind bound and it is the shape the builder should target for added characters.

The `color_num` reader and the `ui_chara_db` route described below remain the mechanism for making
a slot *selectable*; how these mods handle that — patched database, runtime plugin work, or a
dependency on a separate slot-selection plugin — is under investigation.

### B (original analysis) — bounded by data rather than code

**A premise this plan asserted is retired:** the three Mii Fighters do **not** have `c00..c11`
costume slots. They have no `model/body` at all; what they have is ~50 outfit-part directories
each with twelve *colour* variants. They are not evidence that any fighter can have a ninth slot.

What actually bounds the count is **`color_num`, a `ui_chara_db.prc` field**, read at
`main 0x32633f0` across 46 call sites — every one treating it as the loop bound or the validator,
with no hardcoded 8 anywhere. The selected slot is validated as `selected < color_num`, stored as
a byte, and the path is formatted `c%02d`. Two things remain unread: `ui_chara_db.prc` itself is
not extracted locally, and the archive side needs a `fighter/<host>/c08` dir-info node —
`new-dir-files` attaches to an *existing* node, and `new-dir-infos`, which would create one, is
asserted in this project's notes and verified nowhere.

**Experiment, two steps, one deploy each:** raise Pikachu's `color_num` from 8 to 9 and count the
colour strip on the CSS — nine means the bound is data. Then, only if that passes, try to make
`c08` resolve. Establish first whether this ARCropolis build honours `.prcxml` or only binary
`.prcx`, or a null result is uninterpretable.

### What this leaves for decision 26 — revised after reading the mods

The answer split in two, and the halves point opposite ways.

**The slot's FILES need no CSK.** `new-dir-infos` + `new-dir-infos-base` + `new-dir-files` create
and populate the nodes, and smashline's `set_costume` scopes the code. All ARCropolis and
smashline, both already dependencies.

**The slot being a SEPARATE CHARACTER on the CSS requires CSK, or an equivalent of it.** All three
character mods call `add_chara_db_entry_info` and `add_chara_layout_db_entry_info` from
`the_csk_collection_api`, and the reason is structural: **a `.prcxml` can patch an existing
`ui_chara_db` row by index but cannot append one.** The Bomb skin mod in the same folder proves it
— it ships a `ui_chara_db.prcxml` with a 121-entry `db_root` list of which 120 entries are
`dummy` placeholders, patching exactly one real row. Appending a row is what CSK's runtime API is
for.

The Thwomp author documents the whole registration in a text file, which is the most useful single
artefact in the set:

```
Character ID: ui_chara_dossun     Name ID: dossun
Base: ike     Colors: 8     Color Start Index: 40
```

So a new CSS character is `(new ui_chara row, base fighter kind, colour count, colour start
index)` — and the slot numbers are just `start .. start + count`. Waluigi is `dolly` 120–127,
Thwomp is `ike` 40–47, Herobrine is `mewtwo` 32–39.

**The full dependency stack these mods require**, from their own startup checks and READMEs:
ARCropolis, NRO_Hook, Smashline 2, **ParamConfig**, **The CSK Collection**, and — for Thwomp only
— One-Slot Effects. So "drop CSK" is not available for added characters unless the builder
implements the `ui_chara_db` append itself.

**And the `.marker` pattern is verified.** This plan asserted it from a second-hand note and the
first investigation could find no trace of it; the mods ship it plainly —
`fighter/dolly/model/body/c120/waluigi.marker`, `dossun.marker`, `herobrine.marker` — and the
plugins probe it with `nn::fs::GetEntryType` to learn which slot is theirs. Worth knowing that
`herobrine.marker` is 324 bytes of an accidentally-copied AutoHotkey template and works fine,
which confirms only existence is tested.

Whether the builder's default output is a replacement or a scoped slot is now a decision rather
than an unknown — and it is a bigger question than it looked, because the `fighter_param` limit
means a scoped slot cannot carry its own stats. It changes the glossary's *host fighter*, the
collision rules in decision 22, and what a generated mod may overwrite.

## Cross-fighter file sharing: what the investigation found

**The capability is real, engine-native, needs no CSK, and this repo already uses it.**
`pikachu-moveset/mod/config.json` points three Pikachu texture paths at
`fighter/chrom/model/sword/c00/met_chrom_001_*.nutexb` through `share-to-added`, and ARCropolis's
`add_shared_file` resolves that source by a **global hash lookup with no parent-fighter check** —
then leaves `FileData` alone, so the host's directory load pulls the source's real bytes out of
`data.arc`. Path resolution in the archive is global and directory-agnostic; `dir_info` controls
only *residency*. The two keys differ by ordering around ARCropolis's unshare pass:
`share-to-vanilla` before it, `share-to-added` after.

**It does not replace the animation pass, and the reason is measured.** Of the 74 animations this
mod ships, **one** is byte-identical to its vanilla source — Chrom's jab. The other 73 carry two
edits the port depends on: the foreign bone- and mesh-track trimming, and the `Rot` scale multiply
by `source_scale / 0.96`. **You cannot apply an edit to bytes you do not ship**, and going
untrimmed was already measured to fail: the animation directory went from +637 KiB to +3,673 KiB
over vanilla and the in-game self-test reported 25 of 30 added files unresolved
*nondeterministically, differing between two loads of the same build*. Sharing and the trim pass
are mutually exclusive, and the share direction is the one that has already been shown to break.

**Shareability is a computable predicate, and the class is not small.** Measured independently:
**2** of the 74 shipped animations are byte-identical to vanilla — Chrom's jab and Ganondorf's
ledge attack. The reason is not that those sources sit at scale 1.00; it is that
**`chrom`, `ganon`, and `pikachu` are all scale 0.9600**, so the `Rot` multiply by
`source_scale / 0.96` is exactly 1.0 and the only remaining edit, track trimming, had nothing to
drop because those animations touch no bone or mesh Pikachu lacks.

So the rule is: **share when `source_scale == host_scale` and the animation's track set is a
subset of the host's bones and meshes.** Both halves are decidable offline from `tools/scales.tsv`,
the host's skeleton, and the animation's own track list — no byte comparison required, which means
the manifest can resolve `ship` versus `share` per move.

And the class is large. For Pikachu, 10 fighters share 0.9600 (`brave chrom ganon kirby pikachu
roy samus samusd trail wiifit`) — nine candidate sources. Roster-wide the modal scale is
**1.0000 with 38 fighters**, so for a host at 1.00 any of the other 37 same-scale fighters is a
candidate on the scale axis. Sharing is potentially the majority path for same-scale pairs, not a
curiosity worth one special case.

**A correction to this project's own prior finding.** The note that "across 112 HDR motion patches
nobody references a foreign fighter's `.nuanmb`" is true and **vacuous**: `Animation.name` in
`motion_list.bin` is a bare filename carrying no fighter component at all, so it *cannot* name a
foreign fighter and equally cannot fail to. The directory comes from the file-level `motion_path`,
and **vanilla itself ships cross-fighter redirects** — 35 of 1,239 dumped lists disagree with their
own location, and `boss/playable_masterhand/motion/body/c00/` contains only a `motion_list.bin`,
naming six animations that exist solely under `boss/masterhand`. But `motion_path` is one field
per *file*, so redirecting it moves all ~700 entries at once. **Per-move cross-fighter animation
is not expressible in `motion_list.bin`**, which is why the ARCropolis route — host path, foreign
bytes — is the only per-move option.

**What CSK actually is:** mostly plugin behaviour plus assets, not a bundle of share declarations.
Which matters, because ADR 0002's argument for `sys_*`-only is a **residency** argument, and
splicing a FileInfo into the host's DirInfo is precisely what defeats a residency problem. Whether
that extends to effect *archives* — `.eff` containers rather than single files — is **untested**;
the community's own article-cloning guide copies effect files rather than sharing them.

**The experiment, one deploy and one boot:** make Chrom's jab animation a share instead of a copy.
It is the single file where a correct result is provably indistinguishable from today's build, so
the jab either looks unchanged or it does not. It also exercises the `reshare_file` branch (target
path exists in vanilla) which this project has never run — only `add_shared_file` — and settles the
open `BACKLOG.md` item asking whether `share-to-added` resolves a vanilla source at all. Three
existing instruments read the result with no new code: the self-test's RESOLVED line for
`attack_11`, `conflicts.json`, and the Skyline log.

## What The CSK Collection actually provides (5.0.6, read locally)

One 1.4 MB plugin, 86 hooks, 121 defined symbols of which ~40 are callable API. It rebuilds all
ten `ui/param/database/*.prc` files at runtime, and `disable_save_no_checks` is **always on** with
no flag — which is what lets an appended row survive validation.

### There is a declarative path, and it covers characters

CSK scans **`mods:/database/`** with `nn::fs::OpenDirectory` and feeds every file to
`serde_json`. There is no extension filter — the filename is arbitrary. The `Config` struct has 17
keys, and the **first two are `character_database_entries` and `layout_database_entries`**:

```
character_database_entries   layout_database_entries    series_database_entries
bgm_database_entries         stream_set_entries         assigned_info_entries
stream_property_entries      bgm_property_entries       stage_database_entries
gametitle_database_entries   tips_database_entries      amiibo_database_entries
mii_body_database_entries    mii_hat_database_entries   playlist_entries
stage_resource_redirection_entries                      fighter_jingle
```

`CharacterDatabaseEntrySerde` carries ~40 fields, and five are not vanilla `ui_chara_db` columns:
**`is_patch`** (declare the entry a patch rather than an append), **`color_num`**,
**`marker_file`** and **`marker_fighter_folder`** (the `.marker` convention as data rather than as
plugin code), and `extra_hash_maps`/`extra_index_maps` as generic escape hatches.

A second file completes it: **`mods:/flags/reslot.toml`**, parsed into
`CharacterInfo { color_start_index, color_num, original_ui_chara_id }` — exactly the triple the
Thwomp author documents in prose, in declarative form.

**The caveat that keeps this from being settled:** no shipped mod in hand uses
`character_database_entries`. Waluigi, Thwomp, Herobrine and Gogeta all take the plugin route, and
`allow_ui_chara_hash_online` has no `Config` counterpart — so the data path exists in the code and
is not proven complete.

### Why these mods require CSK — and the answer is "one call"

Investigated because every one of them lists CSK as a hard dependency. The causal story is simple
and the READMEs obscure it.

**CSK is needed if and only if you add a row to the CSS.** Across the four slotted characters the
intersection is four symbols, and the strictly necessary set is much smaller:

| | status |
|---|---|
| `add_chara_db_entry_info` | **required, READ.** Appends the `ui_chara_db` row. Nothing else can. |
| `add_chara_layout_db_entry_info` | **very likely required.** A separate registration queue, so the first call does not create a layout row implicitly; 4 of 4 mods call it. Whether the CSS crashes or merely mis-positions without it was not established. |
| `add_narration_characall_entry`, `set_fighter_jingle` | cosmetic — announcer name and victory jingle |
| the 5-call BGM/stream block, `allow_ui_chara_hash_online`, `add_tips_db_entry_info`, `play_bgm`, `add_series_db_entry_info`, `change_entry_chara_ui`, `get_ui_chara_from_entry_id` | **provably optional by counterexample inside the sample** — Thwomp ships with no BGM block and no online permission at all |

So the dependency is **one confirmed call, two in practice — not thirteen.** Thirteen is Gogeta's
total, and eleven of those are music, series art, announcer, jingle, online, and its
transformation gimmick. (Gogeta registers *five* `chara_db` entries — one per form — and calls
`change_entry_chara_ui` from an in-battle hit handler to swap the HUD icon on transform.)

**That strengthens decision 28 considerably.** The declarative path only has to cover
`character_database_entries` and `layout_database_entries` — which happen to be the first two keys
of CSK's `Config` schema. The experiment is therefore narrower than "does the JSON route work at
all": it is "do those two keys replace those two calls".

### Two findings that correct the ecosystem's own claims

**Sonic Re-Imagined has no CSK dependency whatsoever.** Zero CSK symbols — not by PLT, not by GOT,
not by `LookupSymbol` — across all six of its plugin variants, and nothing in its asset tree that
CSK consumes: no `flags/`, no `mods:/osvt`, no narration directory, no `.kime`, no database file.
Its "slotting through the CSK Collection" is self-implemented: the plugin builds
`mods:/fighter/sonic/motion/body/cNN/` itself and probes its own `sonic.battle` / `sonic.modern`
marker files through an ARCropolis event callback.

CSK is nonetheless **operationally** mandatory as shipped, because the author's boilerplate checks
five plugin files with `nn::fs::GetEntryType` and, on any miss, sets a flag that makes `main`
branch straight to its epilogue — so **every hook and every smashline install is downstream of
that check** and the mod silently does nothing. The README overstates a dependency the code does
not have, and the guard makes the overstatement self-fulfilling.

**George is the control case and confirms the model.** Zero CSK anything, no dependency guard, and
only the non-`_costume` smashline installers — consistent with replacing Diddy agent-wide.
Replacing an existing agent needs smashline and nothing else.

**The lesson the builder should take** is about the guard, not about CSK: a generated mod should
check for exactly the plugins it actually calls into, and no more. An over-broad guard turns a
missing optional dependency into a mod that appears installed and does nothing, which is the
hardest failure mode to diagnose from the outside — and this ecosystem has it shipped, twice.

### Flags are global, not per-mod

`mods:/flags/*.flag` resolves on ARCropolis's *merged* filesystem, so any enabled mod dropping the
file turns the feature on process-wide. All flag files are zero bytes; only existence is tested.
Seventeen exist, of which two — `enable_stage_redirection` and
`disable_victory_cam_unsharing` — ship no mod folder and must be created by hand.

### CSK does not touch effects, and the alternative has a price tag

An exhaustive search of all 1,425,408 bytes for `effect`, `.eff`, `ef_`, `EffectModule` and
related tokens returns **one** hit: `effect_load_group_hash`, a field of the *stage* redirection
table. "Enable One Slot Series" is about UI series icons — its neighbours are
`set_chara_db_ui_series_id_lookup` and `results_screen_set_series`. The package ships one NRO and
it is not `libone_slot_eff.nro`.

**So ADR 0002's premise survives**: nothing here makes a foreign fighter's effect archive resident.

And the shipped alternative is not "reference the source's effects" — it is **merge them into the
host's own archive**. Gogeta ships eight `effect/fighter/dolly/ef_dolly_c1NN.eff` files at
**21,672,480 bytes each — 173 MB** — which are the host's archive with the imported effects added,
selected per slot by the separate **One-Slot Effects** plugin. Two of the four mods depend on it.

That reframes decision 4: what this plan calls a "CSK build" is not a CSK feature at all. It is
One-Slot Effects plus a ~21 MB rebuilt host archive per slot.

## The corpus: one private repo, or build it locally?

Both, and they are not alternatives — they are two implementations of one interface.

**The constraint that decides it:** decision 1 says anyone can use the tool, and a private repo
cannot serve anonymous users. So a private corpus repo cannot be the distribution path. It can be
the *development* path, and there it is plainly better than what exists now.

So the generator codes against the Game Database interface and never learns which is behind it:

| | dev corpus | user corpus |
|---|---|---|
| shape | one private repo, cloned | built on first run |
| contents | complete, including the decompile | fetched + extracted |
| gets us | reproducibility, a new machine in one clone, and the decompiled C the transpiler in decision 11 has to read at generation time | no redistribution of anything Nintendo owns |

**What belongs in the private repo is narrower than "all game information", and the rule is: store
what is expensive to ACQUIRE, not what is expensive to COMPUTE.**

| | size | route | store it? |
|---|---|---|---|
| `vendor/main/*` — `.text`, `.rodata`, `.data`, symbols | 97 MB | a console dump: flip `ENABLED` in `exedump.rs`, deploy, load a match, pull | **yes** — irreplaceable without the console, and a nuisance every time |
| `work/emu/*` — 39,213 constants, the const table, 100,539 entry points | 11 MB | Unicorn, locally | **yes** — tiny, and derived from the above |
| the 8 Nintendo asset files currently tracked in `SSBUMods` | small | — | **yes**, and moving them is what would let `SSBUMods` go public |
| `work/ghidra/main-all-100k.c` + `main-ind-31.c` | **364 MB** + 165 KB | 15 minutes of Ghidra from `vendor/main`, plus 3 seconds for the 31 | **no** — cheaper to rebuild than to store, and see the file-size limit below |
| `vendor/SSBU-Dumped-Scripts`, `vendor/ssbu-status-all`, `vendor/refs` | 493 MB | public community repos, already scripted (`fetch-refs.sh`, `fetch-status-scripts.sh`) | **no** — storing a copy of a public repo in a private one is pure duplication |
| `vendor/decomp/**` — 18 module trees, incl. all 8,792 functions of `lua2cpp_common` | 263 MB | Ghidra, ~10 min per module from the NROs | **no** — derived, same rule as above |
| `extracted/**` | **2.3 GB** | arc-network, game running | **no** — too large, and per-host anyway |

Two hard practical limits make the maximal version impossible rather than merely unwise. **GitHub
refuses any file over 100 MB**, so the 364 MB decompile cannot be pushed at all without splitting
it or paying for LFS (whose free tier is 1 GB). And the corpus measures **~3.5 GB** in total
against a 1 GB repo recommendation. The narrow version — `vendor/main` plus `work/emu` plus the
tracked assets — is about **110 MB**, every file under the limit, and it carries exactly the
things that cost a console session to get back.

**Side benefit worth naming:** this is also the cleanest route to `SSBUMods` being publishable.
The 8 tracked Nintendo files are the only reason it cannot be, and they would live in the corpus
repo instead.

## Superseded: the original open questions

1. **Who operates it, and does that change the class-3 answer?** If the answer is "only me and a
   model", the scaffold is enough. If it is "other modders", the scaffold is a trap and class 3
   has to be either solved or refused outright.
2. **Arbitrary host, or a host profile?** A host with no `HaveR`, or fewer articles, or a
   radically different skeleton, breaks resolvers that Pikachu happened to satisfy. Is the tool
   allowed to say "this host cannot take this move"?
3. **Articles: solve, or declare?** 89 of 97 fighters own one. Solving means root-causing the
   `clone_weapon` crash inside the game's own article init. Declaring means most projectile and
   weapon moves ship without their object.
4. **Is the Pikachu port the regression suite, or the first customer?** Regenerating it from the
   builder would prove the builder and risk a working mod. Not regenerating it means the builder's
   only validation is moves nobody has playtested.
5. **How much does a move have to work before the builder claims success?** Completeness is
   checkable offline; correctness needs hardware. What is the release bar for a generated port?
6. **Whole-moveset coherence.** The Pikachu project chose its 25 moves for synergy, and ADR 0001
   exists because a move can carry a property that belongs to its fighter. Does the builder do one
   move at a time, or does it have an opinion about the resulting character?
7. **What happens to `pikachu-moveset/`?** Does it become the builder's first configuration file,
   or stay a hand-written project the builder is validated against?
