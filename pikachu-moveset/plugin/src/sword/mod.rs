// Chrom's Falchion, merged into Pikachu's body model by tools/merge-sword.sh as a mesh
// object named `chrom_sword` bound to a `Sword` bone under HaveR.
//
// Technique from LilyLavender Modding Workshop episode 39: a distinct mesh-object name so
// set_mesh_visibility can target it. The blade is hidden by default and shown only while a
// sword move is running, so Pikachu is not permanently armed.
//
// Deviation from the brief, recorded in BACKLOG.md: the blade is shown for the whole motion
// rather than only the hitbox-active frames. Popping the mesh in and out mid-swing reads worse
// than a continuous swing, and the hitboxes are disjoint either way.
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

const SWORD_MOTIONS: &[&str] = &[
    "attack_11",       // Chrom jab
    "attack_dash",     // Mythra dash attack
    "attack_s3_s",     // Mythra f-tilt
    "attack_s3_hi",
    "attack_s3_lw",
    "attack_air_b",    // Cloud b-air
    "attack_air_hi",   // Corrin u-air
];

unsafe extern "C" fn pikachu_sword_frame(fighter: &mut L2CFighterCommon) {
    let motion = MotionModule::motion_kind(fighter.module_accessor);
    let show = SWORD_MOTIONS.iter().any(|m| motion == smash::hash40(m));
    ModelModule::set_mesh_visibility(fighter.module_accessor, Hash40::new("chrom_sword"), show);
}

pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_sword_frame)
        .install();
}
