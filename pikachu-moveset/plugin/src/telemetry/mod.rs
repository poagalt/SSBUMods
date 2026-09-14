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
mod motions;
pub mod selftest;
pub mod probe;
mod buildstamp;
pub mod exedump;
// Serves main's mapped regions over TCP -- the sink exedump could not use. See memserve.rs.
pub mod memserve;
// Records what the engine does when a REAL fighter holds a victim -- see enginetrace.rs.
pub mod enginetrace;
pub mod moves;

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
/// The victim a SOURCE fighter is holding, latched -- see `trace_source_victim`.
static mut NATIVE_VICTIM: [u32; 8] = [0; 8];
/// One IDFORMAT line per player is enough; the answer does not change during a match.
static mut LOGGED_CAPTURE_ID: [bool; 8] = [false; 8];
/// Last damage value logged, so a change can be reported as an event rather than polled for.
static mut LAST_LOGGED_DAMAGE: [f32; 8] = [0.0; 8];

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


/// Short name for a fighter kind, so a trace line says who it is about.
///
/// Only the fighters this project touches. Anything else prints its raw kind, which is still
/// enough to tell two fighters apart in a log.
fn kind_tag(kind: i32) -> &'static str {
    unsafe {
        if kind == *FIGHTER_KIND_PIKACHU { "pika" }
        else if kind == *FIGHTER_KIND_GAOGAEN { "GAOGAEN" }
        else if kind == *FIGHTER_KIND_DONKEY { "DONKEY" }
        else if kind == *FIGHTER_KIND_GAMEWATCH { "GAMEWATCH" }
        else if kind == *FIGHTER_KIND_PACKUN { "PACKUN" }
        else { "other" }
    }
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
        // A LOAD-CRASH BISECTION MARKER, flushed immediately. Fighter object init happens before
        // any status or ACMD runs, so if the plugin dies there this line never appears -- and its
        // ABSENCE next to the install-time lines is the diagnostic. Cheap: once per entry.
        sink::diag(format!("[rat] p{entry} pikachu object is alive; status/ACMD lines follow"));
        dump_params(boma, entry);
        // Ask the game what it actually loaded, once, while we have a live object to ask through.
        // `global_table[0xC]` is where smashline writes the extended status-kind count
        // (vendor/sl_create_agent.rs:738), so this is also the only way to confirm from inside the
        // running game that the seven new kinds registered rather than assuming they did.
        selftest::run(boma, entry, fighter.global_table[0xC].get_int() as i32);
        // The game's own code, if asked for. Off by default -- see exedump.rs.
        exedump::run(sink::prefix().as_str());
    }
    // The self-test's second pass, two seconds in. Which motions are missing has come back
    // differently from two loads of the same files, so asking once is not a measurement.
    selftest::tick(boma, entry);
    trace_source_victim(boma, entry);
    mirror_probe(boma, entry);
    // ONE PLACE THAT KNOWS A FRAME HAS PASSED. The per-frame status-change budget in
    // src/victim.rs is reset here, and only here: if a status cycle is running away the frame
    // never ends, so this never runs and the counter keeps climbing until it refuses. That is
    // what makes it a detector rather than a guess.
    crate::victim::new_frame(entry);
    crate::victim::tick_hold(entry);
    crate::victim::tick_deferred_cut(entry);
    // THE FORWARD THROW'S DESTINATION IS A SLOT IN THE GLOBAL TABLE, AND DK'S HOLDS THE CARRY.
    // This is the one assignment that makes the cargo native. See `specials::shoulder::retarget`.
    crate::specials::shoulder::retarget_forward_throw(fighter);

    let motion = MotionModule::motion_kind(boma);
    let frame = MotionModule::frame(boma);
    let end = MotionModule::end_frame(boma);
    let status = StatusModule::status_kind(boma);
    // ONE LINE PER MOVE ATTEMPT, which is the index into everything else in this log.
    moves::tick(boma, entry, status);
    let pos_x = PostureModule::pos_x(boma);
    let pos_y = PostureModule::pos_y(boma);

    // WHICH FIGHTER THIS LINE IS ABOUT. The same callback is installed on the source fighters now
    // (see SOURCE_AGENTS), so `p0` alone is ambiguous the moment an Incineroar is in the match --
    // and a trace of the real move mistaken for a trace of ours would be worse than no trace.
    let who = kind_tag(smash::app::utility::get_kind(&mut *boma));

    // DAMAGE EVENTS, FLUSHED, because the counter is defined by one. Tracing what Incineroar does
    // when Revenge absorbs a hit needs the exact frame it arrived on and how much it was, and the
    // per-motion lines cannot show that -- damage does not change the motion.
    let dmg = DamageModule::damage(boma, 0);
    if (dmg - LAST_LOGGED_DAMAGE[entry]).abs() > 0.01 {
        probe::event(format!(
            "[rat] {who}{entry} DAMAGE {:+.1} -> {dmg:.1}% on frame {frame:.0} of {motion:#012x}, \
             {}",
            dmg - LAST_LOGGED_DAMAGE[entry],
            probe::st(status)
        ));
        LAST_LOGGED_DAMAGE[entry] = dmg;
    }

    if motion != LAST_MOTION[entry] {
        // Report how long the motion we are leaving actually lasted, which is the number that
        // matters for perceived endlag, plus how far it carried him. `peak` is measured against
        // the height the motion started at, so a short hop's apex is directly comparable with
        // and without an aerial attack.
        if LAST_MOTION[entry] != 0 {
            rat!(
                "[rat] {}{} motion {:#012x} ended after {:.0}f, moved x {:+.2} y {:+.2} peak {:+.2}",
                who, entry, LAST_MOTION[entry], MOTION_START[entry],
                pos_x - START_X[entry], pos_y - START_Y[entry], PEAK_Y[entry] - START_Y[entry]
            );
        }
        rat!(
            "[rat] {}{} motion {:#012x} start  end={:.0}  status={}  y={:.2}",
            who, entry, motion, end, status, pos_y
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
        // FLUSHED IMMEDIATELY, AND WITH NAMES. Two lessons from the round that found the cargo
        // freeze, both of which cost time rather than being merely untidy:
        //
        // The buffer threw the crash away. Telemetry flushed every 120 frames, so a hard freeze
        // lost up to two seconds -- precisely the frames between entering the throw and dying in
        // it. Status changes are rare (a handful a second at most), so paying a file write for
        // each one is free, and it guarantees the last thing the fighter did survives a freeze.
        //
        // And `status 3 -> 477` needed a lookup table that lived in someone's head. It now reads
        // `DASH(0x3) -> SPECIAL_S(0x1dd)`, and the moveset's own kinds resolve by name too, so
        // seeing RAT_SPECIAL_S_SHOULDER in the log is direct evidence the grab connected.
        probe::event(format!(
            "[rat] {}{} status {} -> {} on frame {:.0} of {:#012x} (anim end {:.0})",
            who, entry, probe::st(LAST_STATUS[entry]), probe::st(status), frame, motion, end
        ));
        LAST_STATUS[entry] = status;
    }

    // LEARNING THE BATTLE-OBJECT ID FORMAT FROM THE GAME, ONCE, DURING A GRAB THAT WORKS.
    //
    // Three separate features are blocked on the same missing fact: how to turn an entry id into
    // the battle object id that `sv_battle_object::module_accessor` wants. Revenge cannot freeze
    // the attacker, the whip cannot read whether its victim entered SWING_GAOGAEN_CATCHED, and
    // the cargo carry cannot check what its victim is doing. Each was left undone rather than
    // guessed, because a bad pointer handed to an engine function is how the cargo throw freezes.
    //
    // Pikachu's ORDINARY grab works and the engine fills in the id itself. So: on the first frame
    // of any catch status, log our own entry id next to the captured object's id. One line, no
    // pointer dereferenced that the engine did not already hand us, and it settles the format
    // from real data instead of from a guess. Round 2 already showed the invalid sentinel is
    // 0x50000000, so the answer is probably 0x50000000 | entry -- this confirms or refutes it.
    if !LOGGED_CAPTURE_ID[entry]
        && (status == *FIGHTER_STATUS_KIND_CATCH
            || status == *FIGHTER_STATUS_KIND_CATCH_WAIT
            || status == *FIGHTER_STATUS_KIND_CATCH_PULL)
    {
        let cap = CatchModule::capture_object_id(boma, false);
        if cap != *BATTLE_OBJECT_ID_INVALID as u64 {
            LOGGED_CAPTURE_ID[entry] = true;
            probe::event(format!(
                "[rat] IDFORMAT p{entry} captured object id = {cap:#x} while I am entry {entry}; \
                 invalid sentinel is {:#x}. If the victim is p1 this says 0x5000000n.",
                *BATTLE_OBJECT_ID_INVALID
            ));
        }
    }

    // One flush check per frame; the sink itself decides when to actually write.
    sink::tick();
}

/// The fighters this moveset ports from. The same telemetry installs on each of them.
///
/// TRACING THE REAL MOVE IS THE ONLY GROUND TRUTH LEFT, and not having it is why Revenge has been
/// wrong three times.
///
/// Everything this project can read offline has been read. The ACMD dumps have the animation
/// commands and nothing else; the status logic that actually implements a counter, a command grab
/// or a cargo carry is C++ in the game binary and appears in no dump. vl.prc gives the constants
/// but not what consumes them. HewDraw Remix reimplements fighters in Rust and for Incineroar's
/// Revenge it overrides only the `Pre` -- it lets the game run the counter, so it has no answer
/// either. Public frame data gives windows, not mechanics.
///
/// So the remaining source of truth is the game itself, running. Install the same per-frame
/// telemetry on Incineroar, Donkey Kong, Game & Watch and Piranha Plant, play the real move, and
/// the log records exactly what the engine does: every status it passes through in order, the
/// motion and frame of each, what it does to speed, and when damage lands. Then our Pikachu doing
/// the same move produces the same shape of log, and the two can be diffed line for line.
///
/// That turns "implement it faithfully" from a reading exercise into a comparison. It is also
/// free: no extra console work, no dumping, no disassembly -- just play Incineroar for a minute.
///
/// Costs nothing when they are not in the match, since the callback only runs for fighters that
/// exist.
const SOURCE_AGENTS: &[&str] = &["gaogaen", "donkey", "gamewatch", "packun"];

pub fn install() {
    // First thing written each boot, so the log always starts with a known marker and a missing
    // file means the sink itself failed rather than the plugin not loading.
    sink::diag(format!("[rat] telemetry installed; plugin BUILD {}", buildstamp::BUILD));
    // Armed before any agent, because the calls worth recording happen inside the engine and not
    // inside a status script this plugin owns.
    enginetrace::install();
    // Its own thread, so a blocking Accept never touches the game thread.
    memserve::install();
    Agent::new("pikachu")
        .on_line(Main, pikachu_telemetry)
        .install();

    for name in SOURCE_AGENTS {
        Agent::new(*name)
            .on_line(Main, pikachu_telemetry)
            .install();
    }
    sink::diag(format!(
        "[rat] source tracing armed on {} -- play one of them and the log records what the \
         REAL move does, frame by frame, to diff against ours",
        SOURCE_AGENTS.join(", ")
    ));
}

/// THE ONE MEASUREMENT THE WHIP'S TIMING CANNOT BE DERIVED WITHOUT, and it needs one minute of
/// native Incineroar rather than another disassembly.
///
/// What is known from the game's files: the victim is thrown at
/// `special_s_through_target_speed` = 11.2, continues past the rope at `_speed_over_rope` = 1.6,
/// comes back at `_return_speed` = 2.8, and the leg is bounded by `_cancel_frame` = 30. What is
/// NOT in any file is how long the ROPE holds them -- how many frames of stretch before it snaps
/// them back -- because that belongs to the rope article, and this port has no rope.
///
/// The port currently passes through `SWING_GAOGAEN_ATTACH_ROPE` in one frame, which is defensible
/// (its `Main` has no `change_status`, so it cannot time itself) and is measurably too short: the
/// playtest reports "the timing needed to get the ideal lariat is way too fast" and "it seems like
/// you're missing pieces of animation for the bounce against the rope". Native Incineroar's throw
/// status runs 52 frames against our 33.
///
/// Subtracting one from the other gives 19 and that is exactly the kind of number this project
/// does not get to invent. So this records the real thing instead. It runs ONLY for a source
/// fighter -- Incineroar, Donkey Kong, Game & Watch, Piranha Plant -- never for our Pikachu, so it
/// costs nothing in an ordinary session and produces, per frame of a native Alolan Whip:
///
///   * the victim's status and its frame, so each leg's length is read off rather than inferred
///   * their x relative to the captor, so the rope's distance and both speeds are measurable
///   * the captor's own status frame, to line the two up
///
/// One native whip in the log settles the stretch duration, the rope distance and the width of the
/// lariat window together.
unsafe fn trace_source_victim(boma: *mut smash::app::BattleObjectModuleAccessor, entry: usize) {
    // Never for us. `SOURCE_AGENTS` installs this same callback on the source fighters, and the
    // only thing that tells the two apart at runtime is the fighter kind.
    if utility::get_kind(&mut *boma) == *FIGHTER_KIND_PIKACHU {
        return;
    }
    let st = StatusModule::status_kind(boma);
    // Gaogaen's six side-B statuses are 0x1dd (common SPECIAL_S) and 0x1e0..0x1e5 in HIS table.
    // Written as a range rather than by name because the names are his and must not be linked
    // against -- see tools/check-status.sh.
    if st != *FIGHTER_STATUS_KIND_SPECIAL_S && !(0x1e0..=0x1e5).contains(&st) {
        return;
    }
    // LATCHED, BECAUSE THE ENGINE UNLINKS THE VICTIM THE MOMENT IT THROWS THEM -- AND THAT BLIND
    // SPOT MADE ME READ THE LAST RECORDING BACKWARDS.
    //
    // Resolving the victim through LINK_NO_CAPTURE every frame means this stops logging for the
    // whole flight: `status_SwingGaogaenThrown`'s first act is `LinkModule::unlink(link=0)`, which
    // the recording shows plainly. So twelve native whips produced a trace that went
    // CATCHED -> follow-up with nothing in between, and I concluded the victim never left CATCHED
    // and the rope flight was a rare branch. The playtest says otherwise and is right: "every time
    // I hit alolan whip with native incin the opponent flies into the ropes -- it's what rebounds
    // them from traveling away from incineroar back towards incineroar for the lariat timing."
    //
    // The missing frames are exactly the ones worth having. Latching the id while the link exists
    // and keeping it for the rest of the move records the flight, the rope, the rebound and the
    // window, which is the whole measurement.
    let linked = LinkModule::get_node_object_id(boma, *LINK_NO_CAPTURE) as u32;
    if crate::victim::boma(linked).is_some() {
        NATIVE_VICTIM[entry] = linked;
    }
    let them = NATIVE_VICTIM[entry];
    let Some(vboma) = crate::victim::boma(them) else {
        return;
    };
    // THE FOUR FLAGS, WHICH ARE THE WHOLE REMAINING QUESTION ABOUT THE LARIAT.
    //
    // The three areas resolve, from Incineroar's own prc, to VERTICAL TRIPWIRES ON `top`:
    //
    //     special_s_lariat_node     = top   size 2   x1,y1 = -24,12   x2,y2 = -24,2
    //     special_s_lariat_end_node = top   size 2   x1,y1 = -10,12   x2,y2 = -10,2
    //     special_s_failure_node    = top   size 2   x1,y1 =  -6,12   x2,y2 =  -6,2
    //
    // x1 == x2 in all three, so each is a line at a fixed x spanning knee-to-head height with a
    // radius of 2 -- which makes the port's scalar comparison nearly right and leaves one thing
    // unexplained. In twelve recorded native whips the victim's offset never went below -5.46, so
    // NONE of those three lines was ever crossed; and Incineroar still reached SPECIAL_S_LARIAT
    // twice, SPECIAL_S_SHOULDER twice and SPECIAL_S_FAILURE three times. The flags were raised by
    // something other than the geometry this port computes.
    //
    // The leading candidate is whose `top` the lines hang off: anchored to the VICTIM's root
    // instead of the captor's, "x = -24" is a line 24 units behind the VICTIM, which the captor
    // sweeps across during the swing -- and the numbers would then work out for exactly the
    // positions the recording shows. That is a guess, and guessing at this is how the last three
    // rounds went, so the flags are logged instead.
    //
    // Reading them off the SOURCE fighter is safe and is the point: on Incineroar these ids are
    // his own, in range, and mean what they say.
    let flag = |id: i32| -> u8 { WorkModule::is_flag(boma, id) as u8 };
    sink::diag(format!(
        "[rat] NATIVE p{entry} captor st={st:#x} f={:.0} | victim {them:#x} st={:#x} f={:.0} \
         off={:.2} vy={:.2} clatter={:.0} | rope={}{} area L={} Lend={} F={} sh={}",
        MotionModule::frame(boma),
        StatusModule::status_kind(vboma),
        MotionModule::frame(vboma),
        (PostureModule::pos_x(vboma) - PostureModule::pos_x(boma)) * PostureModule::lr(boma),
        PostureModule::pos_y(vboma) - PostureModule::pos_y(boma),
        ControlModule::get_clatter_time(vboma, 0),
        flag(*FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE),
        flag(*FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_THROW_ROPE_END),
        flag(*FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_AREA),
        flag(*FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_LARIAT_END_AREA),
        flag(*FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_FAILURE_AREA),
        flag(*FIGHTER_GAOGAEN_STATUS_SPECIAL_S_FLAG_SHOULDER),
    ));
}

/// THE MIRROR PROBE: does a ported hitbox's anchor bone follow Pikachu when he turns around?
///
/// "chrom jab and palutena nair aren't correctly mirrored when pikachu is facing left vs right --
/// they both only work correctly when facing the direction pikachu spawns in."
///
/// Those two moves have exactly one thing in common that no other ported move has, and
/// `tools/bone-remap.sed` says it in its own header: they are the only two whose hitboxes ride a
/// GRAFTED bone. Chrom's jab rides `Sword1`, Palutena's n-air rides `Stick`, and
/// `tools/add-bones.sh` adds both to Pikachu's skeleton as children of `HaveR`.
///
/// EVERYTHING STATIC ABOUT THAT GRAFT CHECKS OUT, which is why this is an instrument and not a
/// fix. The shipped `model.nusktb` has `Sword1` at index 78 and `Stick` at 79, both with
/// `parent_index = 22`, and index 22 is `HaveR`, whose own parent chain runs up through `ArmR` --
/// so they inherit the facing the way every other bone does. The motion entries are right too:
/// `attack_11` has `move` set exactly as Chrom's does, `attack_air_n` has it clear exactly as
/// Palutena's does. So the theory that the graft is the cause has one fact behind it (the
/// correlation) and no mechanism, and guessing at a mechanism is what this project is not allowed
/// to do.
///
/// WHAT IT MEASURES. Once per (motion, facing) pair, the anchor bones' world offset from `top`,
/// expressed in the FIGHTER'S OWN frame by multiplying the forward axis by `lr`. In that frame a
/// bone that follows him reads the SAME both ways round, and a bone that does not flips sign. Two
/// lines per move -- one facing right, one facing left -- and the diff is the answer.
///
/// `haver` is logged beside them as the control: it is a vanilla Pikachu bone, it is the grafted
/// bones' parent, and if IT flips too then the graft is innocent and the problem is the animation.
static mut MIRROR_SEEN: [u32; 8] = [0; 8];

unsafe fn mirror_probe(boma: *mut smash::app::BattleObjectModuleAccessor, entry: usize) {
    let motion = MotionModule::motion_kind(boma);
    // The two moves reported, and `attack_dash` as a third control: a ported move with `move` set
    // whose hitboxes ride `top`, which the playtest says mirrors correctly.
    let which = if motion == hash40("attack_11") {
        1u32
    } else if motion == hash40("attack_air_n") {
        2
    } else if motion == hash40("attack_dash") {
        4
    } else {
        return;
    };
    let lr = PostureModule::lr(boma);
    // One bit per (move, facing), so each combination logs once and then costs a mask test.
    let bit = which << if lr < 0.0 { 3 } else { 0 };
    if MIRROR_SEEN[entry] & bit != 0 {
        return;
    }
    MIRROR_SEEN[entry] |= bit;
    let base = Vector3f {
        x: PostureModule::pos_x(boma),
        y: PostureModule::pos_y(boma),
        z: 0.0,
    };
    let local = |bone: &str| -> String {
        // Writes through an out-parameter rather than returning, so a bone the skeleton does not
        // have leaves the buffer untouched -- which is why it is seeded with a sentinel.
        let mut p = Vector3f { x: -9999.0, y: -9999.0, z: 0.0 };
        ModelModule::joint_global_position(boma, Hash40::new(bone), &mut p, true);
        if p.x < -9000.0 {
            return format!("{bone}=absent");
        }
        // World x is the stage's left-right; multiplying by `lr` expresses it in the fighter's own
        // frame, so a bone that follows him reads the same both ways round.
        format!("{bone}=({:.2},{:.2})", (p.x - base.x) * lr, p.y - base.y)
    };
    sink::diag(format!(
        "[rat] MIRROR p{entry} {} lr={lr:.0} f={:.0} {} {} {}",
        match which {
            1 => "attack_11(chrom,Sword1)",
            2 => "attack_air_n(palutena,Stick)",
            _ => "attack_dash(mythra,top -- control)",
        },
        MotionModule::frame(boma),
        local("sword1"),
        local("stick"),
        local("haver"),
    ));
}
