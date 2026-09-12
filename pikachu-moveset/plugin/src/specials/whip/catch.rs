// The one part of the whip that is NOT a transcription, and the reason why.
//
// ADR 0006 says the whip's statuses are transcribed from Incineroar's, and `mod.rs` does that for
// all six. This file is the exception, kept in its own file so the exception is visible rather
// than buried in a status function.
//
// INCINEROAR'S STATUS SCRIPTS DO NOT DETECT THE CATCH AT ALL. That is not a gap in the dump; it is
// a property of the code. `disasm --sym catch` over the whole gaogaen NRO finds five symbols and
// they are `sv_animcmd::CATCH`, `FT_CATCH_STOP`, `AttackModule__set_catch_only_all_impl` and two
// expression rumbles -- there is no `CatchModule__is_catch_impl` import anywhere in the module.
// `SpecialS` main clears `..._INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER` on entry and five later
// statuses read it back, and NOTHING IN THE NRO EVER WRITES IT.
//
// Reading the six `main_loop`s settled it from the other direction: not one of them looks for a
// victim or changes status on a catch. `0x71000239d0` (SPECIAL_S's loop) is the same generic
// skeleton as the follow-ups. So the transition from SPECIAL_S into SPECIAL_S_THROW is done by
// compiled C++ in `main`, keyed on the fighter kind, and it does not run for a Pikachu.
//
// `docs/the-recording.md` reached the same conclusion empirically: "Neither Donkey Kong nor
// Incineroar ever calls `CaptureModule::capture`." For them the engine establishes the capture from
// the catch collision and none of it passes through a hookable module function. There is therefore
// nothing to transcribe and nothing to route through -- `capture` is the only way in from a plugin,
// and it demonstrably works.
//
// So this file substitutes for exactly one thing: noticing the catch and starting the throw. Every
// consequence of the catch is still the engine's -- the victim's own status reads our
// `param_special_s`, starts the clatter, plays `capture_pulled` and then `capture_wait` every
// frame, and unlinks itself when told. None of that is reproduced here.
use {
    smash::{
        app::{lua_bind::*, *},
        lib::lua_const::*,
        lua2cpp::*,
    },
    super::state,
};

/// THE CATCH WINDOW, STRAIGHT OUT OF THE ACMD, so the port cannot catch at a time Incineroar
/// could not.
///
/// `vendor/SSBU-Dumped-Scripts/.../SpecialSStart.txt`:
///
///     frame 15  ATTACK id 0  (5.0 damage, angle 350)     -- a plain hitbox, not a grab
///     frame 16  ATTACK id 1
///     frame 17  GrabModule::set_rebound(true)
///     frame 18  AttackModule::clear_all(); CATCH id 0; CATCH id 1
///     frame 35  grab!(GRAB_CLEAR_ALL); set_rebound(false)
///
/// So the grab is live for frames 18 to 35 and the two frames before it are an ordinary 5% hit.
/// Looking for a victim outside that span would let the port grab where Incineroar merely hits.
const CATCH_FIRST_FRAME: f32 = 18.0;
const CATCH_LAST_FRAME: f32 = 35.0;

/// The union of the two CATCH capsules, in Pikachu's world space.
///
/// From the same ACMD, with every figure divided by Pikachu's 0.96 `scale` so it lands in the same
/// world space as his body:
///
///   id 0   size 4.166667   at (0, 8.333334, 2.0833335) .. (0, 8.333334, 8.333334)
///   id 1   size 1.5625001  at (0, 8.333334, 2.0833335) .. (0, 8.333334, 10.937501)
///
/// On `top`, Z is forward and Y is up. Their union reaches from 2.08 to 10.94 forward, with the
/// wide box covering 8.33 +/- 4.17 vertically.
const CATCH_REACH_Z: f32 = 10.937501;
const CATCH_TOP_Y: f32 = 8.333334 + 4.166667;

/// The fighter the engine put in our grab, and how we know.
///
/// TWO SIGNALS, BECAUSE THE ENGINE GIVES HALF AN ANSWER. The capture link without the status means
/// it caught them but left them in their own status; the status without the link means it moved
/// them without recording a master. The port has to cope with either.
unsafe fn find_caught_victim(
    boma: *mut BattleObjectModuleAccessor,
) -> Option<(u32, *mut BattleObjectModuleAccessor)> {
    let me = (*boma).battle_object_id;
    for (id, vboma) in crate::victim::others(me) {
        if CaptureModule::is_capture(vboma) && CaptureModule::catch_object_id(vboma) as u32 == me {
            return Some((id, vboma));
        }
        if StatusModule::status_kind(vboma) == *FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED {
            return Some((id, vboma));
        }
    }
    None
}

/// Whoever is standing inside the catch capsules, ignoring whether the engine noticed.
///
/// THE LAST RESORT, AND IT IS ONLY REACHED IF THE ENGINE DECLINES THE CATCH ENTIRELY. A CATCH
/// collision belongs to the fighter that created it, and there is no guarantee the engine links one
/// created by a Pikachu running Incineroar's script -- the native handler checks the fighter kind.
/// If it does link, `find_caught_victim` sees it and this is never called.
///
/// It deliberately does NOT take someone another fighter is already holding, or anyone in a state a
/// grab could not take.
unsafe fn find_victim_in_range(
    boma: *mut BattleObjectModuleAccessor,
) -> Option<(u32, *mut BattleObjectModuleAccessor)> {
    let lr = PostureModule::lr(boma);
    let (mx, my) = (PostureModule::pos_x(boma), PostureModule::pos_y(boma));
    let mut best: Option<(f32, u32, *mut BattleObjectModuleAccessor)> = None;
    for (id, vboma) in crate::victim::others((*boma).battle_object_id) {
        if CaptureModule::is_capture(vboma) {
            continue;
        }
        let st = StatusModule::status_kind(vboma);
        if st == *FIGHTER_STATUS_KIND_DEAD
            || st == *FIGHTER_STATUS_KIND_REBIRTH
            || st == *FIGHTER_STATUS_KIND_ENTRY
        {
            continue;
        }
        let forward = (PostureModule::pos_x(vboma) - mx) * lr;
        let dy = PostureModule::pos_y(vboma) - my;
        if forward < 0.0 || forward > CATCH_REACH_Z || dy < -CATCH_TOP_Y || dy > CATCH_TOP_Y {
            continue;
        }
        if best.map(|(d, _, _)| forward < d).unwrap_or(true) {
            best = Some((forward, id, vboma));
        }
    }
    best.map(|(_, id, vboma)| (id, vboma))
}

/// Claims the victim, both halves.
///
/// THE LINK FIRST, WHICH IS WHAT WAS MISSING FOR SIX ROUNDS.
///
/// `CaptureModule::capture` sets the capture bookkeeping -- `is_capture` goes true and
/// `catch_object_id` names us -- and it does NOT create the link. The victim's own status finds its
/// captor with `LinkModule::get_parent_id(LINK_NO_CAPTURE)`, so without a link it cannot find us,
/// and `docs/the-recording.md` caught the consequence exactly:
///
///   ours:        mario1 ControlModule::start_clatter(0,  0, 0, -1, 0, false, false)
///   Incineroar:  mario1 ControlModule::start_clatter(85, 0, 5, -1, 0, false, false)
///
/// 85, 0 and 5 are `special_s_catch_clatter_frame`, `_damage_mul_frame_add` and `_dec_frame`, read
/// off the CAPTOR. Our victim read zeroes because the accessor it resolved for its parent was null,
/// took the fallback branch, and started a clatter of zero frames -- over on the frame it began.
/// `src/params` had been correct and unreachable the whole time.
///
/// Ghidra on `LinkModule::link` at main + 0x47f120 shows it splices the slot into TWO doubly linked
/// lists, one on the parent and one on the child, so a single call establishes both directions --
/// which is what `get_parent_id` on their side needs.
unsafe fn take_captive(
    fighter: &mut L2CFighterCommon,
    them: u32,
    vboma: *mut BattleObjectModuleAccessor,
) {
    let boma = fighter.module_accessor;
    let me = (*boma).battle_object_id;
    let entry = super::entry_of(boma);

    let linked_now = LinkModule::link(boma, *LINK_NO_CAPTURE, them) != 0;
    CaptureModule::capture(vboma, me, *STATUS_KIND_NONE, true, 0);

    // BOTH ENDS OF THE LINK, because one call only makes one end.
    //
    // The playtest that first reached this code reported:
    //
    //     caught 0x10000: is_capture=true master=0x0 (ours 0x0) link_now=true is_link=false
    //
    // The capture took and we are correctly the master, but the VICTIM had no parent link -- and
    // `docs/the-recording.md` records that every branch of their `SwingGaogaenCatched` status is
    // gated behind `is_link`, so their status ran none of it and they simply stood there.
    //
    // Ghidra on `LinkModule::link` at main + 0x47f120 says why. `Link(this, link_no, id)` writes
    // into the CALLER's own slot -- `slot.parent = this`, `slot.id = id` at `+0x30`/`+0x48` -- and
    // splices the caller's lists. It does not touch the object named by `id`. The comment in this
    // file used to say a single call "establishes both directions"; that was a misreading of the
    // list splicing, which threads two lists belonging to the CALLER, not one list on each side.
    //
    // For Incineroar the engine sets up both ends itself as part of the kind-keyed catch handler.
    // Since that handler is exactly what this file substitutes for, the port has to make the
    // victim's end too.
    let victim_linked = LinkModule::link(vboma, *LINK_NO_CAPTURE, me) != 0;

    crate::victim::request_status(
        vboma,
        *FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED,
        "whip catch",
    );

    // THE ONE WRITE THE SOURCE NEVER MAKES. Every later whip status reads
    // `..._INT_BATTLE_OBJECT_ID_SWING_THROWN_FIGHTER`, and for Incineroar the engine fills it in.
    // Storing it here is what makes the transcribed statuses work unmodified -- `whip_victim_held`,
    // `victim_pos_x` and `whip_release` all read it back through `state`.
    state::set_swing_thrown_fighter(entry, them);

    crate::telemetry::sink::diag(format!(
        "[rat] WHIP p{entry} caught {them:#x}: is_capture={} master={:#x} (ours {me:#x}) \
         ours_link={linked_now} their_link={victim_linked} \
         their_is_link={} their_parent={:#x} their_st={:#x} their_dmg={:.1}",
        CaptureModule::is_capture(vboma),
        CaptureModule::catch_object_id(vboma) as u32,
        LinkModule::is_link(vboma, *LINK_NO_CAPTURE),
        LinkModule::get_parent_id(vboma, *LINK_NO_CAPTURE, false) as u32,
        StatusModule::status_kind(vboma),
        // THEIR DAMAGE, because the same playtest reported the victim taking 7.5% of electric
        // damage from something this move does not own -- the port's whip hitboxes are damage 0.0
        // with `collision_attr_normal`, and the whole ported plugin contains no electric hitbox at
        // all. Logging it at the catch gives a baseline to compare against the next frames, which
        // separates "the catch did it" from "they were already being hit by something else".
        DamageModule::damage(vboma, 0)
    ));
}

/// Looks for a catch this frame, and claims one if it finds it.
///
/// Returns true when the caller should transition into SPECIAL_S_THROW. Called from exactly one
/// place -- `special_s_main_loop` -- so the substitution has a single site.
pub unsafe fn try_catch(fighter: &mut L2CFighterCommon) -> bool {
    let boma = fighter.module_accessor;
    let entry = super::entry_of(boma);

    // Already holding someone: the throw is running and this must not fire twice.
    if state::swing_thrown_fighter(entry) != *BATTLE_OBJECT_ID_INVALID as u32 {
        return false;
    }

    let frame = MotionModule::frame(boma);
    if frame < CATCH_FIRST_FRAME || frame > CATCH_LAST_FRAME {
        return false;
    }

    let found = find_caught_victim(boma).or_else(|| find_victim_in_range(boma));
    let Some((them, vboma)) = found else {
        // INSTRUMENTED, BECAUSE THE FIRST PLAYTEST OF THE REWRITE PRODUCED NO WHIP LINES AT ALL.
        // "Doesn't grab even at point blank" and "no log line" have several possible causes and
        // guessing between them is what the last two rounds of this move were made of: the loop
        // might not be running, the motion frame might never reach the window, `victim::others`
        // might be returning nothing, or the range test might be rejecting a fighter who is right
        // there. One line on three frames of the window distinguishes all four.
        if frame == CATCH_FIRST_FRAME || frame == 26.0 || frame == CATCH_LAST_FRAME {
            // WHY the candidate was rejected, not just that it was. The previous round of this
            // diagnostic reported `others=1 in_range=false` -- an opponent present and the range
            // test refusing them at point blank -- which narrows the cause to one of four
            // conditions in `find_victim_in_range` without saying which. Printing the operands
            // settles it: `forward` outside [0, 10.94] means geometry (and a NEGATIVE forward
            // means we are facing away, which is a different bug entirely), `dy` outside +/-12.5
            // means height, `cap=true` means a capture leaked from an earlier attempt and is
            // making them permanently ineligible, and an odd `vst` means they are in a state a
            // grab refuses.
            let lr = PostureModule::lr(boma);
            let (mx, my) = (PostureModule::pos_x(boma), PostureModule::pos_y(boma));
            let mut detail = String::new();
            for (id, vboma) in crate::victim::others((*boma).battle_object_id) {
                detail.push_str(&format!(
                    " [{id:#x} fwd={:.2} dy={:.2} cap={} vst={:#x}]",
                    (PostureModule::pos_x(vboma) - mx) * lr,
                    PostureModule::pos_y(vboma) - my,
                    CaptureModule::is_capture(vboma),
                    StatusModule::status_kind(vboma)
                ));
            }
            crate::telemetry::sink::diag(format!(
                "[rat] WHIP p{} f{frame} no catch: lr={lr} reach={CATCH_REACH_Z:.2} \
                 topy={CATCH_TOP_Y:.2}{detail}",
                super::entry_of(boma)
            ));
        }
        return false;
    };
    take_captive(fighter, them, vboma);
    true
}
