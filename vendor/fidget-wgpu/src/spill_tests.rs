use super::*;
use fidget_core::{context::Tree, vm::VmData};

fn scene() -> Tree {
    let sphere = |x, y, z| {
        (Tree::x() - x).square()
            + (Tree::y() - y).square()
            + (Tree::z() - z).square()
            - 0.2437
    };
    (sphere(-0.313, 0.117, 0.071).min(sphere(0.323, -0.119, 0.013)))
        .max(-sphere(0.011, 0.033, 0.417))
}

fn spilling(shape: &VmShape) -> RenderShape {
    let data = shape.inner().data();
    let tape = VmData::<3>::from_ssa(data.ssa().clone(), data.vars.clone());
    let bytecode = Bytecode::new(&tape).unwrap();
    assert!(bytecode.mem_count() > 0);
    RenderShape {
        shape: shape.clone(),
        bytecode,
        choice_words: 32,
    }
}

#[test]
fn spill_pipeline_storage_buckets() {
    assert_eq!(TapeStorage::new(3, 0).bucket(), TapeStorage::new(8, 1));
    assert_eq!(
        TapeStorage::new(255, 65).bucket(),
        TapeStorage::new(255, 65)
    );
    assert_eq!(
        TapeStorage::new(255, 12747).bucket(),
        TapeStorage::new(255, 12747)
    );
}

#[test]
fn spills_require_explicit_opt_in() {
    let terms: Vec<_> = (0..512).map(|i| (Tree::x() + i).sin()).collect();
    let sum = terms.iter().cloned().reduce(|a, b| a + b).unwrap();
    let squares = terms
        .iter()
        .map(|t| t.clone().square())
        .reduce(|a, b| a + b)
        .unwrap();
    let shape = VmShape::from(sum / squares);
    assert!(shape.inner().data().slot_count() > 255);
    if cfg!(feature = "experimental-spills") {
        assert!(RenderShape::new(&shape).is_ok());
    } else {
        assert!(matches!(
            RenderShape::new(&shape),
            Err(RenderShapeError::SpillsUnsupported(_))
        ));
    }
    let color = color::ShapeColor::Rgb {
        r: shape.clone(),
        g: shape.clone(),
        b: shape,
    };
    if cfg!(feature = "experimental-spills") {
        assert!(color::ShapeColorBuffers::new(&[color]).is_ok());
    } else {
        assert!(matches!(
            color::ShapeColorBuffers::new(&[color]),
            Err(color::ShapeColorError::SpillsUnsupported(_))
        ));
    }
}

#[test]
#[ignore = "requires a real GPU; compares spilling and ordinary tapes"]
fn gpu_spilling_voxels_match_cpu_and_nonspilling_gpu() {
    let gpu = pollster::block_on(Gpu::init_basic()).unwrap();
    let context = voxel::Context::new(&gpu);
    let mut workspace = context.workspace();
    let mut read = gpu.read_buffer("spill test");
    let shape = VmShape::from(scene());
    let spilled = spilling(&shape);
    let regular = RenderShape::new(&shape).unwrap();
    assert_eq!(regular.bytecode.mem_count(), 0);
    let size = fidget_core::render::VoxelSize::new(96, 80, 192);
    let settings = voxel::RenderConfig::from_size(size);
    let ordinary = context
        .run(&regular, &mut workspace, &mut read, settings.clone())
        .unwrap();
    let actual = context
        .run(&spilled, &mut workspace, &mut read, settings.clone())
        .unwrap();
    assert_eq!(actual.as_slice(), ordinary.as_slice());
    let cpu = settings.run(shape.clone().try_into().unwrap());
    assert_eq!(
        actual.iter().filter(|p| p.depth != 0).count(),
        cpu.iter().filter(|p| p.depth != 0).count()
    );
    for (a, b) in actual.iter().zip(cpu.iter()) {
        // CPU stores the occupied sample plus one; GPU stores the sample itself.
        assert_eq!(if a.depth == 0 { 0 } else { a.depth + 1 }, b.depth);
        if a.depth != 0 {
            for (a, b) in a.normal.iter().zip(&b.normal) {
                assert!((a - b).abs() < 0.00001, "{a} != {b}");
            }
        }
    }
}

#[test]
#[ignore = "requires a real GPU; checks variable prefix and scratch reuse"]
fn gpu_spilling_variables_and_workspace_reuse() {
    let gpu = pollster::block_on(Gpu::init_basic()).unwrap();
    let context = voxel::Context::new(&gpu);
    let mut workspace = context.workspace();
    let mut read = gpu.read_buffer("spill variables test");
    let variable = fidget_core::var::Var::new();
    let shape = VmShape::from(scene() + Tree::from(variable));
    let regular = RenderShape::new(&shape).unwrap();
    let settings = voxel::RenderConfig::from_size(
        fidget_core::render::VoxelSize::new(101, 83, 192),
    );
    let mut vars = ShapeVars::new();
    // Reuse the same workspace across changed variables, expanded choice
    // history, and transitions between spilling and nonspilling pipelines.
    for (value, words) in [(-0.07, 32), (0.03, 4096), (0.11, 32)] {
        vars.insert(variable.index().unwrap(), value);
        let ordinary = context
            .run_with_vars(
                &regular,
                &vars,
                &mut workspace,
                &mut read,
                settings.clone(),
            )
            .unwrap();
        let mut spilling = spilling(&shape);
        spilling.choice_words = words;
        let actual = context
            .run_with_vars(
                &spilling,
                &vars,
                &mut workspace,
                &mut read,
                settings.clone(),
            )
            .unwrap();
        assert_eq!(actual.as_slice(), ordinary.as_slice());
    }
}

#[cfg(feature = "experimental-spills")]
#[test]
#[ignore = "requires a real GPU; compares batched screen regions"]
fn gpu_screen_batches_preserve_geometry() {
    let gpu = pollster::block_on(Gpu::init_basic()).unwrap();
    let context = voxel::Context::new(&gpu);
    let mut workspace = context.workspace();
    let mut read = gpu.read_buffer("batch test");
    let shape = VmShape::from(scene());
    let settings = voxel::RenderConfig::from_size(
        fidget_core::render::VoxelSize::new(37, 29, 128),
    );
    let ordinary = context
        .run(
            &RenderShape::new(&shape).unwrap(),
            &mut workspace,
            &mut read,
            settings,
        )
        .unwrap();
    for tape in [RenderShape::new(&shape).unwrap(), spilling(&shape)] {
        for side in [8, 16, 64] {
            let actual = voxel::diagnostics::batched(
                &context,
                &tape,
                &mut workspace,
                &mut read,
                &settings,
                std::num::NonZeroU32::new(side).unwrap(),
            )
            .unwrap();
            for (a, b) in actual.iter().zip(ordinary.iter()) {
                assert_eq!(a.depth, b.depth);
                for (a, b) in a.normal.iter().zip(&b.normal) {
                    assert!((a - b).abs() < 0.00001, "{a} != {b}");
                }
            }
        }
    }
}
