// Playtest telemetry. Logs over Skyline's TCP channel, readable live with
//
//     cargo skyline listen --ip <console-ip>
//
// which works while the game is running -- exactly when FTP does not.
//
// Every line is prefixed `[rat]` so it can be grepped out of the very chatty Skyline log,
// and motion/status kinds are logged as raw hash40 values. Resolve them locally with
// `tools/motion-patch/target/release/h40.exe <name>` rather than shipping a dictionary.
//
// The high-value field here is `end` (MotionModule::end_frame): the driving animation's true
// length. Endlag on a ported move is bounded by that, not by the FAF in motion_list, and
// measuring it settles what was otherwise a guess. `faf` is what we patched it to.
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

pub const ENABLED: bool = true;

// One slot per player, indexed by entry id.
static mut LAST_MOTION: [u64; 8] = [0; 8];
static mut LAST_STATUS: [i32; 8] = [-1; 8];
static mut MOTION_START: [f32; 8] = [0.0; 8];

unsafe extern "C" fn pikachu_telemetry(fighter: &mut L2CFighterCommon) {
    if !ENABLED {
        return;
    }
    let boma = fighter.module_accessor;
    let entry = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if entry >= 8 {
        return;
    }

    let motion = MotionModule::motion_kind(boma);
    let frame = MotionModule::frame(boma);
    let end = MotionModule::end_frame(boma);
    let status = StatusModule::status_kind(boma);

    if motion != LAST_MOTION[entry] {
        // Report how long the motion we are leaving actually lasted, which is the number that
        // matters for perceived endlag.
        if LAST_MOTION[entry] != 0 {
            println!(
                "[rat] p{} motion {:#012x} ended after {:.0}f",
                entry, LAST_MOTION[entry], MOTION_START[entry]
            );
        }
        println!(
            "[rat] p{} motion {:#012x} start  end={:.0}  status={}",
            entry, motion, end, status
        );
        LAST_MOTION[entry] = motion;
        MOTION_START[entry] = 0.0;
    } else {
        MOTION_START[entry] = frame;
    }

    // Status changes carry the motion frame they happened on. Leaving an attack status is the
    // frame you actually become actionable, which is the number that decides whether our
    // patched FAF is being honoured or whether the animation's length is governing instead.
    if status != LAST_STATUS[entry] {
        println!(
            "[rat] p{} status {} -> {} on frame {:.0} of {:#012x} (anim end {:.0})",
            entry, LAST_STATUS[entry], status, frame, motion, end
        );
        LAST_STATUS[entry] = status;
    }
}

pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_telemetry)
        .install();
}
