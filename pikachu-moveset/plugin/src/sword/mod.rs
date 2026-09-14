// Every source fighter's weapon, merged into Pikachu's body model and shown for its own move.
//
// `tools/merge-weapons.sh` grafts each one as a named mesh object bound to a bone under Pikachu's
// `HaveR` -- technique from LilyLavender Modding Workshop episode 39, a distinct mesh-object name
// so `ModelModule::set_mesh_visibility` can address it. Hidden by default, shown only while the
// move that needs it is running, so Pikachu is not permanently armed.
//
// IT USED TO BE ONE SWORD FOR FOUR FIGHTERS. `SWORD_MOTIONS` listed seven motions and pointed all
// of them at `chrom_sword`, so Cloud's back-air swung Chrom's Falchion, Mythra's dash attack swung
// Chrom's Falchion, and so did Corrin's up-air. That was the honest thing to ship when one blade
// was all that had been merged; it is not the right weapon for any of them.
//
// WHAT IS RIGID AND WHAT IS NOT, because that is the whole of why this table has four rows and not
// seven. A graft carries ONE bone, which is exact for a weapon that is a stick: the bone is the
// handle, and Pikachu's own hand animation moves it correctly for free. An article whose skeleton
// ANIMATES cannot work that way --
//
//     gaogaen/rope        RopeC RopeL1..3 RopeR1..3     the cord bends, and it has to span to a
//                                                       victim who is moving
//     gamewatch/parachute Parachutea1 a2 b1 b2          the canopy opens and billows
//     kamui/dragonhand    Hand1..4 HandD HandU          the hand morphs into the dragon
//
// -- those shapes come from each article's own `model.nuanmb`, which nothing here plays. Grafting
// them would show rigid geometry in a pose, which for the rope in particular is worse than showing
// nothing: a straight bar between Pikachu and wherever the victim was. Corrin's up-air therefore
// keeps no weapon rather than the wrong one, and the rope and parachute are recorded in
// `docs/the-weapons.md` with what they would actually need.
use {
    smash::{
        lua2cpp::*,
        phx::*,
        app::{sv_animcmd::*, lua_bind::*, *},
        lib::{lua_const::*, L2CValue, L2CAgent},
        hash40
    },
    smash_script::*,
    smashline::{*, Priority::*}
};

/// mesh name, then the motions that show it.
///
/// The motion list per weapon is its SOURCE fighter's moves, from `tools/moves.tsv`. A motion in
/// two rows would show two weapons at once, which `tools/check-weapons.sh` refuses.
const WEAPONS: &[(&str, &[&str])] = &[
    // Chrom's Falchion -- his jab.
    ("chrom_sword", &["attack_11"]),
    // Cloud's Buster Sword -- his back-air.
    ("cloud_sword", &["attack_air_b"]),
    // Mythra's sword -- her dash attack and all three of her tilts.
    (
        "mythra_sword",
        &["attack_dash", "attack_s3_s", "attack_s3_hi", "attack_s3_lw"],
    ),
    // Palutena's staff -- her neutral-air and her grab. The grab motions are hers too: the three
    // catch boxes in `moves.tsv` are Palutena's, and a staff grab with no staff reads as a bug.
    (
        "palutena_staff",
        &["attack_air_n", "catch", "catch_dash", "catch_turn", "catch_wait", "catch_attack"],
    ),
];

/// SHOWN FOR THE WHOLE MOTION, not only the hitbox-active frames.
///
/// Recorded as a deviation when the Chrom sword shipped and it still holds: popping the mesh in and
/// out mid-swing reads worse than a continuous swing, and the hitboxes are disjoint either way.
unsafe extern "C" fn pikachu_weapon_frame(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let motion = MotionModule::motion_kind(boma);
    for (mesh, motions) in WEAPONS {
        let show = motions.iter().any(|m| motion == smash::hash40(m));
        ModelModule::set_mesh_visibility(boma, Hash40::new(mesh), show);
    }
}

pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_weapon_frame)
        .install();
}
