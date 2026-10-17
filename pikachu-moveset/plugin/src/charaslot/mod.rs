//! Rat of Doom as its own character slot: one CSS row, one costume, and the gate that keeps the
//! moveset off vanilla Pikachu's other seven.
//!
//! THE SLOT IS PIKACHU'S c00, AND THE MOD'S OWN FILE TREE IS WHAT DECIDED THAT. `mod/fighter/
//! pikachu/` ships `model/body/c00` and every grafted article -- `model/bow`, `model/cshot`,
//! `model/fusionsword`, `model/parachute` and their motions -- under c00 ALONE, because a costume's
//! body model carries its own textures and mirroring c00's over the others would turn every Pikachu
//! default yellow (`tools/mirror-costumes.sh` refuses to, and says so). c00 is therefore the only
//! slot that looks like Rat of Doom, and it no longer looks like Pikachu.
//!
//! WHY NOT AN ADDED SLOT AT c08. That was the first design, recorded here as "the assets must move
//! to c08-c15", and it is a bigger job than it looks: a ninth colour needs `color_num` raised on
//! PIKACHU'S OWN row, a `characall_label_c08`, an `nNN_index`, and CSS/stock art at c08 -- and the
//! mod has no UI-art pipeline at all (`mod/ui/` is one `msg_name.xmsbt`). The four added-slot mods
//! on this card (Bomb over Wario and friends) each ship that art by hand. Nothing about the moveset
//! needs a ninth colour; it needs to stop being on the other eight. So it takes one of the eight.
//!
//! WHAT THE CSS SHOWS, AND THE ONE WART. Rat of Doom gets its own `ui_chara_db` row redirecting to
//! Pikachu at colour 0, built on the API's own "redirected entry" example. Pikachu's row is LEFT
//! ALONE at eight colours, so his first portrait still reaches c00 and still plays Rat of Doom.
//! Shrinking him to colours 1..7 would fix that, and is not done, because `add_chara_db_entry_info`
//! is an ADD -- whether passing an existing `ui_chara_id` with no `clone_from` patches that row or
//! appends a duplicate Pikachu is not stated anywhere in the API and is not worth guessing at on a
//! roster. The gate below is what makes the slot real; the row is how it is presented.
//!
//! EVERY HASH BELOW WAS COMPUTED, AND THE FUNCTION THAT COMPUTED THEM WAS CHECKED against five
//! values the wiki states outright (`ui_chara_mario` 0xedaf3c863, `ui_chara_mario64` 0x10321edf2a,
//! `ui_chara_mario_00` 0x110c383d83, `color_start_index` 0x11895f00fc, `original_ui_chara_hash`
//! 0x160ab9eb98). All five matched, so the two new ones are trustworthy by the same arithmetic.
//!
//! THE ONE THING THE GATE CANNOT REACH is `fighter/common/param/fighter_param.prcxml`, where
//! `tools/gen-params.sh` patches PIKACHU'S ROW of the game's own table -- the jab combo's seven
//! params among them. That table is per-FIGHTER, not per-costume, and the reason those params live
//! there rather than in `src/params`' hook is that the engine's native ComboModule never calls the
//! hook (see the note there). So vanilla Pikachu on c01-c07 keeps Rat of Doom's jab PARAMS while
//! getting none of his scripts: a three-hit jab window on a two-hit jab. Known, bounded, and not
//! fixable from this side.
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

/// THE SLOT. Pikachu's colour 0 -- the one the grafted weapons ship for.
///
/// Changing this number is not enough on its own: `mod/fighter/pikachu/model/*` and the article
/// motions would have to move with it, and `tools/gen-config.sh` re-derive the `new-dir-files`
/// keys. `tools/check-slot.sh` asserts the two halves still agree.
pub const RAT_COSTUME: usize = 0;

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
