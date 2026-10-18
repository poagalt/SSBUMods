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

/// `SEARCH` in the shape the DUMP writes it: the second point as three plain numbers.
///
/// smash-script's takes `x2, y2, z2` as `Option<f32>` -- `None` pushes an empty value, which the
/// engine reads as "a sphere, not a capsule" -- and the dump always writes numbers there, so a
/// verbatim copy did not compile. Piranha Plant's Ptooie wait and shot are the first ported scripts
/// with a SEARCH (a box over his head, where the ball hangs, for the whole wait and shot). This
/// shadows smash-script's and forwards every value as `Some`, which is exactly what the dumped call
/// pushes; nothing ported so far passes the empty form.
#[allow(non_snake_case)]
pub unsafe fn SEARCH(
    agent: &mut L2CAgentBase,
    id: u64,
    part: u64,
    bone: smash::phx::Hash40,
    size: f32,
    x: f32,
    y: f32,
    z: f32,
    x2: f32,
    y2: f32,
    z2: f32,
    collision: i32,
    hit_status: i32,
    unk: u64,
    ground_air: i32,
    collision_category: i32,
    collision_parts: i32,
    unk2: bool,
) {
    smash_script::macros::SEARCH(
        agent, id, part, bone, size, x, y, z, Some(x2), Some(y2), Some(z2), collision, hit_status,
        unk, ground_air, collision_category, collision_parts, unk2,
    );
}

/// `EFFECT` plus a sub-attribute: the engine binds it (`app::sv_animcmd::EFFECT_ATTR`) and
/// smash-script has no wrapper, so deforeign was deleting the call -- and with it Ptooie's
/// `packun_spikeball_shoot` and `packun_spikeball_end`, his only EFFECT_ATTR uses, both with
/// `EFFECT_SUB_ATTRIBUTE_NO_JOINT_SCALE`. Same stack discipline as smash-script's `EFFECT`, with
/// the attribute pushed last, the position it has in the dumped call.
#[allow(non_snake_case)]
pub unsafe fn EFFECT_ATTR<
    A: ToF32,
    B: ToF32,
    C: ToF32,
    D: ToF32,
    E: ToF32,
    F: ToF32,
    G: ToF32,
    H: ToF32,
    I: ToF32,
    J: ToF32,
    K: ToF32,
    L: ToF32,
    M: ToF32,
>(
    agent: &mut L2CAgentBase,
    effect: smash::phx::Hash40,
    bone: smash::phx::Hash40,
    x_pos: A,
    y_pos: B,
    z_pos: C,
    x_rot: D,
    y_rot: E,
    z_rot: F,
    size: G,
    unk8: H,
    unk9: I,
    unk10: J,
    unk11: K,
    unk12: L,
    unk13: M,
    unk14: bool,
    attr: i32,
) {
    agent.clear_lua_stack();
    lua_args!(agent, effect, bone, x_pos.to_f32(), y_pos.to_f32(), z_pos.to_f32(), x_rot.to_f32(),
              y_rot.to_f32(), z_rot.to_f32(), size.to_f32(), unk8.to_f32(), unk9.to_f32(),
              unk10.to_f32(), unk11.to_f32(), unk12.to_f32(), unk13.to_f32(), unk14, attr);
    sv_animcmd::EFFECT_ATTR(agent.lua_state_agent);
    agent.clear_lua_stack();
}
