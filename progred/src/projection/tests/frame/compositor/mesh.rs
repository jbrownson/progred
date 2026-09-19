//! Compare the direct mesh interpretation with the former image round trip.
use super::*;
use puri::{draw::Canvas, mesh};

/// The old renderer retained its readback allocation and copied rows in bulk.
/// Keep those optimizations in the comparison; do not time a naive readback.
#[derive(Default)]
pub(super) struct Readback(Option<vello::wgpu::Buffer>);

impl Readback {
    fn read(&mut self, gpu: &gpu::Gpu, texture: &vello::wgpu::Texture) -> Vec<u8> {
        use vello::wgpu;
        let row_bytes = texture.width() * 4;
        let stride = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let size = u64::from(stride) * u64::from(texture.height());
        if self.0.as_ref().is_none_or(|buffer| buffer.size() != size) {
            self.0 = Some(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("retained diagnostic readback"),
                size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }
        let buffer = self.0.as_ref().unwrap();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(texture.height()),
                },
            },
            texture.size(),
        );
        gpu.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap();
            });
        gpu.wait();
        receive.recv().unwrap().unwrap();
        let mapped = buffer.slice(..).get_mapped_range();
        let mut bytes = Vec::with_capacity(row_bytes as usize * texture.height() as usize);
        for row in mapped.chunks_exact(stride as usize) {
            bytes.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();
        bytes
    }
}

pub(super) fn images(
    gpu: &gpu::Gpu,
    renderer: &mut puri_vello::mesh::Renderer,
    readback: &mut Readback,
    list: &DrawList,
) -> DrawList {
    DrawList(
        list.0
            .iter()
            .map(|cmd| match cmd {
                DrawCmd::Mesh { scene, transform } => {
                    let texture = renderer
                        .render(&scene.geometry, &scene.view, scene.surface.as_ref())
                        .unwrap();
                    DrawCmd::Image {
                        image: ImageData {
                            data: readback.read(gpu, &texture).into(),
                            width: scene.view.width,
                            height: scene.view.height,
                            format: peniko::ImageFormat::Rgba8,
                            alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
                        },
                        transform: *transform,
                    }
                }
                DrawCmd::Clip {
                    shape,
                    transform,
                    children,
                } => DrawCmd::Clip {
                    shape: shape.clone(),
                    transform: *transform,
                    children: images(gpu, renderer, readback, &DrawList(children.clone())).0,
                },
                other => other.clone(),
            })
            .collect(),
    )
}

fn compose(
    gpu: &gpu::Gpu,
    compositor: &mut Compositor,
    resources: &mut Resources,
    target: &vello::wgpu::Texture,
    list: &DrawList,
) -> vello::wgpu::Texture {
    let mut canvas = SplitCanvas::default();
    puri::draw::replay(list, &mut canvas);
    compositor
        .render(
            &gpu.device,
            &gpu.queue,
            &canvas.finish(),
            resources,
            target,
            Color::WHITE,
        )
        .unwrap()
        .texture
}

fn triangle(color: [f32; 3]) -> mesh::Scene {
    mesh::Scene {
        geometry: Arc::new(mesh::Geometry {
            vertices: [[-0.9, -0.8, 0.0], [0.9, -0.8, 0.0], [0.0, 0.9, 0.0]]
                .map(|p| mesh::Vertex {
                    position: nalgebra::Vector3::from(p),
                    color,
                    normal: Default::default(),
                })
                .into(),
            indices: vec![0, 1, 2],
        }),
        view: mesh::View {
            model_to_view: nalgebra::Matrix4::identity(),
            projection: [1.0, 1.0, -0.5, 0.5],
            width: 64,
            height: 64,
        },
        surface: None,
    }
}

#[test]
#[ignore = "headless GPU mesh ordering, clipping, and upload regression"]
fn mesh_layers_match_roundtrip_with_clips_and_reused_target() {
    let gpu = gpu::Gpu::new();
    let mut renderer = puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue);
    let mut readback = Readback::default();
    let target = gpu.texture(180, 90);
    let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
    let mut resources = Resources::default();
    let mut red = triangle([1.0, 0.0, 0.0]);
    for edited in [false, true] {
        if edited {
            for v in &mut Arc::make_mut(&mut red.geometry).vertices {
                v.color = [0.0, 1.0, 0.0];
            }
        }
        let mut list = DrawList::new();
        list.fill(
            Rect::new(0.0, 0.0, 180.0, 90.0),
            Color::WHITE,
            Affine::IDENTITY,
        );
        list.clip(
            Rect::new(8.0, 8.0, 55.0, 65.0),
            Affine::IDENTITY,
            |canvas| {
                canvas.mesh(red.clone(), Affine::translate((5.0, 5.0)));
                canvas.fill(
                    Rect::new(25.0, 30.0, 35.0, 40.0),
                    Color::BLACK,
                    Affine::IDENTITY,
                );
            },
        );
        list.clip(
            kurbo::Circle::new((115.0, 40.0), 26.3),
            Affine::IDENTITY,
            |canvas| {
                canvas.mesh(triangle([0.0, 0.0, 1.0]), Affine::translate((80.0, 8.0)));
            },
        );
        let reference = images(&gpu, &mut renderer, &mut readback, &list);
        let expected = gpu.read(&compose(
            &gpu,
            &mut compositor,
            &mut resources,
            &target,
            &reference,
        ));
        let actual = gpu.read(&compose(
            &gpu,
            &mut compositor,
            &mut resources,
            &target,
            &list,
        ));
        assert_eq!(
            actual, expected,
            "same-sized scratch targets must composite before reuse"
        );
        assert_eq!(compositor.counts.uploads, 0);
        let pixel = |x: usize, y: usize| &actual[(y * 180 + x) * 4..(y * 180 + x + 1) * 4];
        assert_eq!(
            pixel(30, 35),
            [0, 0, 0, 255],
            "vector overlay paints after mesh"
        );
        assert_eq!(pixel(60, 50), [255; 4], "rectangular clip");
        assert!(pixel(35, 45)[usize::from(edited)] > 180);
        assert!(pixel(112, 45)[2] > 180 && pixel(112, 45)[0] == 0);
        assert_eq!(pixel(85, 10), [255; 4], "rounded clip");
    }
}

#[test]
#[ignore = "paired full-editor CAM orbit draw/composition timing; no app launch"]
fn cam_mesh_roundtrip_profile() {
    use super::super::svg;
    use crate::libraries::{
        controls::vocabulary::STATE, fidget::vocabulary as f, toolpath::vocabulary as t,
    };
    let gpu = gpu::Gpu::new();
    let size = kurbo::Size::new(2400.0, 1800.0);
    let scale = 2.0;
    for progress in [0.02, 0.5] {
        let mut editor = svg::cam_editor(t::PREVIEW_MESH);
        editor.model.workspace.left_width = 0.5;
        let path = crate::workspace::declarations(editor.model.doc.root.as_ref())[0]
            .path
            .clone();
        let controls = svg::cam_controls_path(&path);
        let camera = svg::result_path(&controls);
        editor.model.workspace.left.panes[0]
            .view
            .annotations
            .set_field(&controls, STATE, Some(svg::cam_position(progress)));
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
        let target = gpu.texture(size.width as u32, size.height as u32);
        let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
        let mut resources = Resources::default();
        let mut renderer = puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue);
        let mut readback = Readback::default();
        let mut timings = [Vec::new(), Vec::new()];
        let mut builds = Vec::new();
        for iteration in 0..24 {
            runner.editor.model.workspace.left.panes[0]
                .view
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
                    roundtrip = images(&gpu, &mut renderer, &mut readback, &list);
                    &roundtrip
                } else {
                    &list
                };
                let output = compose(&gpu, &mut compositor, &mut resources, &target, commands);
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
            if iteration == 23 {
                assert_eq!(reference, current);
            }
        }
        builds.sort();
        eprintln!(
            "CAM {progress}: frame build/record median {:?}",
            builds[builds.len() / 2]
        );
        for (name, mut times) in ["roundtrip", "direct"].into_iter().zip(timings) {
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

#[test]
#[ignore = "headless GPU depth upload identity and premultiplied-alpha contract"]
fn depth_only_changes_refresh_the_surface_upload() {
    let gpu = gpu::Gpu::new();
    let mut renderer = puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue);
    let mut scene = triangle([1.0, 0.0, 0.0]);
    scene.surface = Some(mesh::Surface {
        frame: mesh::DepthImage {
            image: ImageData {
                data: vec![0, 0, 255, 255].into(),
                width: 1,
                height: 1,
                format: peniko::ImageFormat::Rgba8,
                alpha_type: peniko::ImageAlphaType::Alpha,
            },
            depth: vec![0.8].into(),
            partial: false,
        },
        mesh_start: 3,
    });
    let render = |renderer: &mut puri_vello::mesh::Renderer, scene: &mesh::Scene| {
        gpu.read(
            &renderer
                .render(&scene.geometry, &scene.view, scene.surface.as_ref())
                .unwrap(),
        )
    };
    let center = (32 * 64 + 32) * 4;
    assert!(render(&mut renderer, &scene)[center] > 180);
    // Preserve image identity but change only depth. A retained weak identity
    // must observe Arc::make_mut, just as it does for geometry edits.
    Arc::make_mut(&mut scene.surface.as_mut().unwrap().frame.depth)[0] = 0.2;
    assert_eq!(
        &render(&mut renderer, &scene)[center..center + 4],
        &[0, 0, 255, 255]
    );
    scene.surface.as_mut().unwrap().frame.image.data = vec![200, 100, 50, 128].into();
    assert_eq!(
        &render(&mut renderer, &scene)[center..center + 4],
        &[100, 50, 25, 128]
    );
}
