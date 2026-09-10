// Ported specials.
//
// speciallw is Incineroar's Revenge, reimplemented: SSBU counters are not in the ACMD dumps
// (the scripts only raise a status work flag; the detection is C++), and smashline has no
// cross-fighter status sharing. Its tuning comes from fighter/gaogaen/param/vl.prc.
//
// Game & Watch up-B (Fire + parachute), ported from the George mod's implementation
// with the agent renamed. The motion entries special_hi / special_air_hi /
// special_hi_open / special_hi_close do not exist on vanilla Pikachu; they are
// synthesized into motion_list.bin by tools/motion-patch/src/bin/synth.rs.
pub mod speciallw;
pub mod specials;
pub mod cargo;
pub mod specialhi;
pub mod specialhiclose;
pub mod specialhifall;
pub mod acmd;

pub fn install() {
    speciallw::install();
    specials::install();
    cargo::install();
    specialhi::install();
    specialhiclose::install();
    specialhifall::install();
    acmd::install();
}
