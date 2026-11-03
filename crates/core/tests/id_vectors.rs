//! Runs the shared id vectors (test-vectors/ids.json). The TypeScript port runs the same file.

use op_core::IdGen;
use serde::Deserialize;

#[derive(Deserialize)]
struct Suite {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    ops: Vec<Op>,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Op {
    New { device: String },
    Restore { device: String, ids: Vec<String> },
    Next { at: i64, expect: String },
}

#[test]
fn shared_id_vectors() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/ids.json");
    let suite: Suite =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    assert!(!suite.cases.is_empty());

    for case in suite.cases {
        let mut id_gen: Option<IdGen> = None;
        for (i, op) in case.ops.iter().enumerate() {
            match op {
                Op::New { device } => id_gen = Some(IdGen::new(device)),
                Op::Restore { device, ids } => {
                    id_gen = Some(IdGen::restore(device, ids.iter().map(String::as_str)));
                }
                Op::Next { at, expect } => {
                    let id = id_gen.as_mut().expect("generator created").next(*at);
                    assert_eq!(&id, expect, "{} op {i}", case.name);
                }
            }
        }
    }
}
