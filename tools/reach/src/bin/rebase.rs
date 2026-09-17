// Re-expresses a hitbox anchored to a GRAFTED bone in the frame of the real Pikachu bone it hangs
// off, using the grafted bone's own transform AT THE FRAME THE HITBOX IS CREATED ON.
//
// WHY: A GRAFTED BONE EXISTS IN THE FILE AND NOT AT RUNTIME, AND THE PROBE PROVED IT.
//
// `tools/add-bones.sh` adds Chrom's `Sword1` and Palutena's `Stick` to Pikachu's skeleton as
// children of `HaveR`, and every static check on that graft passes: both are at `parent_index = 22`
// in the shipped `model.nusktb`, index 22 is `HaveR`, the shipped animations animate them, and the
// FK parity harness measures `nair_palutena` at 0.00 deviation over 96 samples with `stick` as the
// anchor. It is correct on disk.
//
// It is not correct in the game. `telemetry::mirror_probe` logs each anchor bone's offset from
// `top` in the fighter's own frame, once per facing, with `haver` beside it as the control:
//
//   MIRROR attack_11(chrom,Sword1) lr=-1 sword1=(7.93,13.83) stick=(6.09,14.58) haver=(-1.68,8.89)
//   MIRROR attack_11(chrom,Sword1) lr= 1 sword1=(-0.02,0.29) stick=(-0.45,-0.29) haver=(-1.68,8.89)
//
// Same move, same frame, both facings. `haver` reads IDENTICALLY -- it mirrors, as every vanilla
// bone does. `sword1` and `stick` read differently from each other, differently between facings,
// and nowhere near `haver`, which is their parent: at `lr=1` they sit essentially at the fighter's
// origin, and in the n-air line below the feet. They are not in the runtime skeleton's transform
// chain at all. The playtest saw the same thing from the other side: "it looks like the jab hitbox
// is appearing on the sword, which is being brandished the wrong direction when pikachu jabs to
// the left", and "it looks like pikachu is trying to do chrom jab with just his hand".
//
// THE TRANSFORM COMES FROM THE ANIMATION, PER FRAME, AND THE FIRST VERSION OF THIS PASS GOT THAT
// WRONG. It used the SKELETON's bind transform for both bones, on the reasoning that a grafted bone
// is rigid relative to its parent. That is true of `Sword1` -- `c00attack11.nuanmb` gives it one
// Transform value, zero translation, a constant rotation -- and it rebased Chrom's jab to 0.00
// world units against the source. It is false of `Stick`: `c05attackairn.nuanmb` animates it with
// 460 keyframes of changing rotation AND translation, because Palutena sweeps the staff through the
// move. The harness said so immediately: `nair_palutena is 13.74 world units off`.
//
// So the transform is read out of the SHIPPED ANIMATION at the frame each `ATTACK` is created on,
// which the generated script states directly in the `frame(...)` call above it. A hitbox lives two
// or three frames, so within its own life the staff barely moves; across the move it moves a great
// deal, and that is the difference between 13.74 and what the harness now measures.
//
// Nothing here is hand-entered: the offsets come from the generated script, the transforms from the
// shipped `.nuanmb`, and the fallback from the shipped `model.nusktb`. Same rule as topscale's MAP,
// which exists because Cloud's scale was typed in as 1.03 against an actual 0.98.
//
// ORDER MATTERS IN THE BUILD: this runs BEFORE topscale, because topscale scales `top`-anchored
// offsets and this pass produces `haver`-anchored ones, which topscale correctly leaves alone.
//
// usage: rebase <plugin-src-dir> <skeleton-json> <anim-json-dir>
use std::collections::HashMap;

/// Which grafted bone to rebase onto what. EMPTY, AND THE EVIDENCE BELOW IS WHY.
///
/// THIS PASS WAS THE ERROR IT WAS TRYING TO FIX. It moved Chrom's jab and Palutena's n-air
/// hitboxes off `Sword1`/`Stick` onto `HaveR`, on the header's premise that a grafted bone is not
/// in the runtime transform chain. Measured with the project's own tools:
///
///     hbtrace, hitboxes on `stick`, Pikachu's skeleton + Palutena's animation:
///         96 active hitbox-frames, deviation 0.00 on every one
///     check-hitboxes.sh, the SHIPPED script after this pass rebased them to `haver`:
///         nair_palutena is 18.53 world units off
///
/// The rebase introduced all 18.53 units. With this table empty the same check reads 0.00, and no
/// other move moves: 38 moves, 460 hitbox-frames, all within 0.05 of the source.
///
/// WHY THE PREMISE WAS WRONG. `Stick`'s parent is `HaveR` and the dropped-in animation drives
/// `HaveR`, `HandR`, `HandL` and `Stick` alike, so the whole chain is animation-driven and lands
/// exactly where the source puts it -- which is what `tools/add-bones.sh:15` said all along:
/// "Both are animated in their source animation, so once the bone exists the dropped-in animation
/// drives it and the hitbox is exact." Two tools in this repo contradicted each other and the
/// wrong one won.
///
/// AND THE PLAYTEST SETTLES IT INDEPENDENTLY. The grafted staff mesh is RIGID on `Stick`, and it
/// visibly sweeps during n-air. A bone outside the runtime transform chain cannot move a mesh
/// bound to it. Whatever `telemetry::mirror_probe` was reading -- quoted in the header above, and
/// the whole basis for this pass -- it was not the transform the renderer uses.
///
/// KEPT RATHER THAN DELETED because the machinery is correct for a bone the animation genuinely
/// does NOT drive: it reads the transform per frame out of the shipped `.nuanmb` and falls back to
/// the skeleton's bind. If such a case appears, one entry here handles it. What must not happen
/// again is rebasing a bone the animation already drives.
// CHROM'S JAB IS BACK ON THE LIST, AND THE PLAYTEST IS WHY.
//
// This was emptied on the strength of a measurement: hitboxes on `stick` measured 0.00 against
// Palutena's own skeleton while the rebased version measured 18.53, so rebasing looked like the
// error. That measurement was right and the conclusion drawn from it was too narrow -- it compared
// the FILE against the source, and the file was never the problem.
//
// The playtest separates them cleanly, and it is the most useful report this project has had:
//
//     "The hitbox on cloud back air is fine both directions (unlike the other swords)."
//
//     cloud bair     hitboxes on `haver` and `top`   -- both VANILLA   -> fine both directions
//     corrin uair    hitboxes on `haver` and `top`   -- both VANILLA   -> never reported
//     mythra dash    hitboxes on `top`               -- VANILLA        -> hitbox not reported
//     chrom jab      hitboxes on `sword1`            -- GRAFTED        -> broken
//     palutena nair  hitboxes on `stick`             -- GRAFTED        -> broken, now on `havel`
//
// A hitbox on a bone Pikachu owns behaves; a hitbox on an appended bone does not. That is the same
// conclusion `telemetry::mirror_probe` reaches from the other side, with `handr` reading identically
// in both facings while the grafted bones differ in Y.
//
// SO THE 18.53 WAS THE WRONG TRADE FOR PALUTENA AND THE RIGHT ONE FOR CHROM. Her n-air keeps its
// hitboxes live for nineteen frames while the staff sweeps, so no single offset can follow it --
// which is why she gets the `HaveL` realisation instead, an exact per-frame answer. Chrom's jab
// hitboxes live three frames and `Sword1` does not move relative to the hand across them, so a
// static offset IS exact for him: this pass measured 0.00 on his jab both times it ran.
//
// `tools/check-hitboxes.sh` is the guard. If this rebase ever stops being exact for chrom it fails
// at the normal 0.05 tolerance, because the 18.6 exemption that used to hide exactly this is gone.
const REBASE: &[(&str, &str)] = &[("Sword1", "HaveR")];

#[derive(Clone, Copy)]
struct M34 {
    r: [[f32; 3]; 3],
    t: [f32; 3],
}

impl M34 {
    fn apply(&self, v: [f32; 3]) -> [f32; 3] {
        let mut o = self.t;
        for i in 0..3 {
            for j in 0..3 {
                o[i] += self.r[i][j] * v[j];
            }
        }
        o
    }
}

/// A quaternion and translation from an animation keyframe, as the matrix that takes a vector in
/// the bone's frame into its parent's.
fn from_qt(q: [f32; 4], t: [f32; 3], s: [f32; 3]) -> M34 {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    // Column k of the rotation is the image of the k-th basis vector, each pre-scaled by the
    // bone's own scale on that axis.
    let r = [
        [
            (1.0 - 2.0 * (y * y + z * z)) * s[0],
            (2.0 * (x * y - w * z)) * s[1],
            (2.0 * (x * z + w * y)) * s[2],
        ],
        [
            (2.0 * (x * y + w * z)) * s[0],
            (1.0 - 2.0 * (x * x + z * z)) * s[1],
            (2.0 * (y * z - w * x)) * s[2],
        ],
        [
            (2.0 * (x * z - w * y)) * s[0],
            (2.0 * (y * z + w * x)) * s[1],
            (1.0 - 2.0 * (x * x + y * y)) * s[2],
        ],
    ];
    M34 { r, t }
}

/// The grafted bone's transform for every frame of one animation, or empty if it is not animated.
fn anim_track(path: &str, bone: &str) -> Vec<M34> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let Ok(j) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };
    let mut out = Vec::new();
    // ssbh_data's AnimData is groups -> nodes -> tracks; only the Transform track matters.
    for g in j["groups"].as_array().into_iter().flatten() {
        for n in g["nodes"].as_array().into_iter().flatten() {
            if n["name"].as_str() != Some(bone) {
                continue;
            }
            for tr in n["tracks"].as_array().into_iter().flatten() {
                let Some(vals) = tr["values"]["Transform"].as_array() else { continue };
                for v in vals {
                    let f = |o: &serde_json::Value, k: &str| o[k].as_f64().unwrap_or(0.0) as f32;
                    let q = [
                        f(&v["rotation"], "x"),
                        f(&v["rotation"], "y"),
                        f(&v["rotation"], "z"),
                        f(&v["rotation"], "w"),
                    ];
                    let t = [
                        f(&v["translation"], "x"),
                        f(&v["translation"], "y"),
                        f(&v["translation"], "z"),
                    ];
                    let mut s = [
                        f(&v["scale"], "x"),
                        f(&v["scale"], "y"),
                        f(&v["scale"], "z"),
                    ];
                    for c in s.iter_mut() {
                        if *c == 0.0 {
                            *c = 1.0;
                        }
                    }
                    out.push(from_qt(q, t, s));
                }
            }
        }
    }
    out
}

/// The bind transform from the shipped skeleton, used when the animation does not animate the bone.
///
/// ssbh_data stores a bone's `transform` relative to its parent with the LOCAL AXES AS ROWS, so the
/// matrix is the transpose. Not assumed: `Sword1`'s rows are [-0.184844,-0.494104,-0.849526],
/// [-0.982662,0.105593,0.152397], [0.0144043,0.862967,-0.505056], and rotating (0,0,1) by its
/// animated quaternion (-0.551049, 0.66998, 0.378878, 0.322372) gives
/// (0.014398, 0.862960, -0.505056) -- row 2. Rows are axes.
fn bind(path: &str) -> HashMap<String, M34> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {path}: {e} -- run tools/add-bones.sh"));
    let j: serde_json::Value = serde_json::from_str(&text).expect("skeleton json");
    let mut out = HashMap::new();
    for b in j["bones"].as_array().expect("bones") {
        let m = b["transform"].as_array().expect("transform");
        let g = |row: usize, col: usize| -> f32 {
            m[row].as_array().expect("row")[col].as_f64().expect("f") as f32
        };
        out.insert(
            b["name"].as_str().unwrap_or("").to_string(),
            M34 {
                r: [
                    [g(0, 0), g(1, 0), g(2, 0)],
                    [g(0, 1), g(1, 1), g(2, 1)],
                    [g(0, 2), g(1, 2), g(2, 2)],
                ],
                t: [g(3, 0), g(3, 1), g(3, 2)],
            },
        );
    }
    out
}

/// Rewrites one `ATTACK`-shaped call: the bone hash, and the three offsets after the size.
///
/// The offsets sit at a fixed position -- `ATTACK(agent, id, part, BONE, damage, angle, kbg, fkb,
/// bkb, size, X, Y, Z, ...)` -- so this counts commas from the bone rather than pattern-matching
/// numbers, which keeps it safe against a macro of a different arity.
fn rebase_line(line: &str, from: &str, to: &str, m: &M34) -> Option<String> {
    let needle = format!("Hash40::new(\"{}\")", from.to_lowercase());
    let at = line.find(&needle)?;
    let rest = &line[at + needle.len()..];
    let mut idx = Vec::new();
    let mut start = None;
    let mut commas = 0usize;
    for (i, c) in rest.char_indices() {
        if c == ',' {
            if let Some(s) = start.take() {
                idx.push((s, i));
            }
            commas += 1;
            if commas > 9 {
                break;
            }
            continue;
        }
        if (7..=9).contains(&commas) && start.is_none() && !c.is_whitespace() {
            start = Some(i);
        }
    }
    if idx.len() < 3 {
        return None;
    }
    let mut v = [0f32; 3];
    for (k, (s, e)) in idx.iter().take(3).enumerate() {
        v[k] = rest[*s..*e].trim().parse().ok()?;
    }
    let w = m.apply(v);
    let mut out = String::with_capacity(line.len() + 16);
    out.push_str(&line[..at]);
    out.push_str(&format!("Hash40::new(\"{}\")", to.to_lowercase()));
    let mut cursor = 0usize;
    for (k, (s, e)) in idx.iter().take(3).enumerate() {
        out.push_str(&rest[cursor..*s]);
        // `{}` PRINTS 0.0f32 AS `0`, WHICH THE MACRO READS AS AN INTEGER AND REFUSES.
        // `{:?}` is f32's Debug, which always emits a decimal point: "0.0", "2.9999998".
        out.push_str(&format!("{:?}", w[k]));
        cursor = *e;
    }
    out.push_str(&rest[cursor..]);
    Some(out)
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: rebase <plugin-src-dir> <skeleton-json> <anim-json-dir>");
    let skel = std::env::args().nth(2).unwrap_or_else(|| "work/bones/out.json".into());
    let anims = std::env::args().nth(3).unwrap_or_else(|| "work/rebase".into());
    let binds = bind(&skel);
    // One track cache per (bone, animation stem), because a file is scanned once and a move's
    // hitboxes are all in the same file.
    let mut tracks: HashMap<(String, String), Vec<M34>> = HashMap::new();
    let mut files = Vec::new();
    collect(std::path::Path::new(&dir), &mut files);
    let mut touched = 0usize;
    let mut animated = 0usize;
    let mut rebased: Vec<String> = Vec::new();

    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let mut changed = false;
        let mut out = String::with_capacity(text.len());
        // The animation stem for the function currently being read, and the frame last named.
        let mut stem = String::new();
        let mut frame = 0usize;
        // THE MIDPOINT OF EACH HITBOX'S LIVE WINDOW, NOT ITS CREATION FRAME.
        //
        // Palutena's n-air creates its hitboxes on frame 5 and does not clear or re-create them
        // until frame 24 -- nineteen frames during which her animation SWEEPS the staff, 460
        // keyframes of changing rotation and translation relative to the hand. So no static offset
        // on any bone Pikachu has can be exact, and the only question is where to put the error.
        //
        // Anchoring at the creation frame makes the first active frame exact and the last one
        // maximally wrong: the harness measured 18.53 world units at frame 23. The midpoint halves
        // the worst case at the cost of the first frame, which is the better trade for a sustained
        // multi-hit whose whole window is live. Chrom's jab is unaffected -- its hitboxes live
        // three frames and its bone does not move relative to the hand at all, so it measures 0.00
        // either way.
        //
        // This is a CHOICE and not a transcription, which is why it is written down here: the
        // transcription is impossible for this move, and `tools/check-hitboxes.sh` carries the
        // resulting number so it cannot grow unnoticed.
        let lines: Vec<&str> = text.lines().collect();
        let marks: Vec<(usize, usize)> = lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let p = l.find("frame(agent.lua_state_agent, ")?;
                let n = l[p + 29..].split(&[')', '.'][..]).next()?.trim().parse().ok()?;
                Some((i, n))
            })
            .collect();
        for (line_no, line) in lines.iter().enumerate() {
            let line = *line;
            // `// ---- jab (Chrom) : game_attack11 ----` resets the frame; the stem comes from the
            // generated fn name, which gen-moves.sh derives from the same column of moves.tsv as
            // the animation stem.
            if let Some(p) = line.find("unsafe extern \"C\" fn pikachu_game_") {
                let name = line[p + 34..].split('(').next().unwrap_or("").to_string();
                stem = name;
                frame = 0;
            }
            if let Some(p) = line.find("frame(agent.lua_state_agent, ") {
                if let Some(n) = line[p + 29..].split(&[')', '.'][..]).next() {
                    frame = n.trim().parse().unwrap_or(frame);
                }
            }
            let mut l = line.to_string();
            for (from, to) in REBASE {
                if !l.contains(&format!("Hash40::new(\"{}\")", from.to_lowercase())) {
                    continue;
                }
                let key = (from.to_string(), stem.clone());
                let track = tracks.entry(key).or_insert_with(|| {
                    // The generated fn is `game_<acmd fn>`; moves.tsv's fourth column is the
                    // animation stem for that same row, and gen-rebase-anims.sh exports one JSON
                    // per stem into `anims`.
                    let p = format!("{anims}/{stem}.json");
                    anim_track(&p, from)
                });
                let m = if track.len() > 1 {
                    animated += 1;
                    // THE CREATION FRAME, and the midpoint of the hitbox's live window was tried
                    // and is worse. The reasoning for the midpoint was sound -- halve the
                    // worst-case error over a window where no offset can be exact -- and the
                    // harness disagreed: 19.22 world units against 18.53, and a worse mean too.
                    // Palutena's staff does not sweep monotonically, so the midpoint is not the
                    // minimiser of anything. The creation frame is the simpler rule, makes the
                    // FIRST active frame exact, and measures better; `marks` is kept because it is
                    // what proved the choice rather than assumed it.
                    let _ = &marks;
                    let _ = line_no;
                    track[frame.min(track.len() - 1)]
                } else if let Some(single) = track.first() {
                    *single
                } else {
                    *binds
                        .get(*from)
                        .unwrap_or_else(|| panic!("{from} not in {skel} -- run tools/add-bones.sh"))
                };
                if let Some(n) = rebase_line(&l, from, to, &m) {
                    l = n;
                    changed = true;
                    touched += 1;
                    if !rebased.contains(&stem) {
                        rebased.push(stem.clone());
                    }
                }
            }
            out.push_str(&l);
            out.push('\n');
        }
        if changed {
            std::fs::write(&f, out).expect("write");
        }
    }
    // RECORDED, SO THE CHECKS' EXEMPTION CANNOT GO STALE. `check-acmd-values.sh` compares the bone
    // anchor of every ported macro call against the source, which is correct for the other fifty
    // scripts and wrong for the ones this pass just rewrote. Writing the list here means the
    // exemption is derived from what actually happened rather than declared beside it -- the
    // failure mode `tools/check-lists.sh` exists to catch in three other places.
    let _ = std::fs::create_dir_all(std::path::Path::new(&anims));
    let _ = std::fs::write(
        format!("{anims}/rebased.txt"),
        rebased.iter().map(|s| format!("{s}\n")).collect::<String>(),
    );
    println!(
        "  rebase: {touched} hitbox(es) moved off a grafted bone onto a bone Pikachu has \
         ({animated} using the animation's own per-frame transform), {} script(s) recorded",
        rebased.len()
    );
}

fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().map_or(false, |x| x == "rs") {
            out.push(p);
        }
    }
}
