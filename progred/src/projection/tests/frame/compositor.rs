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

#[path = "../../../../tests/compositor_experiment/gpu.rs"]
#[allow(dead_code)]
mod gpu;

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
