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
//! WHO GETS IT. Only a weapon whose kind is a NEUTRALIZED row's kind AND whose owner entry is a
//! Pikachu -- every Pikachu runs this moveset, and asking the engine for the entry's current fighter
//! means no state of ours has to be up to date when this runs (it may run at fighter load).
//!
//! NOTHING HAPPENS UNLESS A NEUTRALIZED ROW IS ENABLED IN CANDIDATES: `install` does not even
//! install the hook otherwise. Both rows ship `enabled: false`. Arming one is a watched-boot,
//! single-variable experiment like any new article; the first boot's log says, once per kind,
//! which entry and which slots were neutralised, so a crash can be attributed.

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

/// main+0x339edc0: the weapon initialize-with-kind virtual (see the module doc).
const WEAPON_INITIALIZE: usize = 0x339edc0;
/// main+0x529d400 (.data+0x386400): 615 pointers, one class singleton per WEAPON_KIND_*.
const WEAPON_CLASS_TABLE: usize = 0x529d400;
/// Where a weapon object keeps its class singleton.
const SPECIALIZER_AT: usize = 0x3bb0;
const VTABLE_SLOTS: usize = 114;
/// offset-to-top and RTTI, before the address point.
const VTABLE_HEADER_WORDS: usize = 2;

/// The defaults come from a weapon that uses them in all three slots, read at runtime so the
/// private vtable holds the running game's own addresses. The parachute is clean and takes the
/// majority default in slots 3, 40 and 55 -- the gate checks that too.
unsafe fn default_donor_kind() -> i32 {
    *WEAPON_KIND_GAMEWATCH_PARACHUTE
}

static mut PRIVATE: [usize; 2] = [0; 2];
static mut LOGGED: [bool; 2] = [false; 2];

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
    sv_battle_object::kind(id) == *FIGHTER_KIND_PIKACHU
}

/// The private singleton for one row, built on first use from the running game's own memory.
unsafe fn private_singleton(row: usize, kind: i32, slots: &[usize]) -> usize {
    if PRIVATE[row] != 0 {
        return PRIVATE[row];
    }
    let text = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize;
    let table = (text + WEAPON_CLASS_TABLE) as *const usize;
    let point = *(*table.add(kind as usize) as *const usize) as *const usize;
    let clean = *(*table.add(default_donor_kind() as usize) as *const usize) as *const usize;
    let mut group: Vec<usize> = Vec::with_capacity(VTABLE_HEADER_WORDS + VTABLE_SLOTS);
    for i in (1..=VTABLE_HEADER_WORDS).rev() {
        group.push(*point.sub(i));
    }
    for s in 0..VTABLE_SLOTS {
        group.push(*point.add(s));
    }
    for &s in slots {
        group[VTABLE_HEADER_WORDS + s] = *clean.add(s);
    }
    let group: &'static mut [usize] = Box::leak(group.into_boxed_slice());
    let singleton: &'static mut usize = Box::leak(Box::new(group.as_ptr().add(VTABLE_HEADER_WORDS) as usize));
    PRIVATE[row] = singleton as *mut usize as usize;
    PRIVATE[row]
}

#[skyline::hook(offset = WEAPON_INITIALIZE)]
unsafe fn weapon_initialize(object: *mut u8, id: u32, kind: u32, entry: u32, unk: u64) {
    call_original!(object, id, kind, entry, unk);
    for (row, row_kind, slots, host) in live_rows() {
        if kind as i32 != row_kind || !owner_is_pikachu(entry) {
            continue;
        }
        let at = object.add(SPECIALIZER_AT) as *mut usize;
        let before = *at;
        let private = private_singleton(row, row_kind, slots);
        *at = private;
        if !LOGGED[row] {
            LOGGED[row] = true;
            crate::telemetry::sink::diag(format!(
                "[rat] articles: NEUTRALISED {host} (kind {row_kind:#x}) for entry {entry}: slots {slots:?} -> \
                 defaults; specializer {before:#x} -> private {private:#x}"
            ));
        }
    }
}

/// Installs the hook only when a NEUTRALIZED row is live, so a build with both rows off is the
/// same binary behaviour as one without this module.
pub fn install() {
    let live: Vec<&str> = live_rows().map(|(_, _, _, h)| h).collect();
    if live.is_empty() {
        crate::telemetry::sink::diag("[rat] articles: no neutralised row is enabled; weapon hook not installed".into());
        return;
    }
    crate::telemetry::sink::diag(format!("[rat] articles: installing the weapon hook for {live:?}"));
    skyline::install_hooks!(weapon_initialize);
}
