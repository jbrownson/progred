//! Compare the direct mesh composite with one through an intermediate image.
use super::*;

#[test]
#[ignore = "paired full-editor CAM orbit draw/composition timing; no app launch"]
fn cam_mesh_roundtrip_profile() {
    use crate::libraries::{
        controls::vocabulary::STATE, fidget::vocabulary as f, toolpath::vocabulary as t,
    };
    use crate::test_examples::{cam_editor, cam_position, result_path};
    let gpu = gpu::Gpu::new();
    let size = kurbo::Size::new(2400.0, 1800.0);
    let scale = 2.0;
    for progress in [0.02, 0.5] {
        let mut editor = cam_editor(t::PREVIEW_MESH);
        let path = crate::workspace::panes(editor.model.doc.root.as_ref()).remove(0);
        let controls = result_path(&path);
        let camera = [controls.clone(), result_path(&[])].concat();
        editor
            .model
            .workspace
            .view_mut(&crate::display::widget::view::Root::pane(path.clone()))
            .unwrap()
            .annotations
            .set_field(&controls, STATE, Some(cam_position(progress)));
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
            assert!(runner.editor.computations.poll());
            runner.refresh_frame(scale, size);
        }
        let target = gpu.texture(size.width as u32, size.height as u32);
        let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
        let mut resources = Resources::default();
        let mut renderer = puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue);
        let mut readback = gpu::Readback::default();
        let mut timings = [Vec::new(), Vec::new()];
        let mut builds = Vec::new();
        for iteration in 0..24 {
            runner
                .editor
                .model
                .workspace
                .view_mut(&crate::display::widget::view::Root::pane(path.clone()))
                .unwrap()
                .annotations
                .set_field(
                    &camera,
                    f::CAMERA,
                    Some(Value::record([
                        (
                            f::YAW,
                            crate::libraries::f32::value(30.0 + iteration as f32 * 2.0),
                        ),
                        (f::PITCH, crate::libraries::f32::value(60.0)),
                        (f::ZOOM, crate::libraries::f32::value(1.0)),
                    ])),
                );
            let start = Instant::now();
            runner.refresh_frame(scale, size);
            let mut list = DrawList::new();
            puri::frame::render(runner.prepare_paint(scale, size).renders, &mut list);
            runner.frame_presented();
            if iteration >= 4 {
                builds.push(start.elapsed());
            }
            let mut reference = None;
            let mut current = None;
            for mode in [iteration % 2, 1 - iteration % 2] {
                gpu.wait();
                let start = Instant::now();
                let roundtrip;
                let commands = if mode == 0 {
                    roundtrip = gpu::mesh_images(&gpu, &mut renderer, &mut readback, &list);
                    &roundtrip
                } else {
                    &list
                };
                let output =
                    gpu::composite(&gpu, &mut compositor, &mut resources, &target, commands);
                gpu.wait();
                if iteration >= 4 {
                    timings[mode].push(start.elapsed());
                }
                assert_eq!(compositor.counts.uploads, usize::from(mode == 0));
                if iteration == 23 {
                    if mode == 0 {
                        reference = Some(gpu.read(&output));
                    } else {
                        current = Some(gpu.read(&output));
                    }
                }
            }
            // The roundtrip rasterizes the mesh into an image first, so its
            // silhouette resolves a little differently from the direct pass:
            // a few hundred edge pixels of four million. The composition as a
            // whole must match.
            if iteration == 23 {
                let (reference, current) = (reference.unwrap(), current.unwrap());
                let differing = reference
                    .chunks(4)
                    .zip(current.chunks(4))
                    .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 2))
                    .count();
                assert!(
                    differing * 1000 <= reference.len() / 4,
                    "{differing} pixels differ between the direct and roundtrip composites"
                );
            }
        }
        builds.sort();
        eprintln!(
            "CAM {progress}: frame build/record median {:?}",
            builds[builds.len() / 2]
        );
        for (name, mut times) in ["roundtrip", "direct"].into_iter().zip(timings) {
            crate::perf::record(&format!("{progress}/{name}"), times.iter().copied());
            times.sort();
            eprintln!(
                "{name}: median {:?}, max {:?}, min {:?}",
                times[times.len() / 2],
                times[times.len() - 1],
                times[0]
            );
        }
    }
}
