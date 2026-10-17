#![feature(proc_macro_hygiene)]
#![allow(
    unused_imports,
    unused_macros,
    unused_variables,
    unused_assignments,
    unused_unsafe,
    non_upper_case_globals,
    non_snake_case,
    clippy::borrow_interior_mutable_const
)]

// The ACMD macros smash-script does not export, re-exporting the ones it does. Every generated
// module shadows `macros` with this, so a verbatim-copied script can call all of them.
mod acmd_ext;
mod normals;
mod smashes;
mod aerials;
mod throws;
mod defensive;
mod landings;
mod dodges;
// Generated ACMDs for ported specials -- currently Incineroar Revenge. Separate from the
// hand-written status machines in src/specials/.
mod specials_acmd;
mod telemetry;
mod specials;
mod sword;
mod cues;
mod articles;
mod victim;
// Answers the engine when it asks our Pikachu for one of Incineroar's params -- see params/mod.rs.
mod params;

#[skyline::main(name = "rat_of_doom")]
pub fn main() {
    // ---- BISECT, 2026-10-17 -------------------------------------------------------------------
    //
    // Moving `librat_of_doom.nro` out of `plugins/` stopped the air-damage crash dead, with every
    // other thing still in place -- our 866 mod files, both prcxml patches, George's plugin.nro,
    // smashline v1.6.6, ARCropolis, the skin mods. So the fault is in THIS plugin's code, and the
    // job now is which install.
    //
    // GROUPED, NOT ONE FLAG PER MODULE, because the modules are not independent: `specials_acmd`
    // installs scripts for statuses `specials` registers, and `sword` reads article ids that
    // `articles` creates. Splitting those pairs would break a boot for a reason unrelated to the
    // bug and spend a playtest learning nothing.
    //
    // Flip ONE group, rebuild, deploy, one boot. Everything eliminated so far is in
    // `telemetry::damageprobe`'s module doc; do not re-run those.
    const MOVES: bool = true; // normals, smashes, aerials, throws, defensive, landings, dodges
    const SPECIALS: bool = true; // CLEARED by bisect step 1: crashed with this off: 28 new status kinds, the most invasive
    const WEAPONS: bool = true; // sword + articles
    const PARAMS: bool = true; // CLEARED by bisect step 3: no crash with this on, observers off
    // OBSERVERS split three ways after step 3 localised the fault here. `damageprobe` is NOT in
    // the list: the crash predates it, and three probes from it produced nothing.
    const CUES: bool = true; // CLEARED by bisect step 4: no crash with this on, telemetry off
    const TELEMETRY_FRAME: bool = true; // the per-frame pikachu_telemetry callback
    const MEMSERVE: bool = true; // spawns a thread doing a blocking Accept

    if MOVES {
        normals::install();
        smashes::install();
        aerials::install();
        throws::install();
        defensive::install();
        landings::install();
        dodges::install();
    }
    if SPECIALS {
        specials::install();
        specials_acmd::install();
    }
    if WEAPONS {
        sword::install();
        articles::install();
    }
    if PARAMS {
        params::install();
    }
    if CUES {
        cues::install();
    }
    if TELEMETRY_FRAME || MEMSERVE {
        telemetry::install_selective(TELEMETRY_FRAME, MEMSERVE);
    }
    crate::telemetry::sink::diag(format!(
        "[rat] BISECT moves={MOVES} specials={SPECIALS} weapons={WEAPONS} params={PARAMS}          cues={CUES} telemetry_frame={TELEMETRY_FRAME} memserve={MEMSERVE}"
    ));
}
