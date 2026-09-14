// The ACMD macros a ported script calls that `smash-script` does not export.
//
// WHY THIS FILE EXISTS, AND WHAT IT COST TO NOT HAVE IT. `tools/reach/src/bin/deforeign.rs`
// deletes any statement calling a `macros::` name that is not in smash-script's own list, because
// such a call fails the build outright rather than degrading. That was the right default and it
// was also silently dropping a real line:
//
//     gaogaen/SpecialSShoulder.txt:16   macros::CHECK_FINISH_CAMERA_IF_NOT_HP_MODE(agent, 0, 0);
//
// which is the Alolan Whip shoulder's request for the game's finish camera -- the zoomed dramatic
// shot on a finishing blow. The two lines AFTER it survived, because
// `FighterCutInManager::set_throw_finish_zoom_rate` and `set_throw_finish_offset` are plain
// lua_bind calls and not macros. So the port was setting that camera's zoom rate to 1.5 and its
// offset to the origin and then never asking for the camera. The lariat's own
// `CHECK_FINISH_CAMERA` is exported and came through, which is why one half of the move has the
// effect and the other half does not.
//
// The engine side is `app::sv_animcmd::CHECK_FINISH_CAMERA_IF_NOT_HP_MODE` at 0x22af920 in
// 13.0.5, and skyline-smash DOES bind it -- only smash-script's wrapper is missing. So this is a
// four-line thunk, copied in shape from smash-script's own `CHECK_FINISH_CAMERA` (macros.rs:1010)
// so the stack discipline matches exactly: clear, push both args, call, clear.
//
// A GENERATED FILE CALLS THESE, and the way it reaches them is worth stating. gen-moves.sh emits
// `use crate::acmd_ext as macros;` after the glob imports in every generated ACMD module. An
// explicit `use` shadows a glob-imported name, so `macros::` in a verbatim-copied script resolves
// here first and falls through to smash-script for the other 112. The generated scripts therefore
// stay byte-identical to the dump, which is the whole premise of `tools/reach/src/bin/acmdparity.rs`.
pub use smash_script::macros::*;

use smash::{app::sv_animcmd, lua2cpp::L2CAgentBase};
use smash_script::lua_args;

/// `CHECK_FINISH_CAMERA`, except the engine skips it in stamina mode.
///
/// Incineroar uses this one for the shoulder and the plain one for the lariat; both take the same
/// two arguments and he passes `0, 0` to all four call sites.
#[allow(non_snake_case)]
pub unsafe fn CHECK_FINISH_CAMERA_IF_NOT_HP_MODE<A: ToF32, B: ToF32>(
    agent: &mut L2CAgentBase,
    unk: A,
    unk2: B,
) {
    agent.clear_lua_stack();
    lua_args!(agent, unk.to_f32(), unk2.to_f32());
    sv_animcmd::CHECK_FINISH_CAMERA_IF_NOT_HP_MODE(agent.lua_state_agent);
    agent.clear_lua_stack();
}
