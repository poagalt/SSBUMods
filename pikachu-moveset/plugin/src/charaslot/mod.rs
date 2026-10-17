//! Rat of Doom as its own character-select slot, through the CSK Collection API.
//!
//! WHAT THIS DOES AND WHAT IT DELIBERATELY DOES NOT. This registers the two database rows a extra
//! CSS slot needs -- a `ui_chara_db` entry and a `ui_layout_db` entry -- so the moveset appears
//! beside Pikachu rather than replacing him on the roster. It does NOT by itself make the moveset
//! costume-gated; see THE TWO THINGS STILL OWED, below, which is the larger half of the job.
//!
//! THE SHAPE IS THE API'S OWN "redirected entry" EXAMPLE, not an invention. Coolsonickirby's wiki
//! shows `ui_chara_mario64` cloned from `ui_chara_mario` with two extra maps:
//!
//!   `color_start_index`      where this slot's colours begin in the FIGHTER's colour space
//!   `original_ui_chara_hash` the real fighter this row redirects to
//!
//! That is how an added slot works: there is one Pikachu fighter with more colours than vanilla,
//! and the second CSS portrait is a row pointing into the upper half of them.
//!
//! EVERY HASH BELOW WAS COMPUTED, AND THE FUNCTION THAT COMPUTED THEM WAS CHECKED against five
//! values the wiki states outright (`ui_chara_mario` 0xedaf3c863, `ui_chara_mario64` 0x10321edf2a,
//! `ui_chara_mario_00` 0x110c383d83, `color_start_index` 0x11895f00fc, `original_ui_chara_hash`
//! 0x160ab9eb98). All five matched, so the two new ones are trustworthy by the same arithmetic.
//!
//! THE TWO THINGS STILL OWED before this is a real slot rather than a roster entry:
//!
//!   1. THE ASSETS MUST MOVE TO c08-c15. `COLOR_START` is 8, so this row shows Pikachu's colours
//!      8 through 15. The mod currently ships c00-c07, which is vanilla Pikachu's own range, so
//!      as it stands this portrait would show HIS costumes. `tools/mirror-costumes.sh` is the
//!      pass that would have to emit the upper eight.
//!   2. THE MOVESET MUST BE GATED ON COLOUR. Every `Agent::new("pikachu")` registration in this
//!      plugin applies to EVERY Pikachu, so without a gate the vanilla slots get the moveset too
//!      and the second portrait means nothing. smashline's `*_costume` installers -- the five this
//!      plugin already imports -- take the costume list that does exactly this, and
//!      `src/sword`'s WEAPONS poll would need the same test.
//!
//! Neither is written yet, and the registration is harmless without them: an extra CSS row that
//! plays the same character. It is committed separately so the database half can be verified on
//! hardware on its own, which is the only way to tell a bad hash from a bad gate.
use std::collections::HashMap;

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

/// Where this slot's colours begin in Pikachu's colour space. See owed item 1.
const COLOR_START: u8 = 8;
/// How many colours the slot offers.
const COLOR_NUM: u8 = 8;

/// Set false to leave the roster untouched without unpicking the module.
pub const ENABLED: bool = true;

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
         ui_chara_pikachu ({UI_CHARA_PIKACHU:#x}) with colours {COLOR_START}..{}",
        COLOR_START + COLOR_NUM - 1
    ));
    csk::add_chara_db_entry_info(csk::CharacterDatabaseEntry {
        ui_chara_id: UI_CHARA_RATOFDOOM,
        clone_from_ui_chara_id: Some(UI_CHARA_PIKACHU),
        // `StringType` wraps `CStrCSK`, not `String` -- the wiki example does not compile here.
        name_id: csk::StringType::Overwrite(csk::CStrCSK::new("ratofdoom")),
        color_num: csk::UnsignedByteType::Overwrite(COLOR_NUM),
        extra_index_maps: csk::UnsignedByteMap::Overwrite(HashMap::from([(
            COLOR_START_INDEX,
            csk::UnsignedByteType::Overwrite(COLOR_START),
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
