// Computes where a hitbox actually ends up in world space during the SOURCE fighter's
// animation, so a limb-anchored hitbox can be re-expressed relative to `top`.
//
// Why this exists: 39 of 75 ported hitboxes anchor to a limb bone (`armr`, `arml`, `kneer`, …)
// or to the item-hold bone. Those offsets are bone-local, and on the source fighter the bone is
// carried into position by the source's own animation. Pikachu plays HIS animation, so the bone
// stays near his body and the hitbox is tiny or buried -- observed in game as Little Mac's
// f-smash having almost no range and Falco's u-tilt second hit never connecting.
//
// Convention: ssbh stores 4x4 matrices row-major with translation in the last row, i.e. row
// vectors, p' = p * M. A bone's world matrix is therefore local * parent * ... * root.
//
// usage: reach <skel.json> <anim.json> <bone> <frame> [ox oy oz]
use serde_json::Value;

type M4 = [[f32; 4]; 4];

const ID: M4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn mul(a: M4, b: M4) -> M4 {
    let mut o = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                o[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    o
}

fn point_mul(p: [f32; 3], m: M4) -> [f32; 3] {
    let v = [p[0], p[1], p[2], 1.0];
    let mut o = [0.0f32; 4];
    for j in 0..4 {
        for k in 0..4 {
            o[j] += v[k] * m[k][j];
        }
    }
    [o[0], o[1], o[2]]
}

// Row-vector rotation matrix from a quaternion (transpose of the column-vector form).
fn quat_to_m4(x: f32, y: f32, z: f32, w: f32) -> M4 {
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y), 0.0],
        [2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x), 0.0],
        [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn srt_to_m4(s: [f32; 3], q: [f32; 4], t: [f32; 3]) -> M4 {
    let scale: M4 = [
        [s[0], 0.0, 0.0, 0.0],
        [0.0, s[1], 0.0, 0.0],
        [0.0, 0.0, s[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let rot = quat_to_m4(q[0], q[1], q[2], q[3]);
    let mut m = mul(scale, rot);
    m[3][0] = t[0];
    m[3][1] = t[1];
    m[3][2] = t[2];
    m
}

fn m4_from_rows(v: &Value) -> M4 {
    let mut m = ID;
    for i in 0..4 {
        for j in 0..4 {
            m[i][j] = v[i][j].as_f64().unwrap_or(if i == j { 1.0 } else { 0.0 }) as f32;
        }
    }
    m
}

fn f(v: &Value, k: &str) -> f32 {
    v[k].as_f64().unwrap_or(0.0) as f32
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 4 {
        eprintln!("usage: reach <skel.json> <anim.json> <bone> <frame> [ox oy oz]");
        std::process::exit(2);
    }
    let skel: Value = serde_json::from_str(&std::fs::read_to_string(&a[0]).unwrap()).unwrap();
    let anim: Value = serde_json::from_str(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let want = a[2].to_lowercase();
    let frame: usize = a[3].parse().unwrap();
    let off = if a.len() >= 7 {
        [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()]
    } else {
        [0.0, 0.0, 0.0]
    };

    // skeleton: name, parent, bind transform
    let bones = skel["bones"].as_array().unwrap();
    let names: Vec<String> = bones.iter().map(|b| b["name"].as_str().unwrap().to_lowercase()).collect();
    let parents: Vec<i64> = bones.iter().map(|b| b["parent_index"].as_i64().unwrap_or(-1)).collect();
    let binds: Vec<M4> = bones.iter().map(|b| m4_from_rows(&b["transform"])).collect();

    // animation: bone name -> local matrix at this frame
    let mut animated: std::collections::HashMap<String, M4> = Default::default();
    if let Some(groups) = anim["groups"].as_array() {
        for g in groups {
            if g["group_type"] != "Transform" {
                continue;
            }
            for node in g["nodes"].as_array().unwrap() {
                let n = node["name"].as_str().unwrap().to_lowercase();
                let vals = &node["tracks"][0]["values"]["Transform"];
                let arr = vals.as_array().unwrap();
                let idx = frame.min(arr.len().saturating_sub(1));
                let t = &arr[idx];
                let m = srt_to_m4(
                    [f(&t["scale"], "x"), f(&t["scale"], "y"), f(&t["scale"], "z")],
                    [f(&t["rotation"], "x"), f(&t["rotation"], "y"), f(&t["rotation"], "z"), f(&t["rotation"], "w")],
                    [f(&t["translation"], "x"), f(&t["translation"], "y"), f(&t["translation"], "z")],
                );
                animated.insert(n, m);
            }
        }
    }

    // `top` is not a skeleton bone -- it is the fighter's own origin, which is why no fighter's
    // nusktb contains it even though every ACMD script anchors to it. A `top`-relative position
    // therefore means "relative to the fighter", so the root motion in `Trans`/`Rot` must be
    // excluded: the fighter's position already moves with it, and double-counting it inflated
    // Little Mac's f-smash z by the whole length of his forward lunge.
    let root_motion: [&str; 2] = ["trans", "rot"];
    let world = |start: usize| -> M4 {
        let mut m = ID;
        let mut cur = start as i64;
        while cur >= 0 {
            let i = cur as usize;
            let local = if root_motion.contains(&names[i].as_str()) {
                binds[i]
            } else {
                animated.get(&names[i]).copied().unwrap_or(binds[i])
            };
            m = mul(m, local);
            cur = parents[i];
        }
        m
    };

    let Some(bi) = names.iter().position(|n| *n == want) else {
        eprintln!("bone '{want}' not in skeleton ({} bones)", names.len());
        std::process::exit(1);
    };

    let bw = world(bi);
    let origin = point_mul([0.0, 0.0, 0.0], bw);
    let tip = point_mul(off, bw);

    println!("frame {frame}  bone {want}  (animated: {})", animated.contains_key(&want));
    println!("  bone origin in world : x={:8.3} y={:8.3} z={:8.3}", origin[0], origin[1], origin[2]);
    println!("  hitbox point in world: x={:8.3} y={:8.3} z={:8.3}", tip[0], tip[1], tip[2]);
    println!();
    println!("  -> as a top-anchored offset, use: y={:.2} z={:.2} (x={:.2})", tip[1], tip[2], tip[0]);
}
