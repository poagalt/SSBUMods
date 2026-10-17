# Traps

Each line below cost at least one round. Several cost several. They are grouped by where they
bite, not by when they were found.

## The engine

| Trap | What it looks like |
|---|---|
| `Main` runs **once** at status entry | a per-frame hook written on `Main` is simply inert; per-frame logic is `main_loop` via `sub_shift_status_main` |
| A status has lines you did not register | the carry walked `+0.00` units with the animation frame pinned at 0, because DK's `SHOULDER_WALK` has an `Exec` line and the port had none |
| `init_settings`' kinetic type is overridden same-frame | a faithful transcription of the first call that is still wrong |
| A common status is not fighter-agnostic | `status_SwingGaogaenThrown` reads its linked parent's position with no null check; no rope means a null pointer and a Data Abort at fault address 0 |
| `LinkModule::unlink` on an empty link | crash, inside `cross2 + 0x47f4a0`. Explains two separate crashes |
| Forcing a victim into the status it is already in | crash — re-entry re-runs the status entry path |
| `CatchModule::catch_cut` is a **request**, not an action | `this->[0x93] = 1`, consumed only if a catch is active. Raised with nothing to cut, it survives to the **next** catch — the "cannot grab again until the game's own reset" bug. A save-state restore does not clear it |
| `capture_cut`'s gate is `p2 && p3 && !p4` | `(true,false,false)` and `(true,true,true)` both miss the teardown; `(true,true,false)` is the only combination that reaches it, and it was never tried for five rounds behind a comment saying the gate was `p2` alone |
| `CaptureModule::is_capture` is true on the **captor** too | a "who is holding whom" test that reads it on one side answers the wrong question |
| `AttackModule::is_attack_occur` is **sticky for the whole move** | Revenge spent its damage boost on its own counter-hit, twice, until the read became a rising-edge latch |
| INT and INT64 work ids are **separate arrays** | `set_int64` on an INT id writes the wrong variable *self-consistently*: the write and its read-back agree, and the engine sees nothing |
| A status-scoped work id is an **array index** | `..._STATUS_SPECIAL_S_FLOAT_ROPE_X = 0x01000007` means "float 7 of the current status". Answering it globally fed a world coordinate into every other status's float 7 — "pikachu running really fast", erratic, surviving resets |
| Instance work ids collide across fighters | every fighter numbers its own from `FIGHTER_INSTANCE_WORK_ID_FLAG_TERM = 0x200000E0`, so Incineroar's flag 7 **is** Pikachu's `FLAG_PAUSE` |
| `get_param_float(boma, object, key)` — the first hash is the **object** | passing `(name, 0)` returns 0 silently, and a `.max(0.1)` floor downstream then hides it |
| MOTION kinetic energy is stateful | `reset_energy` must precede `set_speed`, or the write lands on stale state |
| `sv_kinetic_energy::*` are lua-stack calls | invisible to function hooks, so a runtime recording will never show them |
| `cancel_frame` cannot **lengthen** a move | the attack status ends when the animation ends; use `FT_MOTION_RATE_RANGE` and reset the rate to 1.0 |
| `FT_MOTION_RATE(r)` is game frames per **animation** frame | `0.25` is four times faster, not four times slower. Reconciles a script's frame 8 with a frame-data site's frame 3 |
| `ATTACK_ABS` / `ATK_HIT_ABS` are not positional | every instance in the corpus is a THROW or CATCH kind with no position arguments |
| `|x| <= size` or the hitbox cannot connect | it has to intersect the plane the fighters stand on; 14 of 53 capsule far ends were dead geometry in the background |
| A grafted bone does not mirror | a hitbox on a bone the host does not really have works in one facing only |
| `.bss` is **sparse** | reserved address space with only touched pages backed; a linear read faults. `Region::Bss` also returns the page-aligned *end*, not the start — MOD0+8 and MOD0+0xC are the authority |
| Map-alias IPC | Horizon transfers large IPC buffers by remapping the sender's pages **out of its own address space**. Handing `from_raw_parts` pointers into the game's mapped image to a socket write makes the game's `.data` briefly vanish from itself. Always `copy_from_slice` to a heap buffer first |
| A Skyline plugin loads at **game launch** | deploying to a running game changes nothing, and a whole recording session was lost to not saying "restart Smash" |
| SD throughput through Skyline's fs layer is ~300 KB/s | a 1 MB `write_all` holds the card about 3 seconds; a 22 MB synchronous write on the game thread during load crashes it |

## The port's own plumbing

| Trap | What it looks like |
|---|---|
| A synthesised motion entry names **four** scripts | `synth` repointed `game_script` only, so an up-B entry cloned from a Quick Attack template still named `expression_specialairhi1` — and Pikachu's own script calls `VisibilityModule::set_whole(false)`. The up-B was invisible, hidden by his own code running under Game & Watch's animation |
| A dropped-in animation carries a track per **source** bone | 77 of Incineroar's 123 transform tracks are for bones Pikachu does not have, and all 57 visibility tracks name meshes he does not have. Untrimmed, the set ran 38% over the fighter's memory budget |
| The over-budget symptom is **nondeterministic** | motions resolved 48/77 in one load and 77/77 in another *off the same files*. Identical files cannot fail differently, so "it worked last time" is not evidence. Trimming took 3,673 KiB to 637 KiB and moved zero hitboxes |
| A param provider must coerce like the engine | `special_s_lariat_offset_x1` is stored as an **int** (−24) and the source reads it with `get_param_float`. The engine coerces; a hand-written provider that searches only its float table misses the key, falls through to the engine, and a host with no such param object gets **0**. Five whip values were silently zero, including the lariat's motion rates — a rate of 0 is a frozen animation |
| Article ids collide across fighters | `FIGHTER_ELIGHT_GENERATE_ARTICLE_ESWORD = 0x0` and `FIGHTER_PIKACHU_GENERATE_ARTICLE_DENGEKIDAMA = 0x0` are the same number, so a guard meant for Mythra's sword tested for Pikachu's electro ball |
| `.prcx` and `.prcxml` patch the same file | the reference mod ships a **binary** `fighter_param.prcx`; this project ships `.prcxml`. Establish which ARCropolis honours before concluding a param change is applying — measuring the live values on the first frame of a match is the way that settles it, and it is how "none of the attributes are applying" was found |
| `arc_network` unwraps its `bind` in a loop | killing the extraction client mid-transfer leaves port 43022 in `TIME_WAIT`, so the next bind panics with `EADDRINUSE`. Only a full console reboot clears it |
| A log sink that probes with `fs::write` truncates | every launch erased the previous session's log, so a crash's evidence was gone by the time it was read — and the absence was mistaken for "the code never ran". Rotate to `.prev` and append |

## Diagnosing from absence

Four claims in this project's history were built on something *not* appearing, and each was wrong:

- "`End` never fires for these statuses" — the log had 129 End-line hits; the real cause was an
  invented `is_capture` guard.
- "The early-press branch isn't in the recording" — the hooks covered capture-state calls only,
  and that branch touches none of them.
- "`is_catch` never fires" — every line of the cited log stopped one frame before the window
  opened.
- "Pikachu's own specials are unreachable because nothing requests those statuses" — an argument
  about absent code that had already been wrong twice. Seven retired status kinds now bounce to
  wait/fall and **log loudly**, which turns the claim into a measurement.

The rule: an instrument that cannot see something is not evidence that it did not happen. Before
concluding from silence, check that the instrument covers the thing.

## Reading a crash report

**The Atmosphère backtrace walks the frame pointer, not the link register.** A function that never
establishes a frame does not appear at all, and Rust release builds are full of those — leaf
functions, outlined prologues and epilogues, and anything LLVM tail-called. So
`ReturnAddress[00]` is *the nearest return address belonging to a function that kept a frame*, not
the caller of the faulting code.

Worked example: an `Instruction Abort` naming `rat_of_doom + 0x9cf8` at frame 0 was not our code
branching to garbage. `sub_shift_status_main` ends in a real `blr x9` and does keep a frame, so a
fault inside the frameless status loop it calls reports *its* caller — ours — and omits the two
frames between.

The signal is **`ReturnAddress[00]` plus the fault type**. A Data Abort with a small fault address
is a null field read and the top frame usually is the culprit. An Instruction Abort is a branch to
something that is not code, and the culprit may be several calls above.

Two riders: Rust strips symbols, so identify your own code by its **strings** (find the
`adr xN, $+…` that points past `.text` and read that offset out of the file — a format string is
unique to one function); and an offset means nothing without the build, since the `.nro` was
159,744 bytes when one report was written and 167,936 two commits later.

## Tooling and the shell

| Trap | Consequence |
|---|---|
| `cargo skyline build \| grep "^error"` | cargo emits ANSI colour codes first, so a build failing with five type errors printed nothing. Gate on the exit code **and** on the `.nro` being newer than every source file |
| `cargo: command not found` inside a tool call | a silent false "clean build". Twice |
| Running a **prebuilt** codegen binary | a source change to `synth.rs` silently no-opped. Tools are rebuilt as build step 0 for this reason |
| Hand-editing an `@generated` file | the build regenerates over it and the commit contains none of the lines. "build ok" means it compiled, not that the edit survived |
| `topscale` is not idempotent | ran twice, scaled by 1.158². It now refuses any file without the `@generated` banner |
| Backticks inside a double-quoted shell string | command substitution. Bit this project **four** times, twice in list strings and twice in commit messages |
| `perl -0pi` / `sed -i` on scripts | ate `$` variables, executed backticks, wrote literal `\n`, and turned one script into a binary file. Use the editing tools or a heredoc |
| Apostrophes in a heredoc or an awk comment | "Mac's" terminated the script |
| `printf "$var" \| grep -q` under `pipefail` | SIGPIPE, reported as failure |
| `grep -c … \|\| echo 0` | prints `0\n0` |
| `tr -d '0x'` | deletes every `0` and every `x` in the input |
| `tr -d ' '` on `pid timestamp` | mangles it into one token and the lock reads as stale |
| `awk -F'\t'` on a space-aligned TSV | every row reports missing |
| `format!("{}", 0.0f32)` | prints `0`, an integer literal the `ATTACK` macro rejects. `{:?}` prints `0.0` |
| `$$` in Git Bash is an **MSYS pid** | `tasklist` and `ps -W` cannot see it — `ls -d /proc/<pid>`. Reported "no processes left" while pid 150 was alive |
| MSYS rewrites POSIX-looking arguments | `--contains /c00/` became `C:/Program Files/Git/c00/`. Needs `MSYS2_ARG_CONV_EXCL='*'` |
| w64devkit's `grep` returns 0 for patterns containing `/` | put w64devkit **last** on `PATH` |
| `grep` under `set -e` | killed the build three separate times. `|| true` at each call site |
| Piping a long job through `grep`/`tail` | buffers everything, so a finished 4-minute job looks like a 12-minute hang, and an `until grep -c OK` loop waits forever on a file that never receives the lines. **Write to a file and read the file** |
| Chaining a deploy into anything | `deploy.sh:84` forbids it. Paid twice: a six-hour hung task, and a stale lock that silently skipped the revert of a **crashing** build |
| A shell name clash with a stdlib module | a scratch script named `dis.py` broke `import capstone`, which imports `inspect`, which imports `dis` |

## The console side

- **sys-ftpd truncates ~1 upload in 80 to zero bytes with a clean return code**, and truncates
  `LIST` responses under load, so directory listings are unreliable. A targeted `curl -sI`
  `Content-Length` is authoritative. Three consecutive deploys shipped zero-byte files before
  size-based verification and a repair pass existed; those produced three false playtest reports.
- **A manifest recorded at verification time** skips a file rebuilt between upload and check.
  Record it at upload time.
- **`ftpq.sh get` cannot be used on a binary.** It returns the file through a shell command
  substitution, which **silently strips every NUL byte** — a 245,760-byte plugin came back as
  175,923 with a one-line `warning: command substitution: ignored null byte in input` that is easy
  to read past. Printable strings survive, so it is fine for reading a build stamp out of an
  `.nro`, and useless for comparing sizes or hashes. Use `ftpq.sh size` for that, which is the
  authoritative query anyway.
- **`work/.deploy-manifest` is not a "last deploy" timestamp.** Its mtime has been Sep 11 for
  days; the manifest format changed and the live one is written elsewhere. Read the newest
  `work/deploy-*.log` instead — and read the whole thing, because "nothing was uploaded, all 86
  files unchanged since the last verified deploy" is the **mod folder's** verdict and sits below a
  separate `== deploying plugin ==` section that did upload.
- **Dot-prefixing a folder does not disable an ARCropolis mod.** `conflicts.json` proved it. Move
  the folder out of `ultimate/mods/`.
- nxdumptool needs `sdmc:/switch/prod.keys` (Lockpick_RCM); RomFS lives inside encrypted NCA
  containers. Without those keys the running game is the only route to `data.arc`.

## The meta-trap

The most expensive pattern in this project's history was not any single bug. It was: form a
hypothesis from params, scripts, a wiki, or reasoning; implement it; ship it; learn one bit —
"still wrong" — and start again. **One bit per round against a hypothesis space that was not
small.** The fix was not more care. It was an instrument that yields the whole answer in one
round: a per-frame probe printing every input the decision reads, for every object, on every frame
of the window. One playtest then contains the answer instead of a bit of it.

The corollary, from the same experience: **when a bug survives two fixes, stop proposing a third
and ship a probe that prints the real state at the real moment.**
