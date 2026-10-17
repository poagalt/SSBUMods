//! Rat of Doom as its own character slot: an ADDED Pikachu colour, its own CSS row, and the gate
//! that keeps the moveset off every colour Pikachu actually has.
//!
//! THE DESIGN CHANGED ON 2026-10-17 AFTER READING TWO MODS THAT ALREADY DO THIS. It first took
//! Pikachu's c00, on the reasoning that the grafted weapons ship for c00 alone and so exactly one
//! of his eight could look like Rat of Doom. That worked, and it carried a wart that the first
//! hardware test found immediately: "both pikachu slots appear to load rat of doom". They did, and
//! by design -- c00 is reachable from Pikachu's own row as well as from this one, and the only way
//! to close that was to shrink his row, which depended on two things the CSK API does not write
//! down.
//!
//! `Documents/switch/Sans Moveset` (over Palutena) and `Gogeta Moveset` (over Min Min) do not have
//! the problem, because they do not take one of the host's colours. Sans takes palutena c120-c127,
//! Gogeta takes minmin c103-c110, and the README states the registration outright:
//!
//!     Duplicate Palutena / Character ID: ui_chara_sans / Name ID: sans
//!     Series ID: undertale / Amount of Colors: 8 / Color Start Index: 120
//!
//! An ADDED colour is outside the host's range, so his eight costumes are untouched BY
//! CONSTRUCTION. There is no overlap to gate, no row to shrink and no wart. Everything the c00
//! design needed a gate for, this design does not have.
//!
//! THE GATE STAYS ANYWAY, and it is not redundant. Assets are per-slot by path, so moving them to
//! c120 is enough for THEM -- but this plugin's `Agent` registrations are per-FIGHTER, and an
//! ungated one would install Little Mac's jab on every Pikachu on the card whatever colour he is.
//! `agent()` below is the only constructor that sets a costume list, and gate 4p fails the build
//! on a raw `Agent::new`. The same is true of the two `skyline::hook` sites, which smashline's
//! costume filter cannot reach and which call `is_rat_costume` themselves.
//!
//! WHAT AN ADDED SLOT ALSO BUYS, beyond the wart:
//!
//!   * ITS OWN SOUND BANK. `sound/bank/fighter/se_pikachu_c120.nus3audio` -- Pikachu has the full
//!     per-slot set in vanilla (`se_pikachu_c00` … `_c07`), and Sans ships exactly this for
//!     Palutena. So the ported moves can carry their source's sounds without a single byte of
//!     vanilla Pikachu's audio changing. See handbook/open-questions.md.
//!   * ITS OWN EFFECT ARCHIVE, `effect/fighter/pikachu/ef_pikachu_c120.eff`, which is what One Slot
//!     Effects is for and what Sans ships for all eight of its slots.
//!
//! WHAT AN ADDED SLOT DOES NOT FIX, AND I BRIEFLY WROTE HERE THAT IT DID. `fighter_param.prcxml`
//! patches Pikachu's row of `fighter_param_table`, which is keyed by FIGHTER KIND. A c120 Pikachu
//! is still `FIGHTER_KIND_PIKACHU` and reads that same row, so the jab-combo params -- the seven
//! that have to live there because native ComboModule never calls the `get_param_int` hook -- still
//! reach vanilla Pikachu on all eight of his colours. Moving the slot changes nothing about that.
//! `lib_paramconfig` (CSharpM7), which BOTH reference mods list as a dependency, is the mechanism
//! for a genuinely per-slot param, and is the route if this ever needs closing.
//!
//! ONE COLOUR, NOT EIGHT, AND THAT IS THE ONE PLACE WE DIFFER. Both reference mods register eight
//! because they ship eight recolours -- Sans's c121 has 544 files of its own. Rat of Doom has one
//! look, so it registers one colour and `RAT_COSTUMES` is a slice for the day that changes.
//!
//! EVERY HASH BELOW WAS COMPUTED, AND THE FUNCTION THAT COMPUTED THEM WAS CHECKED against five
//! values the wiki states outright (`ui_chara_mario` 0xedaf3c863, `ui_chara_mario64` 0x10321edf2a,
//! `ui_chara_mario_00` 0x110c383d83, `color_start_index` 0x11895f00fc, `original_ui_chara_hash`
//! 0x160ab9eb98). All five matched, so the two new ones are trustworthy by the same arithmetic.
//!
//! THE CSS ART HAS TO BE SHIPPED AND IS NOT YET. The portraits are files named after the row --
//! `ui/replace/chara/chara_N/chara_N_ratofdoom_00.bntx` -- and `original_ui_chara_hash` redirects
//! the fighter a row resolves to, not a texture lookup; `CharacterDatabaseEntry` has no art field
//! among its 38. That is why the first hardware test reported "the second one has no character
//! model on the character select screen". Sans ships the same eight files under its own name.
//! `tools/arc-pull/paths-ui.txt` pulls Pikachu's c00 set to copy from, and the pull needs the game
//! running.
use std::collections::HashMap;

use smashline::Agent;
use the_csk_collection_api as csk;

/// `ui_chara_ratofdoom`.
const UI_CHARA_RATOFDOOM: u64 = 0x1223b5768f;
/// `ui_chara_ratofdoom_00`.
const UI_LAYOUT_RATOFDOOM_00: u64 = 0x154939732d;
/// `ui_chara_pikachu` -- the row this one is cloned from and redirects to.
const UI_CHARA_PIKACHU: u64 = 0x105cf985ed;
/// `ui_chara_pikachu_00`.
const UI_LAYOUT_PIKACHU_00: u64 = 0x134291d3d9;
/// `color_start_index`, stated by the wiki.
const COLOR_START_INDEX: u64 = 0x11895f00fc;
/// `original_ui_chara_hash`, stated by the wiki.
const ORIGINAL_UI_CHARA_HASH: u64 = 0x160ab9eb98;

/// THE SLOT. An ADDED colour, outside Pikachu's own eight.
///
/// 120 IS THE CONVENTION, READ OFF TWO WORKING MODS rather than picked. `Documents/switch/Sans
/// Moveset` takes palutena c120-c127 and `Gogeta Moveset` takes minmin c103-c110; the base is
/// chosen per mod so that two movesets on one host fighter do not collide, and being outside 0-7
/// is the point -- the host's own costumes cannot be reached by this row and this row cannot be
/// reached from his. Sans's README states it in the CSS Manager's own field names:
///
///     Duplicate Palutena / Character ID: ui_chara_sans / Name ID: sans
///     Series ID: undertale / Amount of Colors: 8 / Color Start Index: 120
///
/// Changing this number is one edit: `tools/relocate-slot.sh` moves the built tree onto it,
/// `tools/gen-config.sh` derives every `config.json` key from it, and `tools/check-slot.sh` fails
/// if the two disagree. What it is NOT free to change is the base once the mod has shipped --
/// a save file remembers costumes by index.
pub const RAT_COSTUME: usize = 120;

/// The costume list every `Agent` in this plugin is installed under.
///
/// A one-element list today. It is a slice rather than a bare constant so that adding a second
/// Rat of Doom colour later is an edit here and nowhere else.
pub const RAT_COSTUMES: &[usize] = &[RAT_COSTUME];

/// Set false to leave the roster untouched without unpicking the module.
pub const ENABLED: bool = true;


/// A Pikachu `Agent` already gated to Rat of Doom's costume.
///
/// EVERY REGISTRATION IN THIS PLUGIN GOES THROUGH HERE, including the eight generated modules --
/// `tools/gen-moves.sh` emits this call, not `Agent::new`. `Agent::install` routes acmd, status,
/// line and state installs through smashline's `*_costume` variants with whatever `set_costume`
/// was given, so one call site gates all four kinds at once.
///
/// Agents for OTHER fighters are not built here and must not be: `telemetry::SOURCE_AGENTS`
/// installs a line callback on Little Mac and Incineroar to record what the real move does, and
/// gating those to colour 0 would silently trace only their default costumes.
pub fn agent(name: &str) -> Agent {
    let mut agent = Agent::new(name);
    agent.set_costume(RAT_COSTUMES.to_vec());
    agent
}

/// Is this object a Rat of Doom -- a Pikachu on the gated costume?
///
/// For the hooks smashline's costume filter cannot reach: `src/params`' four `WorkModule`
/// replacements and `src/articles/neutralize.rs`' `WEAPON_INITIALIZE`, all of which are installed
/// globally and see every object in the fight.
pub unsafe fn is_rat_costume(boma: *mut smash::app::BattleObjectModuleAccessor) -> bool {
    use smash::{app::lua_bind::WorkModule, lib::lua_const::*};
    if boma.is_null() {
        return false;
    }
    let color = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR) as usize;
    RAT_COSTUMES.contains(&color)
}

pub fn install() {
    if !ENABLED {
        return;
    }
    // NO VERSION CHECK, AND THE WIKI IS WRONG THAT ONE IS AVAILABLE. Its first example calls
    // `csk_collection_version()`, but at rev e144a25 that name exists only inside the crate's
    // `extern "C"` block -- there is no safe wrapper, so the call does not compile. The API's
    // notice that a missing libthe_csk_collection.nro crashes still stands, which is why this runs
    // LAST in lib.rs: if the plugin is absent, the moveset has already installed.
    crate::telemetry::sink::diag(format!(
        "[rat] CHARASLOT registering ui_chara_ratofdoom ({UI_CHARA_RATOFDOOM:#x}) redirecting to \
         ui_chara_pikachu ({UI_CHARA_PIKACHU:#x}) at colour {RAT_COSTUME}; the moveset is installed \
         on costume(s) {RAT_COSTUMES:?} and vanilla Pikachu holds the rest"
    ));
    csk::add_chara_db_entry_info(csk::CharacterDatabaseEntry {
        ui_chara_id: UI_CHARA_RATOFDOOM,
        clone_from_ui_chara_id: Some(UI_CHARA_PIKACHU),
        // `StringType` wraps `CStrCSK`, not `String` -- the wiki example does not compile here.
        name_id: csk::StringType::Overwrite(csk::CStrCSK::new("ratofdoom")),
        color_num: csk::UnsignedByteType::Overwrite(RAT_COSTUMES.len() as u8),
        extra_index_maps: csk::UnsignedByteMap::Overwrite(HashMap::from([(
            COLOR_START_INDEX,
            csk::UnsignedByteType::Overwrite(RAT_COSTUME as u8),
        )])),
        extra_hash_maps: csk::Hash40Map::Overwrite(HashMap::from([(
            ORIGINAL_UI_CHARA_HASH,
            csk::Hash40Type::Overwrite(UI_CHARA_PIKACHU),
        )])),
        ..Default::default()
    });

    // THE LAYOUT ROW IS NOT OPTIONAL. `ui_chara_db` says the slot exists; `ui_layout_db` says what
    // the CSS draws for it. Cloning Pikachu's `_00` keeps his portrait geometry, which is correct
    // here because the model is his.
    csk::add_chara_layout_db_entry_info(csk::CharacterLayoutDatabaseEntry {
        ui_layout_id: UI_LAYOUT_RATOFDOOM_00,
        clone_from_ui_layout_id: Some(UI_LAYOUT_PIKACHU_00),
        ui_chara_id: csk::Hash40Type::Overwrite(UI_CHARA_RATOFDOOM),
        ..Default::default()
    });
}
