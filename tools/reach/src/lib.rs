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

use serde_json::Value;

pub type M4 = [[f32; 4]; 4];

pub const ID: M4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub fn mul(a: M4, b: M4) -> M4 {
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

pub fn point_mul(p: [f32; 3], m: M4) -> [f32; 3] {
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
pub fn quat_to_m4(x: f32, y: f32, z: f32, w: f32) -> M4 {
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y), 0.0],
        [2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x), 0.0],
        [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn srt_to_m4(s: [f32; 3], q: [f32; 4], t: [f32; 3]) -> M4 {
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

pub fn m4_from_rows(v: &Value) -> M4 {
    let mut m = ID;
    for i in 0..4 {
        for j in 0..4 {
            m[i][j] = v[i][j].as_f64().unwrap_or(if i == j { 1.0 } else { 0.0 }) as f32;
        }
    }
    m
}

pub fn f(v: &Value, k: &str) -> f32 {
    v[k].as_f64().unwrap_or(0.0) as f32
}



/// Loads a skeleton JSON into (lowercased names, parent indices, bind matrices).
pub fn load_skel(path: &str) -> (Vec<String>, Vec<i64>, Vec<M4>) {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let bones = v["bones"].as_array().unwrap();
    (
        bones.iter().map(|b| b["name"].as_str().unwrap().to_lowercase()).collect(),
        bones.iter().map(|b| b["parent_index"].as_i64().unwrap_or(-1)).collect(),
        bones.iter().map(|b| m4_from_rows(&b["transform"])).collect(),
    )
}

/// Loads an animation JSON into bone name -> per-frame local matrices.
pub fn load_anim(path: &str) -> std::collections::HashMap<String, Vec<M4>> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut out: std::collections::HashMap<String, Vec<M4>> = Default::default();
    if let Some(groups) = v["groups"].as_array() {
        for g in groups {
            if g["group_type"] != "Transform" { continue; }
            for node in g["nodes"].as_array().unwrap() {
                let n = node["name"].as_str().unwrap().to_lowercase();
                let arr = node["tracks"][0]["values"]["Transform"].as_array().unwrap();
                let frames = arr.iter().map(|t| srt_to_m4(
                    [f(&t["scale"],"x"), f(&t["scale"],"y"), f(&t["scale"],"z")],
                    [f(&t["rotation"],"x"), f(&t["rotation"],"y"), f(&t["rotation"],"z"), f(&t["rotation"],"w")],
                    [f(&t["translation"],"x"), f(&t["translation"],"y"), f(&t["translation"],"z")],
                )).collect();
                out.insert(n, frames);
            }
        }
    }
    out
}

/// World transform of `bone` at `frame`, relative to the fighter's origin
/// (root motion in Trans/Rot deliberately excluded -- see the note in main.rs).
pub fn bone_world(
    names: &[String], parents: &[i64], binds: &[M4],
    anim: &std::collections::HashMap<String, Vec<M4>>,
    bone: &str, frame: usize,
) -> Option<M4> {
    let bi = names.iter().position(|n| n == bone)?;
    let mut m = ID;
    let mut cur = bi as i64;
    while cur >= 0 {
        let i = cur as usize;
        // Take every bone from the animation when it has a track, including `Rot`: its
        // translation is the fighter's standing height and the animation carries the SOURCE's
        // value (Corrin 11.0, Mythra 9.42, Little Mac 8.33 -- each equal to that fighter's own
        // bind). A dropped-in animation therefore lifts the host to the source's height, so
        // substituting the host's bind here understated the drop-in's accuracy by ~6 units.
        //
        // `Trans` is different: its translation is root motion, which the game consumes to move
        // the fighter (Little Mac's f-smash runs 0 -> 17.77 there). A position expressed
        // relative to `top` is already relative to the moved fighter, so counting it again
        // inflated reach by the whole length of the lunge. Its translation alone is dropped.
        let mut local = anim
            .get(&names[i])
            .map(|fr| fr[frame.min(fr.len().saturating_sub(1))])
            .unwrap_or(binds[i]);
        if names[i] == "trans" {
            local[3][0] = 0.0;
            local[3][1] = 0.0;
            local[3][2] = 0.0;
        }
        m = mul(m, local);
        cur = parents[i];
    }
    Some(m)
}

pub fn transform_point(p: [f32; 3], m: M4) -> [f32; 3] { point_mul(p, m) }
