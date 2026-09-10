// Playtest telemetry, written to TWO places so neither has to be arranged in advance.
//
//     cargo skyline listen --ip <console-ip>      live, needs attaching before the thing happens
//     <sd>/rat-telemetry.log                       after the fact, over FTP -- see sink.rs
//
// The old note here said the TCP log works "exactly when FTP does not". That was wrong, and
// believing it cost a lot of back and forth: sys-ftpd runs as a boot sysmodule and serves the card
// whatever the console is doing. The file is therefore the primary channel -- a session can be
// played with nobody watching and read afterwards.
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

pub mod sink;

pub const ENABLED: bool = true;

// Every telemetry line goes to BOTH the TCP log and the file on the card.
//
// The TCP log needs `cargo skyline listen` attached at the moment the thing happens, which means
// coordinating with whoever is holding the controller. The file does not -- sys-ftpd serves it
// regardless of what the console is running, so a session can be played now and read later. Same
// text either way, so nothing has to be re-derived from a different format.
macro_rules! rat {
    ($($arg:tt)*) => {{
        let line = format!($($arg)*);
        println!("{}", line);
        crate::telemetry::sink::log(line);
    }};
}

// One slot per player, indexed by entry id.
static mut LAST_MOTION: [u64; 8] = [0; 8];
static mut LAST_STATUS: [i32; 8] = [-1; 8];
static mut MOTION_START: [f32; 8] = [0.0; 8];
static mut START_X: [f32; 8] = [0.0; 8];
// Vertical travel, and the highest point reached during a motion.
//
// A dropped-in animation supplies the SOURCE fighter's `Rot` standing height -- around 11 model
// units against Pikachu's 5.22 -- so his body is DRAWN about 5.7 world units higher during these
// moves. Playtest read that as the aerials "carrying pikachu higher/lower depending on the jump
// height of the character the attack came from". None of the aerial motions has the `move` flag
// and their Trans tracks are flat, so the animation cannot be moving him; his trajectory should
// be pure physics. This logs it instead of arguing it: compare peak_y for a short hop with an
// aerial against a short hop without one.
static mut START_Y: [f32; 8] = [0.0; 8];
static mut PEAK_Y: [f32; 8] = [0.0; 8];

static mut DUMPED_PARAMS: [bool; 8] = [false; 8];

// Reads the attribute values the game is actually using, so "are the stat changes working?"
// becomes a measurement rather than a guess. fighter_param.prcxml edits are invisible otherwise:
// if ARCropolis fails to apply the patch, everything still runs, just with vanilla numbers.
unsafe fn dump_params(boma: *mut smash::app::BattleObjectModuleAccessor, entry: usize) {
    let p = |name: &str| -> f32 { WorkModule::get_param_float(boma, smash::hash40(name), 0) };
    println!(
        "[rat] p{} PARAMS weight={:.2} walk={:.3} dash={:.3} run={:.3}",
        entry, p("weight"), p("walk_speed_max"), p("dash_speed"), p("run_speed_max")
    );
    println!(
        "[rat] p{} PARAMS jump_y={:.2} mini_jump_y={:.2} jump_aerial_y={:.2} jump_initial_y={:.3}",
        entry, p("jump_y"), p("mini_jump_y"), p("jump_aerial_y"), p("jump_initial_y")
    );
    println!(
        "[rat] p{} PARAMS air_x={:.3} air_y={:.3} dive_y={:.3} grav={:.3} air_acc_mul={:.3} air_acc_add={:.3}",
        entry, p("air_speed_x_stable"), p("air_speed_y_stable"), p("dive_speed_y"),
        p("air_accel_y"), p("air_accel_x_mul"), p("air_accel_x_add")
    );
    println!(
        "[rat] p{} EXPECTED weight=135 walk=1.575 dash=2.42 run=2.464 jump_y=50.51 \
         mini=17.34 aerial=50.51 init=27.781 air_x=1.332 air_y=1.63 dive=3.097 grav=0.13",
        entry
    );
}

unsafe extern "C" fn pikachu_telemetry(fighter: &mut L2CFighterCommon) {
    if !ENABLED {
        return;
    }
    let boma = fighter.module_accessor;
    let entry = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if entry >= 8 {
        return;
    }
    if !DUMPED_PARAMS[entry] {
        DUMPED_PARAMS[entry] = true;
        dump_params(boma, entry);
    }

    let motion = MotionModule::motion_kind(boma);
    let frame = MotionModule::frame(boma);
    let end = MotionModule::end_frame(boma);
    let status = StatusModule::status_kind(boma);
    let pos_x = PostureModule::pos_x(boma);
    let pos_y = PostureModule::pos_y(boma);

    if motion != LAST_MOTION[entry] {
        // Report how long the motion we are leaving actually lasted, which is the number that
        // matters for perceived endlag, plus how far it carried him. `peak` is measured against
        // the height the motion started at, so a short hop's apex is directly comparable with
        // and without an aerial attack.
        if LAST_MOTION[entry] != 0 {
            rat!(
                "[rat] p{} motion {:#012x} ended after {:.0}f, moved x {:+.2} y {:+.2} peak {:+.2}",
                entry, LAST_MOTION[entry], MOTION_START[entry],
                pos_x - START_X[entry], pos_y - START_Y[entry], PEAK_Y[entry] - START_Y[entry]
            );
        }
        rat!(
            "[rat] p{} motion {:#012x} start  end={:.0}  status={}  y={:.2}",
            entry, motion, end, status, pos_y
        );
        LAST_MOTION[entry] = motion;
        MOTION_START[entry] = 0.0;
        START_X[entry] = pos_x;
        START_Y[entry] = pos_y;
        PEAK_Y[entry] = pos_y;
    } else {
        MOTION_START[entry] = frame;
        if pos_y > PEAK_Y[entry] {
            PEAK_Y[entry] = pos_y;
        }
    }

    // Status changes carry the motion frame they happened on. Leaving an attack status is the
    // frame you actually become actionable, which is the number that decides whether our
    // patched FAF is being honoured or whether the animation's length is governing instead.
    if status != LAST_STATUS[entry] {
        rat!(
            "[rat] p{} status {} -> {} on frame {:.0} of {:#012x} (anim end {:.0})",
            entry, LAST_STATUS[entry], status, frame, motion, end
        );
        LAST_STATUS[entry] = status;
    }

    // One flush check per frame; the sink itself decides when to actually write.
    sink::tick();
}

pub fn install() {
    Agent::new("pikachu")
        .on_line(Main, pikachu_telemetry)
        .install();
}
