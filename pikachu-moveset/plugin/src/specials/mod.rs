// Ported specials.
//
// speciallw is Incineroar's Revenge, reimplemented: SSBU counters are not in the ACMD dumps
// (the scripts only raise a status work flag; the detection is C++), and smashline has no
// cross-fighter status sharing. Its tuning comes from fighter/gaogaen/param/vl.prc.
//
// Game & Watch up-B (Fire + parachute), ported from the George mod's implementation.
//
// THE UP-B TOOK TWO WRONG TURNS BEFORE IT WORKED, and both are worth keeping.
//
// FIRST, it kept George's FIGHTER_DIDDY_STATUS_KIND_SPECIAL_HI_CHARGE (0x1F0) and _CHARGE_DAMAGE
// (0x1F1). Fighter extra status kinds are PER-FIGHTER: those are Diddy's own statuses, and on
// Pikachu -- whose table ends at NUM = 0x1F0 -- they are one and two past the end. That looked
// like the bug, so v0.22 remapped them onto Pikachu's own up-B extras.
//
// SECOND, that remap was worse. SPECIAL_HI_WARP is Quick Attack's second leg, and the port only
// registers Pre/Main/End on it, so Pikachu's own Init and Exec still ran: they hide his model,
// apply the warp displacement and spawn the direction guide. Playtest got all three symptoms at
// once -- "up b appears to teleport modified pikachu extremely high and/or turn him invisible"
// and "spawns quick attack arrows around if I change the direction of the stick".
//
// The real answer is that Diddy's NUMBERS were fine and the reasoning about them was not:
// smashline extends the status table, so 0x1F0 and 0x1F1 are simply free ids on Pikachu. They are
// now declared for what they are in ./status.rs. tools/check-status.sh guards the range.
//
// The motion entries special_hi / special_air_hi / special_hi_open / special_hi_close do not
// exist on vanilla Pikachu; they are synthesized into motion_list.bin by
// tools/motion-patch/src/bin/synth.rs, and they ADOPT G&W's animations -- they used to keep
// Pikachu's Quick Attack ones, which is its own reason the move looked like a teleport.
pub mod status;
pub mod lockout;
pub mod speciallw;
pub mod acmd_speciallw;
pub mod specials;
pub mod cargo;
pub mod specialhi;
pub mod specialhiclose;
pub mod specialhifall;
pub mod acmd;

pub fn install() {
    lockout::install();
    speciallw::install();
    acmd_speciallw::install();
    specials::install();
    cargo::install();
    specialhi::install();
    specialhiclose::install();
    specialhifall::install();
    acmd::install();
}
