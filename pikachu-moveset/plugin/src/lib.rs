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
mod articles;
mod victim;

#[skyline::main(name = "rat_of_doom")]
pub fn main() {
    normals::install();
    smashes::install();
    aerials::install();
    throws::install();
    defensive::install();
    landings::install();
    dodges::install();
    telemetry::install();
    specials::install();
    specials_acmd::install();
    sword::install();
    articles::install();
}
