// Ported specials.
//
// speciallw is Incineroar's Revenge, reimplemented: SSBU counters are not in the ACMD dumps
// (the scripts only raise a status work flag; the detection is C++), and smashline has no
// cross-fighter status sharing. Its tuning comes from fighter/gaogaen/param/vl.prc.
//
// Game & Watch up-B (Fire + parachute), ported from the George mod's implementation.
//
// THE PORT WAS BROKEN UNTIL v0.22 and the cause is worth keeping. It kept George's
// FIGHTER_DIDDY_STATUS_KIND_SPECIAL_HI_CHARGE (0x1F0) and _CHARGE_DAMAGE (0x1F1). Fighter extra
// status kinds are PER-FIGHTER: Pikachu's run 0x1E1 to 0x1EF with NUM = 0x1F0, so Diddy's charge
// status is exactly one past the end of Pikachu's table and the damage one is two past. Pikachu
// was being sent to statuses that do not exist, which is why he floated in free fall instead of
// rising with the parachute. They now map onto his own up-B extras, SPECIAL_HI_WARP and
// SPECIAL_HI_END, which this moveset replaces anyway. Same collision class as the article and
// work-flag ids deforeign strips from GENERATED code -- but this was hand-written, so nothing
// was checking it. tools/check-status.sh now does.
// with the agent renamed. The motion entries special_hi / special_air_hi /
// special_hi_open / special_hi_close do not exist on vanilla Pikachu; they are
// synthesized into motion_list.bin by tools/motion-patch/src/bin/synth.rs.
pub mod speciallw;
pub mod acmd_speciallw;
pub mod specials;
pub mod cargo;
pub mod specialhi;
pub mod specialhiclose;
pub mod specialhifall;
pub mod acmd;

pub fn install() {
    speciallw::install();
    acmd_speciallw::install();
    specials::install();
    cargo::install();
    specialhi::install();
    specialhiclose::install();
    specialhifall::install();
    acmd::install();
}
