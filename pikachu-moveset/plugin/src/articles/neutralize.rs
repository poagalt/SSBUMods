//! Per-INSTANCE neutralisation of the vtable slots that make a cloned article unsafe.
//!
//! WHAT IT FIXES. A cloned weapon runs its donor's native code, and two of the articles this port
//! wants read their donor's `FighterParamAccessor2` entry unguarded: the rope (slots 40 and 55 read
//! GAOGAEN's) and the spikeball (slots 3 and 55 read PACKUN's). That entry only exists while the
//! donor is in the match, so the clone null-derefs -- the spikeball did, ten boots out of ten.
//! docs/the-article-path.md derived the fix and left it "designed, not written": give OUR
//! instances a private copy of the class vtable with exactly those slots pointed at the engine's
//! own defaults, which 552 (slot 40) and 442 (slot 55) and 412 (slot 3) of the game's weapon kinds
//! already use, while every real Incineroar's rope keeps the class's own.
//!
//! EVERY ADDRESS BELOW WAS READ OUT OF THE GAME, 2026-10-12, through the code index
//! (handbook/lookup.md), and tools/check-article-donor-params.sh re-checks them against
//! work/rea/main.elf on every run:
//!
//! - `WEAPON_INITIALIZE` is a weapon's initialize-with-kind virtual. It is a slot of the weapon
//!   base class's vtable in `.data` (the pointer sits at main+0x51863c8, beside the next virtual's
//!   at +0x51863d0), so pooled objects pass through it every time they are given a kind. Its
//!   arguments are (object, battle_object_id, kind, entry_id, ...) -- it forwards them to the base
//!   initialiser, which stores them at +0x8, +0xc and +0x10, the crate's own `BattleObject` layout --
//!   and near its end it stores `WEAPON_CLASS_TABLE[kind]` at `object + SPECIALIZER_AT`.
//! - The dangerous slots are first called in the NEXT virtual (the slot-3 call is at
//!   main+0x33a07b8, inside the function at 0x33a0490), so patching after the original returns is
//!   early enough. The only slot used inside initialise is 85, which the rope shares with 611 kinds.
//! - A class "singleton" is a stateless 8-byte object whose one word points 0x18 bytes later, at
//!   the vtable's address point; the two words between are offset-to-top and a null RTTI pointer,
//!   and the 114 slots follow (0x390 bytes, exactly the spacing of neighbouring singletons). So a
//!   private copy is an 8-byte object pointing at a copied group, and carries no hidden state.
//!
//! WHO GETS IT. A weapon whose kind is a NEUTRALIZED row's kind, whenever that row's DONOR has no
//! param entry (see `donor_absent`) -- the exact condition under which the class crashes, whoever
//! owns it -- or, with the donor present, when the owner entry is a Rat of Doom. The owner test
//! alone never matched at match load (2026-10-18), because the entry's fighter is not answerable
//! while the fighter's objects are still being built.
//!
//! THE ROPE ALSO GETS A GUARDED SLOT 3 (`rope_slot3_guard`): its module start resets its camera
//! ranges through a pointer computed from Incineroar's param data, which is null+0x108 without him.
//!
//! NOTHING HAPPENS UNLESS A NEUTRALIZED ROW IS ENABLED IN CANDIDATES: `install` does not even
//! install the hook otherwise. Both rows are enabled now -- the rope since 2026-10-17, the
//! spikeball for Ptooie on 2026-10-18 -- and each was armed as its own watched boot; the first
//! boot's log says, once per kind, which entry and which slots were neutralised, so a crash can be
//! attributed.

use smash::app::{lua_bind::{FighterEntry, FighterManager}, sv_battle_object, FighterEntryID};
use smash::lib::lua_const::*;

/// Host names of CANDIDATES rows that may be installed although the audit calls them dirty, and the
/// vtable slots that make them so. tools/check-article-donor-params.sh parses this and FAILS unless
/// every slot that points at an unguarded donor read is listed -- and warns for any listed slot that
/// is not one, because neutralising a clean slot throws away the article's real behaviour.
pub const NEUTRALIZED: &[(&str, &[usize])] = &[
    ("rope", &[40, 55]),
    ("spikeball", &[3, 55]),
];

/// THE HOOK SITE, PER GAME VERSION, AND IT IS VERIFIED BEFORE IT IS USED.
///
/// The weapon initialize-with-kind virtual is main+0x339edc0 on 13.0.5 (the dump this module was
/// written against) and main+0x339e810 on 13.0.4 -- the 0x5B0 bump that runs through this region of
/// `main` (smashline's own 13.0.5 commit adds `+ 0x5B0` to its offsets here), and the vendored
/// 13.0.4 decomp has a 5,832-byte function starting at exactly 0x339e810. Console #2 runs 13.0.4.
///
/// A WRONG ADDRESS IS A CRASH AT MATCH LOAD, so neither is trusted: `install` reads each
/// candidate's instructions out of the running game and hooks only the one that matches the
/// 13.0.5 function -- its first twenty instructions with the two branch-link targets masked, plus
/// the `str x0, [x19, #0x3bb0]` that stores the class singleton (at +0x1550 in 13.0.5). If neither
/// matches, nothing is hooked, `active()` stays false, and the rope is never spawned.
const INIT_1304: usize = 0x339e810;
const INIT_1305: usize = 0x339edc0;
const SIGNATURE: [u32; 20] = [
    0xd10143ff, 0xa9015ff8, 0xa90257f6, 0xa9034ff4, 0xa9047bfd, 0x910103fd, 0xf9401018, 0xaa0403f5,
    0x2a0303f6, 0x2a0203f4, 0xaa0003f3, 0x2a0103f7, 0x94000000, 0x37000060, 0x320003e8, 0x3900c308,
    0xaa1303e0, 0x94000000, 0xaa1303e0, 0x2a1703e1,
];
/// Words 12 and 17 are `bl`, whose target differs between versions: compare the opcode only.
const SIGNATURE_MASK: [u32; 20] = [
    !0, !0, !0, !0, !0, !0, !0, !0, !0, !0, !0, !0, 0xfc000000, !0, !0, !0, !0, 0xfc000000, !0, !0,
];
/// `str x0, [x19, #0x3bb0]`.
const STORE_SPECIALIZER: u32 = 0xf91dda60;
/// Where a weapon object keeps its class singleton.
const SPECIALIZER_AT: usize = 0x3bb0;
const VTABLE_SLOTS: usize = 114;
/// offset-to-top and RTTI, before the address point.
const VTABLE_HEADER_WORDS: usize = 2;

/// THE ENGINE'S DEFAULT FOR EVERY NEUTRALISED SLOT IS A BARE `ret`, measured: in the 13.0.5 dump the
/// majority implementation of slot 3 (main+0x33be810, 347 of 522 vtables), slot 40 (main+0x33be9b0,
/// 477) and slot 55 (main+0x33bea70, 381) is the single instruction `d65f03c0`. So the private
/// vtable points those slots at THIS function, which is the same thing -- and that is why this
/// module no longer needs the weapon class table's address or a clean donor's vtable, the two
/// version-pinned reads the first version made.
extern "C" fn default_slot() {}

static mut PRIVATE: [usize; 2] = [0; 2];
static mut LOGGED: [bool; 2] = [false; 2];
static mut ACTIVE: bool = false;

/// True once the weapon hook is installed on a verified site. Nothing may spawn a neutralised
/// article unless this is true.
pub fn active() -> bool {
    unsafe { ACTIVE }
}

pub fn covers(host_name: &str) -> bool {
    NEUTRALIZED.iter().any(|(h, _)| *h == host_name)
}

/// The NEUTRALIZED rows that are actually installed, with the weapon kind each one clones.
fn live_rows() -> impl Iterator<Item = (usize, i32, &'static [usize], &'static str)> {
    NEUTRALIZED.iter().enumerate().filter_map(|(i, (host, slots))| {
        let c = super::CANDIDATES.iter().find(|c| c.host_name == *host)?;
        if !(c.enabled && super::CLONE_ENABLED) {
            return None;
        }
        super::weapon_kind(c).map(|k| (i, k, *slots, *host))
    })
}

/// Is the fighter in this entry slot a RAT OF DOOM -- a Pikachu on the gated costume?
///
/// THE COSTUME TEST IS HERE AND NOT IN THE HOOK because the hook runs once per weapon spawn for
/// every object in the fight, and this is already the narrowing test. `WEAPON_INITIALIZE` is a
/// `skyline::hook`, so smashline's `*_costume` filter -- which gates every `Agent` this plugin
/// registers -- does not reach it; without the test, a vanilla Pikachu on c01-c07 would have his
/// article's specializer vtable swapped for a private one, for a moveset he is not running.
unsafe fn owner_is_pikachu(entry: u32) -> bool {
    if entry >= 8 {
        return false;
    }
    let mgr = singletons::FighterManager();
    if mgr.is_null() {
        return false;
    }
    let fe = FighterManager::get_fighter_entry(mgr, FighterEntryID(entry as i32)) as *mut smash::app::FighterEntry;
    if fe.is_null() {
        return false;
    }
    let id = FighterEntry::current_fighter_id(fe) as u32;
    if sv_battle_object::kind(id) != *FIGHTER_KIND_PIKACHU {
        return false;
    }
    // THE ACCESSOR COMES FROM `victim::boma`, NOT FROM `sv_battle_object::module_accessor`, and
    // gate 4d is right to insist on it even though the id came from FighterManager a line ago. The
    // entry's `current_fighter_id` is as capable of being stale as any other engine-supplied id --
    // this hook runs on every weapon spawn in the fight, including ones during a respawn or a
    // transformation -- and the failure mode of getting it wrong here is the Instruction Abort at a
    // garbage address that `boma` exists to prevent. A costume test is not worth a crash.
    crate::victim::boma(id).map_or(false, |b| crate::charaslot::is_rat_costume(b))
}

/// The fighter whose `FighterParamAccessor2` entry a NEUTRALIZED row's class reads.
fn donor_kind(host: &str) -> Option<i32> {
    match host {
        "rope" => Some(*FIGHTER_KIND_GAOGAEN),
        "spikeball" => Some(*FIGHTER_KIND_PACKUN),
        _ => None,
    }
}

static mut FPA2_INSTANCE_PTR: usize = 0;

/// IS THE DONOR'S PARAM ENTRY ABSENT? That is the condition that makes these classes crash, so it
/// is the condition that decides who gets the private vtable.
///
/// The owner test alone NEVER MATCHED AT MATCH LOAD: two rope boots crashed with no NEUTRALISED
/// line in either log, although `diag` writes straight to the card. At fighter load the entry's
/// `current_fighter_id` and the costume test's battle-object lookup are not answerable yet. And
/// it was the wrong question anyway: clone_weapon gives EVERY Pikachu the article, vanilla
/// costumes included, and each of them crashes the same way when the donor is absent.
///
/// `FighterParamAccessor2` holds one 56-byte entry per fighter kind at `this + 0x70`, whose first
/// word is the fighter's param data -- proven from `hit_target_no_impl` (see
/// tools/check-article-donor-params.sh), and null exactly when that fighter is not loaded, which
/// is the `[null + 0x138]` the spikeball died on. When the donor IS in the match, his data is
/// there, the class runs as the game wrote it, and nobody's article is touched.
unsafe fn donor_absent(fighter_kind: i32) -> bool {
    if FPA2_INSTANCE_PTR == 0 {
        let mut out: usize = 0;
        let name = "_ZN3lib9SingletonIN3app21FighterParamAccessor2EE9instance_E\0";
        if skyline::nn::ro::LookupSymbol(&mut out as *mut usize, name.as_ptr()) != 0 || out == 0 {
            // Cannot tell: treat as absent, because absent is the case that crashes.
            return true;
        }
        FPA2_INSTANCE_PTR = out;
    }
    let inst = *(FPA2_INSTANCE_PTR as *const usize);
    if inst == 0 || fighter_kind < 0 {
        return true;
    }
    *((inst + 0x70 + fighter_kind as usize * 0x38) as *const usize) == 0
}

/// THE ROPE'S CAMERA RANGE, Incineroar's own `camera_range_rope` (fighter/gaogaen/param/vl.prc):
/// center_y 10, front 19, back -9, up 12, down -9, in the file's order. Four copies, because the
/// camera reset loops over a record count it reads from the module, and one is expected.
/// Mutable so it lands in writable memory: the camera reads it as param data, and nothing proves
/// it never writes back through the pointer.
static mut ROPE_CAMERA_RANGE: [[f32; 5]; 4] = [[10.0, 19.0, -9.0, 12.0, -9.0]; 4];
static mut ROPE_SLOT3_ORIGINAL: usize = 0;
static mut ROPE_GUARD_LOGGED: bool = false;

/// THE ROPE'S SLOT 3, GUARDED. Slot 3 is the rope's module start, and its first act is its
/// CameraModule's vfunc +0x50, which tail-calls +0x58 (13.0.5 0x3eaa80): reset each camera range
/// record from the pointer at camera+0x30. That pointer is Incineroar's compiled param lookup for
/// camera_range_rope, `data + 0x108` (main 0xac8de0) -- and with no Incineroar in the match the data
/// base is null, so it reads 0x108, and the reset faulted on it in both crash reports (01792305616,
/// 01792308093: LR main+0x3403a30, PC in that loop). So: a pointer below one page is not a pointer;
/// point it at his values, then run the real slot 3 with every other step untouched.
extern "C" fn rope_slot3_guard(this: *mut u8, object: *mut u8, a2: u64, a3: u64) {
    unsafe {
        let boma = *(object.add(0x20) as *const *mut u8);
        if !boma.is_null() {
            let cam = *(boma.add(0x60) as *const *mut u8);
            if !cam.is_null() {
                let at = cam.add(0x30) as *mut usize;
                let before = *at;
                if before < 0x1000 {
                    *at = core::ptr::addr_of_mut!(ROPE_CAMERA_RANGE) as usize;
                }
                if !ROPE_GUARD_LOGGED {
                    ROPE_GUARD_LOGGED = true;
                    let count = *(cam.add(0x20) as *const u32);
                    crate::telemetry::sink::diag(format!(
                        "[rat] articles: rope slot-3 guard: camera range pointer {before:#x}{} (record count {count})",
                        if before < 0x1000 { " -> Incineroar's camera_range_rope" } else { ", left alone" }
                    ));
                }
            }
        }
        let original: extern "C" fn(*mut u8, *mut u8, u64, u64) = core::mem::transmute(ROPE_SLOT3_ORIGINAL);
        original(this, object, a2, a3);
    }
}

// ---- THE SPIKEBALL'S STAND-IN FOR PIRANHA PLANT'S PARAM DATA (2026-10-18, for Ptooie) ----------
//
// Its slot 3 (main 0x3476e20) reads `[FPA2 + 0x1228]` -- Piranha Plant's entry -- and then
// `[that + 0x138]`, a std::vector of his compiled `hit_data_spikeball` records, before a camera
// reset like the rope's; its slot 55 (0x3476dd0) builds camera pointers `entry + 0x108` and
// `entry + 0x120`. His own param lookup puts exactly those fields there (main 0xe15240 hit_data
// +0x138, 0xe152b8 camera_range +0x108; the clip sphere is the 0x18 after it). With him absent the
// entry is null: slot 3 reads [null + 0x138] -- the September crash -- and slot 55's pointers are
// null-based.
//
// So both slots run against a STAND-IN: for the duration of the call his entry points at a static
// block holding his own values at his own offsets, then goes back to null. Everything else either
// slot does runs as the game wrote it.
//   +0x108  camera_range_spikeball   up 15, down 0, center_y 0, front 5, back -5 (his vl.prc), in
//                                    the order the rope's guard uses (center_y, front, back, up,
//                                    down) so the two stay consistent
//   +0x120  camera_clip_sphere_...   node `top`, offsets 0 0 0, r 40 (offsets at +8/+0xc/+0x10, as
//                                    the clip-sphere struct lookup returns them)
//   +0x138  hit_data_spikeball       EMPTY (begin == end): its 0x38-byte compiled layout is not yet
//                                    pinned down, and a wrong record would hand HitModule a garbage
//                                    joint. Cost: the ball has no hurtbox until it is.
// A pointer the slots left null-based (slot 55 may already have run, unpatched, inside the
// object's initialise) is rebased onto the block by slot 3's shim, as the rope's guard does.
const PACKUN_STANDIN_SIZE: usize = 0x200;
static mut PACKUN_STANDIN: [u64; PACKUN_STANDIN_SIZE / 8] = [0; PACKUN_STANDIN_SIZE / 8];
static mut PACKUN_STANDIN_READY: bool = false;
static mut SPIKE_SLOT3_ORIGINAL: usize = 0;
static mut SPIKE_SLOT55_ORIGINAL: usize = 0;
static mut SPIKE_LOGGED: bool = false;

unsafe fn packun_standin() -> usize {
    let base = core::ptr::addr_of_mut!(PACKUN_STANDIN) as *mut u8;
    if !PACKUN_STANDIN_READY {
        PACKUN_STANDIN_READY = true;
        let f = |off: usize, v: f32| *(base.add(off) as *mut f32) = v;
        // camera range
        for (i, v) in [0.0f32, 5.0, -5.0, 15.0, 0.0].iter().enumerate() {
            f(0x108 + 4 * i, *v);
        }
        // clip sphere
        *(base.add(0x120) as *mut u64) = smash::hash40("top");
        f(0x128, 0.0);
        f(0x12c, 0.0);
        f(0x130, 0.0);
        f(0x134, 40.0);
        // an empty vector: begin == end, both pointing inside the block
        let empty = base.add(0x1c0) as u64;
        *(base.add(0x138) as *mut u64) = empty;
        *(base.add(0x140) as *mut u64) = empty;
        *(base.add(0x148) as *mut u64) = empty;
    }
    base as usize
}

/// Run `f` with Piranha Plant's FPA2 entry pointing at the stand-in, if (and only if) it is null.
unsafe fn with_packun_standin(f: impl FnOnce()) {
    if FPA2_INSTANCE_PTR == 0 {
        let _ = donor_absent(*FIGHTER_KIND_PACKUN);
    }
    let inst = if FPA2_INSTANCE_PTR == 0 { 0 } else { *(FPA2_INSTANCE_PTR as *const usize) };
    if inst == 0 {
        f();
        return;
    }
    let at = (inst + 0x70 + *FIGHTER_KIND_PACKUN as usize * 0x38) as *mut usize;
    if *at != 0 {
        f();
        return;
    }
    *at = packun_standin();
    f();
    *at = 0;
}

extern "C" fn spikeball_slot3_shim(this: *mut u8, object: *mut u8, a2: u64, a3: u64) {
    unsafe {
        let standin = packun_standin();
        let boma = *(object.add(0x20) as *const *mut u8);
        if !boma.is_null() {
            let cam = *(boma.add(0x60) as *const *mut u8);
            if !cam.is_null() {
                let mut rebased = 0;
                for off in (0x20..0x80).step_by(8) {
                    let at = cam.add(off) as *mut usize;
                    let v = *at;
                    if (0x100..PACKUN_STANDIN_SIZE).contains(&v) {
                        *at = standin + v;
                        rebased += 1;
                    }
                }
                if !SPIKE_LOGGED {
                    SPIKE_LOGGED = true;
                    crate::telemetry::sink::diag(format!(
                        "[rat] articles: spikeball slot-3 shim: {rebased} null-based camera pointer(s) rebased onto the stand-in"
                    ));
                }
            }
        }
        let original: extern "C" fn(*mut u8, *mut u8, u64, u64) = core::mem::transmute(SPIKE_SLOT3_ORIGINAL);
        with_packun_standin(|| original(this, object, a2, a3));
    }
}

extern "C" fn spikeball_slot55_shim(this: *mut u8, a1: u64, a2: u64, a3: u64) {
    unsafe {
        let original: extern "C" fn(*mut u8, u64, u64, u64) = core::mem::transmute(SPIKE_SLOT55_ORIGINAL);
        with_packun_standin(|| original(this, a1, a2, a3));
    }
}

/// The private singleton for one row, built on first use from the object's OWN class: the
/// specializer the engine just stored at +0x3bb0 points at its vtable's address point, so the
/// group is copied from there and the listed slots repointed at `default_slot`.
unsafe fn private_singleton(row: usize, singleton: usize, slots: &[usize]) -> usize {
    if PRIVATE[row] != 0 {
        return PRIVATE[row];
    }
    let point = *(singleton as *const usize) as *const usize;
    let mut group: Vec<usize> = Vec::with_capacity(VTABLE_HEADER_WORDS + VTABLE_SLOTS);
    for i in (1..=VTABLE_HEADER_WORDS).rev() {
        group.push(*point.sub(i));
    }
    for s in 0..VTABLE_SLOTS {
        group.push(*point.add(s));
    }
    for &s in slots {
        group[VTABLE_HEADER_WORDS + s] = default_slot as usize;
    }
    if NEUTRALIZED[row].0 == "rope" {
        ROPE_SLOT3_ORIGINAL = group[VTABLE_HEADER_WORDS + 3];
        group[VTABLE_HEADER_WORDS + 3] = rope_slot3_guard as usize;
    }
    // The spikeball's two listed slots run against the stand-in instead of returning: the
    // ORIGINALS are read off the class's own vtable, since the loop above already overwrote them.
    if NEUTRALIZED[row].0 == "spikeball" {
        SPIKE_SLOT3_ORIGINAL = *point.add(3);
        SPIKE_SLOT55_ORIGINAL = *point.add(55);
        group[VTABLE_HEADER_WORDS + 3] = spikeball_slot3_shim as usize;
        group[VTABLE_HEADER_WORDS + 55] = spikeball_slot55_shim as usize;
    }
    let group: &'static mut [usize] = Box::leak(group.into_boxed_slice());
    let singleton: &'static mut usize = Box::leak(Box::new(group.as_ptr().add(VTABLE_HEADER_WORDS) as usize));
    PRIVATE[row] = singleton as *mut usize as usize;
    PRIVATE[row]
}

static mut SEEN: [u32; 2] = [0; 2];

unsafe fn neutralise(object: *mut u8, kind: u32, entry: u32) {
    for (row, row_kind, slots, host) in live_rows() {
        if kind as i32 != row_kind {
            continue;
        }
        let absent = donor_kind(host).map_or(true, |k| donor_absent(k));
        let ours = !absent && owner_is_pikachu(entry);
        if SEEN[row] < 4 {
            SEEN[row] += 1;
            crate::telemetry::sink::diag(format!(
                "[rat] articles: {host} object {object:p} initialised for entry {entry}: donor params {} -> {}",
                if absent { "ABSENT" } else { "present" },
                if absent || ours { "private vtable" } else { "left native (the donor's own article)" }
            ));
        }
        if !(absent || ours) {
            continue;
        }
        let at = object.add(SPECIALIZER_AT) as *mut usize;
        let before = *at;
        if before == 0 {
            continue;
        }
        let private = private_singleton(row, before, slots);
        *at = private;
        if !LOGGED[row] {
            LOGGED[row] = true;
            crate::telemetry::sink::diag(format!(
                "[rat] articles: NEUTRALISED {host} (kind {row_kind:#x}) for entry {entry}: slots {slots:?} -> \
                 ret; specializer {before:#x} -> private {private:#x}"
            ));
        }
    }
}

#[skyline::hook(offset = INIT_1304)]
unsafe fn weapon_initialize_1304(object: *mut u8, id: u32, kind: u32, entry: u32, unk: u64) {
    call_original!(object, id, kind, entry, unk);
    neutralise(object, kind, entry);
}

#[skyline::hook(offset = INIT_1305)]
unsafe fn weapon_initialize_1305(object: *mut u8, id: u32, kind: u32, entry: u32, unk: u64) {
    call_original!(object, id, kind, entry, unk);
    neutralise(object, kind, entry);
}

/// Does main+`at` hold the weapon initialize virtual? See `SIGNATURE`.
unsafe fn verified(at: usize) -> bool {
    let text = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize;
    let code = (text + at) as *const u32;
    for i in 0..SIGNATURE.len() {
        if *code.add(i) & SIGNATURE_MASK[i] != SIGNATURE[i] {
            return false;
        }
    }
    (0..0x1800 / 4).any(|i| *code.add(i) == STORE_SPECIALIZER)
}

/// Installs the hook only when a NEUTRALIZED row is live, so a build with both rows off is the
/// same binary behaviour as one without this module.
pub fn install() {
    let live: Vec<&str> = live_rows().map(|(_, _, _, h)| h).collect();
    if live.is_empty() {
        crate::telemetry::sink::diag("[rat] articles: no neutralised row is enabled; weapon hook not installed".into());
        return;
    }
    unsafe {
        if verified(INIT_1304) {
            skyline::install_hooks!(weapon_initialize_1304);
            ACTIVE = true;
            crate::telemetry::sink::diag(format!(
                "[rat] articles: weapon hook on main+{INIT_1304:#x} (13.0.4 signature verified) for {live:?}"));
        } else if verified(INIT_1305) {
            skyline::install_hooks!(weapon_initialize_1305);
            ACTIVE = true;
            crate::telemetry::sink::diag(format!(
                "[rat] articles: weapon hook on main+{INIT_1305:#x} (13.0.5 signature verified) for {live:?}"));
        } else {
            crate::telemetry::sink::diag(format!(
                "[rat] articles: NO weapon hook -- neither main+{INIT_1304:#x} nor main+{INIT_1305:#x} matches the \
                 initialize signature on this game version, so {live:?} will not be spawned"));
        }
    }
}
