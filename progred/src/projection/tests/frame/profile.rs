use super::*;

/// Profiling loop: record the IoP tree drawing each frame so a sampler
/// sees mostly interpreter time.
/// `./tools/sandbox-cargo test --release -p progred iop_tree_profile_loop -- --ignored`
#[test]
#[ignore]
fn iop_tree_profile_loop() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/iop-tree.gid"
    )))
    .expect("the IoP tree demo parses");
    let declaration = crate::workspace::declarations(doc.root.as_ref())
        .into_iter()
        .next()
        .expect("the picture is declared as a pane");
    let root = doc.root.as_ref().unwrap();
    let value = crate::spine::get(root, &declaration.path);
    let source = Some((declaration.path.as_slice(), value));
    let iterations: usize = std::env::var("IOP_PROFILE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(30);
    eprintln!(
        "runtime value: {} bytes",
        std::mem::size_of::<grap::RuntimeValue>()
    );
    let mut context = BenchContext::new();
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let (bench, _) = context.place(
            &doc,
            None,
            &Annotations::default(),
            1400.0,
            None,
            None,
            source,
        );
        std::hint::black_box(&bench.list);
    }
    eprintln!(
        "IoP profile: {iterations} frames, {:.1?} each",
        start.elapsed() / iterations as u32,
    );
}

/// Profile the document view's top viewport, clipping offscreen drawings.
/// This is the library/name/projection lookup canary.
#[test]
#[ignore]
fn iop_tree_source_profile_loop() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/iop-tree.gid"
    )))
    .expect("the IoP tree demo parses");
    let iterations = 30;
    let mut context = BenchContext::new();
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let (bench, _) = context.place(
            &doc,
            None,
            &Annotations::default(),
            1400.0,
            None,
            Some(Rect::new(0.0, 0.0, 1400.0, 900.0)),
            None,
        );
        std::hint::black_box(&bench.list);
    }
    eprintln!(
        "IoP source profile: {iterations} frames, {:.1?} each",
        start.elapsed() / iterations,
    );
}
