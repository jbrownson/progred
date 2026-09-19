use super::*;
use puri_vello::{
    VelloCanvas,
    compositor::{Compositor, Resources, SplitCanvas},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Instant,
};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};

mod mesh;

#[path = "../../../../tests/compositor_experiment/gpu.rs"]
#[allow(dead_code)]
mod gpu;

#[test]
#[ignore = "headless CAM hover GPU regression"]
fn cam_hover_compositor_pixels() {
    let gpu = gpu::Gpu::new();
    // At this size, hundreds of pane-sized highlight clips left the controls'
    // vector pass unpainted. Small-window tests did not reproduce the failure.
    let size = kurbo::Size::new(3028.0, 1836.0);
    let scale = 2.0;
    let source = crate::command::Example::Toolpaths.source().replace(
        &crate::libraries::toolpath::vocabulary::PREVIEW_REFINED
            .simple()
            .to_string(),
        &crate::libraries::toolpath::vocabulary::PREVIEW_MESH
            .simple()
            .to_string(),
    );
    let (doc, names) = crate::gid_text::parse(&source).unwrap();
    let mut editor = crate::test_editor(doc);
    editor.model.workspace.left_width = 0.70;
    let jobs = Arc::new(Mutex::new(VecDeque::<incremental::background::Job>::new()));
    editor.computations = crate::computations::Computations::new(
        incremental::background::Executor::new({
            let jobs = jobs.clone();
            move |job| jobs.lock().unwrap().push_back(job)
        }),
        || {},
    );
    let mut runner = crate::EditorRunner::new(editor);
    runner.refresh_frame(scale, size);
    loop {
        let job = jobs.lock().unwrap().pop_front();
        let Some(job) = job else { break };
        std::thread::spawn(job).join().unwrap();
        assert!(runner.editor.computations.tasks.poll());
        runner.refresh_frame(scale, size);
    }
    let pane = runner
        .frame
        .dispatch
        .view_regions
        .iter()
        .find(|region| matches!(region.root.target(), crate::workspace::Target::Pane { .. }))
        .unwrap()
        .rect;
    let point = (1..(160.0 * scale) as usize)
        .flat_map(|dy| {
            (30..pane.width() as usize - 30)
                .step_by(8)
                .map(move |dx| (dx, dy))
        })
        .find_map(|(dx, dy)| {
            let point = Point::new(pane.x0 + dx as f64, pane.y1 - dy as f64);
            match runner
                .frame
                .dispatch
                .hover_geometry
                .probe(Some(point), None, 0.0)
            {
                Some((
                    _,
                    Claim::Direct(Hovered::Tree(Hover::Source(SourceTrace::InCell {
                        cell,
                        path,
                        ..
                    }))),
                )) if cell == names["evenly_spaced"] => {
                    let value = path.iter().try_fold(
                        runner.editor.model.doc.cells.value(cell)?,
                        |value, step| match step {
                            Step::Key(key) => value.as_record()?.get(key),
                            Step::Element(position) => value.as_list()?.get(position),
                            _ => None,
                        },
                    )?;
                    (value
                        .as_record()?
                        .get(&grap::vocabulary::FUNCTION)?
                        .as_cell()?
                        == crate::libraries::tree::vocabulary::LEAF)
                        .then_some(point)
                }
                _ => None,
            }
        })
        .expect("a generated chamfer leaf has a source-linked notch");
    let target = gpu.texture(size.width as u32, size.height as u32);
    let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
    let mut resources = Resources::default();
    let mut baseline: Option<Vec<u8>> = None;
    let mut baseline_meshes = None;
    fn meshes(commands: &[DrawCmd]) -> Vec<(puri::mesh::Scene, kurbo::Affine)> {
        commands
            .iter()
            .flat_map(|command| match command {
                DrawCmd::Mesh { scene, transform } => vec![(scene.clone(), *transform)],
                DrawCmd::Clip { children, .. } => meshes(children),
                _ => Vec::new(),
            })
            .collect()
    }
    for hovered in [false, true, false] {
        runner.editor.pointer = hovered.then_some(point);
        runner.editor.modifiers = if hovered {
            ui_events::keyboard::Modifiers::META | ui_events::keyboard::Modifiers::CONTROL
        } else {
            Default::default()
        };
        runner.refresh_frame(scale, size);
        let mut list = DrawList::new();
        puri::frame::render(runner.prepare_paint(scale, size).renders, &mut list);
        runner.frame_presented();
        let current_meshes = meshes(&list.0);
        if let Some(baseline) = &baseline_meshes {
            let baseline: &Vec<(puri::mesh::Scene, kurbo::Affine)> = baseline;
            assert_eq!(current_meshes.len(), baseline.len());
            for ((scene, transform), (prior, prior_transform)) in
                current_meshes.iter().zip(baseline)
            {
                assert!(Arc::ptr_eq(&scene.geometry, &prior.geometry));
                assert_eq!(scene.view.model_to_view, prior.view.model_to_view);
                assert_eq!(scene.view.projection, prior.view.projection);
                assert_eq!(
                    (scene.view.width, scene.view.height),
                    (prior.view.width, prior.view.height)
                );
                assert_eq!(transform, prior_transform);
                assert!(scene.surface.is_none() && prior.surface.is_none());
            }
        } else {
            assert!(!current_meshes.is_empty());
            baseline_meshes = Some(current_meshes);
        }
        let mut canvas = SplitCanvas::default();
        puri::draw::replay(&list, &mut canvas);
        let output = compositor
            .render(
                &gpu.device,
                &gpu.queue,
                &canvas.finish(),
                &mut resources,
                &target,
                Color::WHITE,
            )
            .unwrap();
        let pixels = gpu.read(&output.texture);
        if let Some(baseline) = &baseline {
            let changed = pixels
                .chunks_exact(4)
                .zip(baseline.chunks_exact(4))
                .enumerate()
                .filter(|(i, (a, b))| {
                    i % (size.width as usize) < pane.x1 as usize
                        && a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 2)
                })
                .count();
            if hovered {
                assert!(changed > 0, "the source-linked notches should highlight");
                assert!(
                    changed < (pane.area() / 20.0) as usize,
                    "hover should tint only notches, not erase controls or fill the pane: {changed} changed pixels"
                );
            } else {
                // Test the same bottom control band searched above. Large mesh
                // shading can vary by a few pixels even for identical inputs
                // on Metal (also on the old separate-device path); the exact
                // mesh-input checks above isolate that from hover behavior.
                let controls_changed = pixels
                    .chunks_exact(4)
                    .zip(baseline.chunks_exact(4))
                    .enumerate()
                    .filter(|(i, (a, b))| {
                        let point = Point::new(
                            (i % size.width as usize) as f64,
                            (i / size.width as usize) as f64,
                        );
                        pane.contains(point) && point.y >= pane.y1 - 160.0 * scale && a != b
                    })
                    .count();
                assert_eq!(
                    controls_changed, 0,
                    "leaving the source restores the controls"
                );
            }
        } else {
            baseline = Some(pixels);
        }
    }
}

#[test]
#[ignore = "headless full-editor GPU compositor comparison"]
fn editor_compositor_profile() {
    let gpu = gpu::Gpu::new();
    let size = kurbo::Size::new(2400.0, 1600.0);
    let base_color = Color::new([0.965, 0.965, 0.972, 1.0]);
    for example in [
        crate::command::Example::IopTree,
        crate::command::Example::Toolpaths,
    ] {
        let source = example.source().replace(
            &crate::libraries::toolpath::vocabulary::PREVIEW_REFINED
                .simple()
                .to_string(),
            &crate::libraries::toolpath::vocabulary::PREVIEW_MESH
                .simple()
                .to_string(),
        );
        let (doc, _) = crate::gid_text::parse(&source).unwrap();
        let mut editor = crate::test_editor(doc);
        editor.model.workspace.left_width = 0.5;
        let jobs = Arc::new(Mutex::new(VecDeque::<incremental::background::Job>::new()));
        editor.computations = crate::computations::Computations::new(
            incremental::background::Executor::new({
                let jobs = jobs.clone();
                move |job| jobs.lock().unwrap().push_back(job)
            }),
            || {},
        );
        let mut runner = crate::EditorRunner::new(editor);
        runner.refresh_frame(2.0, size);
        loop {
            let job = jobs.lock().unwrap().pop_front();
            let Some(job) = job else { break };
            std::thread::spawn(job).join().unwrap();
            assert!(runner.editor.computations.tasks.poll());
            runner.refresh_frame(2.0, size);
        }
        let mut list = DrawList::new();
        puri::frame::render(runner.prepare_paint(2.0, size).renders, &mut list);
        // This older benchmark isolates image/vector composition, not mesh
        // drawing. Freeze identical GPU-rasterized images for both routes.
        let list = mesh::images(
            &gpu,
            &mut puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue),
            &mut mesh::Readback::default(),
            &list,
        );
        let reference = gpu.texture(size.width as u32, size.height as u32);
        let scratch = gpu.texture(size.width as u32, size.height as u32);
        let mut renderer = Renderer::new(&gpu.device, RendererOptions::default()).unwrap();
        let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
        let mut resources = Resources::default();
        let mut direct = Vec::new();
        let mut composed = Vec::new();
        let mut scene = Scene::new();
        for frame in 0..16 {
            let mut times = [Default::default(); 2];
            let mut output = None;
            for index in if frame % 2 == 0 { [0, 1] } else { [1, 0] } {
                gpu.wait();
                let start = Instant::now();
                if index == 0 {
                    scene.reset();
                    puri::draw::replay(&list, &mut VelloCanvas(&mut scene));
                    renderer
                        .render_to_texture(
                            &gpu.device,
                            &gpu.queue,
                            &scene,
                            &reference.create_view(&Default::default()),
                            &RenderParams {
                                base_color,
                                width: size.width as u32,
                                height: size.height as u32,
                                antialiasing_method: AaConfig::Msaa16,
                            },
                        )
                        .unwrap();
                } else {
                    let mut canvas = SplitCanvas::default();
                    puri::draw::replay(&list, &mut canvas);
                    output = Some(
                        compositor
                            .render(
                                &gpu.device,
                                &gpu.queue,
                                &canvas.finish(),
                                &mut resources,
                                &scratch,
                                base_color,
                            )
                            .unwrap(),
                    );
                }
                gpu.wait();
                times[index] = start.elapsed();
            }
            if frame >= 4 {
                direct.push(times[0]);
                composed.push(times[1]);
            }
            if frame == 15 {
                let a = gpu.read(&reference);
                let b = gpu.read(&output.unwrap().texture);
                let mut errors: Vec<_> = a.iter().zip(&b).map(|(a, b)| a.abs_diff(*b)).collect();
                errors.sort_unstable();
                eprintln!(
                    "pixel errors: max={} p99={}",
                    errors.last().unwrap(),
                    errors[errors.len() * 99 / 100]
                );
                assert!(errors[errors.len() * 99 / 100] <= 2);
                assert!(*errors.last().unwrap() <= 5);
            }
        }
        direct.sort();
        composed.sort();
        eprintln!(
            "editor {:?}: direct={:?} composed={:?} {:?}; {} images, {} scratch textures",
            example,
            direct[direct.len() / 2],
            composed[composed.len() / 2],
            compositor.counts,
            resources.uploaded_images(),
            resources.scratch_textures()
        );
        assert_eq!(compositor.counts.uploads, 0);
    }
}
