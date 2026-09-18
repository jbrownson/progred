//! Headless, opt-in export and reference timings for tools/bend-fidget.
use super::*;
use fidget_engine::{
    eval::{Function, TracingEvaluator},
    shape::{EzShape, Shape, ShapeTape},
    types::Interval,
    var::Var,
    vm::VmTrace,
};
use std::{hint::black_box, path::Path};

fn query(i: usize) -> [Interval; 3] {
    let key = (i as u32).wrapping_mul(2_654_435_761) & 4095;
    std::array::from_fn(|axis| {
        let p = (((key >> (axis * 4)) & 15) as f32 - 7.5) / 12.0;
        let r = if i % 7 == 0 { 0.0 } else { 1.0 / 128.0 };
        Interval::new(p - r, p + r)
    })
}

fn run<F: Function<Trace = VmTrace>>(
    tape: &ShapeTape<<F::IntervalEval as TracingEvaluator>::Tape>,
    count: usize,
    threads: usize,
    choices: usize,
) -> Vec<[u32; 3]> {
    let both = (0..choices).fold(0u32, |h, _| h.wrapping_mul(33).wrapping_add(3));
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|thread| {
                s.spawn(move || {
                    let mut eval = Shape::<F>::new_interval_eval();
                    (count * thread / threads..count * (thread + 1) / threads)
                        .map(|i| {
                            let [x, y, z] = query(i);
                            let (v, trace) = eval.eval(tape, x, y, z).unwrap();
                            // Fidget returns no trace only when all choices were Both.
                            let hash = trace.map(|t| {
                                t.as_slice()
                                    .iter()
                                    .fold(0u32, |h, c| h.wrapping_mul(33).wrapping_add(*c as u32))
                            });
                            [
                                v.lower().to_bits(),
                                v.upper().to_bits(),
                                hash.unwrap_or(both),
                            ]
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    })
}

fn bench<F: Function<Trace = VmTrace>>(
    label: &str,
    shape: &Shape<F>,
    count: usize,
    choices: usize,
    reference: &[[u32; 3]],
) {
    let start = Instant::now();
    let tape = shape.ez_interval_tape();
    eprintln!(
        "bend-reference {label} tape_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    for threads in [1, 8] {
        for round in 0..4 {
            let start = Instant::now();
            let got = run::<F>(&tape, count, threads, choices);
            let elapsed = start.elapsed();
            assert_eq!(got, reference, "{label} results");
            black_box(got);
            eprintln!(
                "bend-reference {label} samples={count} threads={threads} round={round} ms={:.3}",
                elapsed.as_secs_f64() * 1000.0
            );
        }
    }
}

#[test]
#[ignore = "exports real CAM tapes and times interval evaluation; see tools/bend-fidget"]
fn bend_fidget_export() {
    let dir =
        std::path::PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap()).join("bend-fidget");
    std::fs::create_dir_all(&dir).unwrap();
    let (path, radius) = paths();
    for (name, progress, count) in [
        ("small", 0.002, 4096),
        ("medium", 0.02, 4096),
        ("large", 1.0, 1024),
    ] {
        let tree = stock(&path, radius, progress);
        let start = Instant::now();
        let vm = VmShape::from(tree.clone());
        eprintln!(
            "bend-reference {name} prepare_ms={:.3}",
            start.elapsed().as_secs_f64() * 1000.0
        );
        let data = vm.inner().data();
        let mut axes = vec![0; data.vars.len()];
        for (v, i) in data.vars.iter() {
            axes[i] = match v {
                Var::X => 0,
                Var::Y => 1,
                Var::Z => 2,
                _ => panic!("unexpected variable"),
            };
        }
        let reference = run::<fidget_engine::vm::VmFunction>(
            &vm.ez_interval_tape(),
            count,
            1,
            data.choice_count(),
        );
        let fixture = serde_json::json!({"ops":data.iter_asm().collect::<Vec<_>>(), "slots":data.slot_count(), "choices":data.choice_count(), "axes":axes, "samples":count, "expected":reference});
        write(&dir.join(format!("{name}.json")), &fixture);
        eprintln!(
            "bend-reference {name} ops={} slots={} choices={}",
            data.len(),
            data.slot_count(),
            data.choice_count()
        );
        bench(
            &format!("{name}/vm"),
            &vm,
            count,
            data.choice_count(),
            &reference,
        );
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        if std::env::var_os("BEND_FIDGET_JIT").is_some() {
            let jit = fidget_engine::jit::JitShape::from(tree);
            bench(
                &format!("{name}/jit"),
                &jit,
                count,
                data.choice_count(),
                &reference,
            );
        }
    }
}

fn write(path: &Path, value: &serde_json::Value) {
    serde_json::to_writer(std::fs::File::create(path).unwrap(), value).unwrap();
}
