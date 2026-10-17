// Every motion of every motion_list.bin under a directory, one JSON object per line.
//
//   mljson <dir>            e.g. mljson ../../extracted/fighter
//
// WHY THIS EXISTS. tools/move-index.py joins each fighter's motions to their ACMD, animations and
// statuses, and there are 5,660 motion lists under extracted/fighter. One process over the whole
// tree beats 5,660 runs of `mlshow`, and reading them through motion_list_rs keeps the format in
// the one parser this directory already trusts. Hashes are printed as integers and named by the
// caller from the project's dictionaries, so nothing here guesses a label.
//
// Line format:
//   {"file":"<path>","motion_path":H,"kind":H,"game_script":H,"flags":N,"blend_frames":N,
//    "animations":[[H,unk],...],"scripts":[H,...],"extra":{"xlu_start":N,...}|null}
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p.file_name().map_or(false, |n| n == "motion_list.bin") {
            out.push(p);
        }
    }
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn main() {
    let root = std::env::args().nth(1).expect("usage: mljson <dir>");
    let mut files = Vec::new();
    walk(Path::new(&root), &mut files);
    let mut bad = 0;
    for f in &files {
        let m = match motion_lib::open(f) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{}: {e:?}", f.display());
                bad += 1;
                continue;
            }
        };
        let file = esc(&f.to_string_lossy().replace('\\', "/"));
        for (kind, mo) in &m.list {
            let anims: Vec<String> = mo.animations.iter().map(|a| format!("[{},{}]", a.name.0, a.unk)).collect();
            let scripts: Vec<String> = mo.scripts.iter().map(|s| s.0.to_string()).collect();
            let extra = match &mo.extra {
                Some(x) => format!(
                    "{{\"xlu_start\":{},\"xlu_end\":{},\"cancel_frame\":{},\"no_stop_intp\":{}}}",
                    x.xlu_start, x.xlu_end, x.cancel_frame, x.no_stop_intp
                ),
                None => "null".into(),
            };
            let flags: u16 = mo.flags.into();
            println!(
                "{{\"file\":\"{file}\",\"motion_path\":{},\"kind\":{},\"game_script\":{},\"flags\":{flags},\"blend_frames\":{},\"animations\":[{}],\"scripts\":[{}],\"extra\":{extra}}}",
                m.motion_path.0, kind.0, mo.game_script.0, mo.blend_frames, anims.join(","), scripts.join(",")
            );
        }
    }
    eprintln!("{} motion lists, {} unreadable", files.len(), bad);
}
