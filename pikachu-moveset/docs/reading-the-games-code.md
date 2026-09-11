# Reading the game's code

Everything in `vendor/` is gitignored, so this is the versioned copy of what is in there, how it
was obtained and what each source is good for. If `vendor/` is empty, this page rebuilds it.

## The four sources, in order of how much they settle

### 1. `lua2cpp_common.nro` — every status the roster shares

    printf 'prebuilt:/nro/release/lua2cpp_common.nro\n' \
      | tools/arc-pull/target/release/arc-pull.exe <console-ip>:43022 vendor/nro

8466432 bytes, 5428 imported PLT stubs, **8792 defined symbols**. Needs the game running with
the arc_network plugin loaded — the copy on the card is encrypted and decrypting it needs
prod.keys we do not have, so the running game is the only source.

This is where every VICTIM-side status lives. A command grab puts its victim into a common status
— `SWING_GAOGAEN_CATCHED`, `SHOULDERED_DONKEY`, `CAPTURE_PULLED` — and none of those appear in
any fighter's own agent, which is why three rounds of "the grab crashes" could not be explained
from the fighter NROs. See ADR 0004.

### 2. `lua2cpp_<fighter>.nro` — that fighter's own statuses

Same pull, `lua2cpp_gaogaen.nro`, `_donkey`, `_gamewatch`, `_packun`, `_pikachu`. Fighter STATUS
logic is not in `data.arc`'s params, animations or ACMD scripts, and it is not in `main` either;
`main` holds only the engine (`app::lua_bind::*Module__*_impl`). It is in these.

All five pulled builds report `patch-13.0.1` even on a 13.0.5 console, which is why the public
decomp of 13.0.1 matches them exactly.

### 3. The public decomp — the same NROs, as readable C

    git clone --filter=blob:none --no-checkout --depth 1 \
        https://github.com/Coolsonickirby/SSBU-Dumped-Status-Scripts
    git sparse-checkout set ghidra_scripts/lua2cpp_gaogaen.nro ...   # 95 fighters available

Ghidra C output produced by blu-dev's `Lua2CPPAnalysis.java`, which recovers status names by
walking the agent vtable (slot 9 is `SetStatusScripts`). **Fighter agents only — there is no
`lua2cpp_common` in it**, which is exactly the gap source 1 fills.

It also omits every unnamed `FUN_*` helper, and those are where the interesting parts hide. Four
mattered and all four are now read, by source 4.

### 4. `tools/disasm` — the NROs with every call named

    disasm <nro> --sym <substring>      find symbols
    disasm <nro> --calls <addr> [n]     just the call targets, in order
    disasm <nro> <addr> [n]             annotated disassembly
    disasm <nro> --header

Pure Rust (yaxpeax-arm), because w64devkit's objdump has no aarch64 support and there is no
Ghidra on this machine. It walks MOD0 → `.dynamic` → `.rela.plt`, matches each PLT stub to its
JUMP_SLOT relocation, and prints the imported symbol beside every call — which is what makes a
hundred `bl $+0x117e34` lines into something readable.

Addresses map straight through: Ghidra loaded these at `0x7100000000` and `.text` starts at file
offset 0, so `FUN_710001ff80` is file offset `0x1ff80`.

What it answered, each of which had been guessed at wrongly before:

| question | answer |
|---|---|
| how does Incineroar detect the Alolan Whip's catch | he does not. `--sym catch` finds five symbols in the whole NRO and none is `CatchModule`; the engine does it in native code keyed on the fighter kind |
| `FUN_710001ff80` (whip, per-frame) | air control: `is_flag`, `reset_energy`, `controller_set_accel_x_mul/add`, `set_stable_speed` |
| `FUN_710001d4b0` (whip) | the ground/air motion switch |
| `FUN_7100013e50` (Revenge, both statuses) | `get_sum_speed_x/y`, three `get_param_float`, `change_kinetic(UNIQ)`, `clear_unable_energy` twice, `reset_energy`/`set_speed`/`set_accel` on MOTION, `enable_energy` |
| `FUN_7100022520` (DK cargo) | the throw input: `get_clatter_time` on the victim, cat2 masked against four throw flags, `IsThrowStick`, cat1 buttons, five `change_status` |

### Reversing a hash40 back to a name

The disassembly gives 40-bit constants, not names. `tools/motion-patch --bin h40` hashes forward;
to go backwards, hash every token in `vendor/param-labels/ParamLabels.csv` and match. That is how
`special_air_lw_hit_accel_y` was found — a param Revenge reads that nobody knew was involved,
because its name had never appeared anywhere.

## Other people's mods

`vendor/hdr` is HewDraw Remix, the largest smashline gameplay mod, sparse-checked-out to
`fighters/` for the characters this moveset borrows from. It is the best available answer to "is
this how you are supposed to write it": it confirmed the `sub_shift_status_main(L2CValue::Ptr(..))`
idiom, and its `energy/motion.rs` decodes `FighterKineticEnergyMotion` well enough to explain why
`reset_energy` must precede `set_speed`.

**It is a rebalance mod, so its values are not the game's values.** Its `gaogaen`
`special_lw_pre` passes `FIGHTER_STATUS_ATTR_START_TURN` where vanilla passes nothing. The rule
is HDR for *how*, the decomp for *what*.

## The discipline this replaced

Revenge was implemented wrongly five times, the whip's catch three times, and the reason was the
same each time: the logic was being reconstructed from params, ACMD scripts, wiki pages and
reasoning, and **none of those contain it**. Every one of those rounds produced a plausible
implementation and a playtest report saying it was wrong.

So: read the code first. When the code and a wiki disagree, the code wins. When the decomp and
another mod disagree, the decomp wins. When a claim in a comment says "measured", check that a
log actually contains the measurement — one such claim asserted `is_catch` never fires, and every
line of the log it referred to stopped one frame before the window opened.
