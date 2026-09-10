// Neutral B — Piranha Plant's Ptooie (the `spikeball` article).
//
// DISABLED. Enabling the clone below crashed the game on fighter load, every time,
// with a null dereference inside smashline:
//
//   ReturnAddress[06]: (smashline + 0x2a68c) (fighter_initialize_object_data + 0x2c)
//   ReturnAddress[00]: (smashline + 0x1769c) (load_fighter_effects + 0x48)
//
// Cause: episode46's guide warns that cloning an article which references fighter-specific
// effects requires copying those effects to the receiving agent. Ptooie's spikeball uses
// packun effects, and nothing under effect/fighter/pikachu/ provides them.
//
// Note the tension with ADR 0002: supplying them means shipping a fighter-prefixed effect
// archive, which is the CSK dependency the brief forbids. Resolve that before re-enabling.
//
// Assets are extracted and staged; see BACKLOG.md.
//
// pub const FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL: i32 = 0x7;

pub fn install() {
    // Intentionally empty. See above before re-enabling.
    //
    // smashline::clone_weapon(
    //     "packun",
    //     *smash::lib::lua_const::WEAPON_KIND_PACKUN_SPIKEBALL,
    //     "pikachu",
    //     "spikeball",
    //     true,
    // );
}
