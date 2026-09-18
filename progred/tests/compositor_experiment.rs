//! Headless checks of the production compositor; never launches the app.
#[path = "compositor_experiment/gpu.rs"]
mod gpu;

use gpu::Gpu;
use puri::draw::{Canvas, CanvasSink};
use puri_vello::VelloCanvas;
use puri_vello::compositor::{Compositor, Layer, Resources, SplitCanvas};
use std::{path::Path, time::Instant};
use vello::{
    AaConfig, RenderParams, Renderer, RendererOptions, Scene,
    kurbo::{Affine, Circle, Rect, RoundedRect},
    peniko::{Color, ImageAlphaType, ImageData, ImageFormat},
};

fn image(width: u32, height: u32, rgba: impl Fn(u32, u32) -> [u8; 4]) -> ImageData {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            pixels.extend(rgba(x, y));
        }
    }
    ImageData {
        data: pixels.into(),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width,
        height,
    }
}

fn pattern() -> ImageData {
    image(80, 64, |x, y| {
        [
            ((x * 3) % 256) as u8,
            ((y * 4) % 256) as u8,
            if (x / 10 + y / 8) % 2 == 0 { 240 } else { 30 },
            if x < 20 { 60 } else { 220 },
        ]
    })
}

fn scene(canvas: &mut dyn CanvasSink, pattern: &ImageData, complex: bool) {
    canvas.fill(
        Rect::new(0.0, 0.0, 256.0, 192.0),
        Color::from_rgb8(32, 55, 70),
        Affine::IDENTITY,
    );
    canvas.image(
        pattern.clone(),
        Affine::translate((12.0, 9.0)) * Affine::scale(2.0),
    );
    canvas.clip(
        Rect::new(22.0, 28.0, 220.0, 184.0),
        Affine::IDENTITY,
        |canvas| {
            canvas.fill(
                Rect::new(10.0, 14.0, 110.0, 176.0),
                Color::new([0.9, 0.2, 0.1, 0.5]),
                Affine::IDENTITY,
            );
            canvas.image(
                pattern.clone(),
                Affine::translate((130.0, 35.0)) * Affine::scale(1.4),
            );
            canvas.fill(
                Rect::new(145.0, 80.0, 235.0, 118.0),
                Color::new([0.1, 0.9, 0.2, 0.6]),
                Affine::IDENTITY,
            );
            if complex {
                canvas.clip(
                    Circle::new((104.3, 107.8), 62.6),
                    Affine::IDENTITY,
                    |canvas| {
                        canvas.fill(
                            Rect::new(10.0, 20.0, 220.0, 185.0),
                            Color::new([0.2, 0.1, 0.8, 0.65]),
                            Affine::IDENTITY,
                        );
                        canvas.clip(
                            RoundedRect::new(58.2, 56.6, 162.8, 163.1, 17.4),
                            Affine::IDENTITY,
                            |canvas| {
                                canvas.image(
                                    pattern.clone(),
                                    Affine::translate((67.3, 48.1))
                                        * Affine::rotate(0.22)
                                        * Affine::scale(1.7),
                                );
                                canvas.fill(
                                    Rect::new(20.0, 96.0, 192.0, 130.0),
                                    Color::new([1.0, 0.8, 0.1, 0.4]),
                                    Affine::IDENTITY,
                                );
                            },
                        );
                        canvas.fill(
                            Circle::new((139.2, 125.1), 30.5),
                            Color::new([0.1, 0.8, 0.9, 0.35]),
                            Affine::IDENTITY,
                        );
                    },
                );
            }
        },
    );
    canvas.fill(
        Rect::new(50.0, 164.0, 236.0, 180.0),
        Color::new([0.8, 0.85, 1.0, 0.8]),
        Affine::IDENTITY,
    );
}

fn vello(gpu: &Gpu, renderer: &mut Renderer, scene: &Scene, target: &vello::wgpu::Texture) {
    renderer
        .render_to_texture(
            &gpu.device,
            &gpu.queue,
            scene,
            &target.create_view(&Default::default()),
            &RenderParams {
                base_color: Color::TRANSPARENT,
                width: target.width(),
                height: target.height(),
                antialiasing_method: AaConfig::Msaa16,
            },
        )
        .unwrap();
}

fn png(path: &Path, width: u32, height: u32, pixels: &[u8]) {
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels)
        .unwrap();
}

#[test]
#[ignore = "headless GPU experiment; requires explicit GPU access"]
fn compositor_pixels() {
    let gpu = Gpu::new();
    let reference = gpu.texture(256, 192);
    let actual = gpu.texture(256, 192);
    let mut renderer = Renderer::new(&gpu.device, RendererOptions::default()).unwrap();
    let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
    let mut resources = Resources::default();
    let pattern = pattern();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/compositor-experiment");
    std::fs::create_dir_all(&dir).unwrap();
    for complex in [false, true] {
        let mut original = Scene::new();
        scene(&mut VelloCanvas(&mut original), &pattern, complex);
        vello(&gpu, &mut renderer, &original, &reference);
        let mut split = SplitCanvas::default();
        scene(&mut split, &pattern, complex);
        let output = compositor
            .render(
                &gpu.device,
                &gpu.queue,
                &split.finish(),
                &mut resources,
                &actual,
                Color::TRANSPARENT,
            )
            .unwrap();
        let expected = gpu.read(&reference);
        let pixels = gpu.read(&output.texture);
        let mut errors: Vec<_> = expected
            .chunks_exact(4)
            .zip(pixels.chunks_exact(4))
            .map(|(a, b)| a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)).max().unwrap())
            .collect();
        errors.sort_unstable();
        let over4 = errors.iter().filter(|e| **e > 4).count();
        eprintln!(
            "complex={complex}: max={} p99={} pixels_over_4={over4}/{} {:?}",
            errors.last().unwrap(),
            errors[errors.len() * 99 / 100],
            errors.len(),
            compositor.counts
        );
        png(
            &dir.join(format!("reference-{complex}.png")),
            256,
            192,
            &expected,
        );
        png(
            &dir.join(format!("composite-{complex}.png")),
            256,
            192,
            &pixels,
        );
        let difference: Vec<_> = expected
            .chunks_exact(4)
            .zip(pixels.chunks_exact(4))
            .flat_map(|(a, b)| {
                [
                    a[0].abs_diff(b[0]).saturating_mul(12),
                    a[1].abs_diff(b[1]).saturating_mul(12),
                    a[2].abs_diff(b[2]).saturating_mul(12),
                    255,
                ]
            })
            .collect();
        png(
            &dir.join(format!("difference-{complex}.png")),
            256,
            192,
            &difference,
        );
        assert!(
            errors[errors.len() * 99 / 100] <= 2,
            "large interior differences"
        );
        assert!(
            *errors.last().unwrap() <= 3,
            "clipping/filtering disagreement"
        );
    }
    // The source is produced only on the GPU. Readback occurs after composition,
    // solely for this assertion, not between producer and compositor.
    let source = gpu.texture(256, 192);
    gpu.clear(
        &source,
        vello::wgpu::Color {
            r: 0.2,
            g: 0.4,
            b: 0.6,
            a: 1.0,
        },
    );
    let output = compositor
        .render(
            &gpu.device,
            &gpu.queue,
            &[Layer::Texture(source, Affine::IDENTITY)],
            &mut resources,
            &actual,
            Color::TRANSPARENT,
        )
        .unwrap();
    assert!(
        gpu.read(&output.texture)
            .chunks_exact(4)
            .all(|p| p == [51, 102, 153, 255])
    );
}

fn preview(canvas: &mut dyn CanvasSink, image: &ImageData, width: u32, height: u32) {
    canvas.fill(
        Rect::new(0.0, 0.0, width as f64, height as f64),
        Color::from_rgb8(35, 40, 50),
        Affine::IDENTITY,
    );
    canvas.clip(
        Rect::new(0.0, 0.0, width as f64, height as f64),
        Affine::IDENTITY,
        |canvas| {
            canvas.image(image.clone(), Affine::IDENTITY);
            canvas.fill(
                RoundedRect::new(
                    20.0,
                    height as f64 - 100.0,
                    width as f64 - 20.0,
                    height as f64 - 20.0,
                    12.0,
                ),
                Color::new([0.2, 0.25, 0.3, 0.7]),
                Affine::IDENTITY,
            );
        },
    );
}

#[test]
#[ignore = "headless GPU experiment; requires explicit GPU access"]
fn compositor_updates_and_timing() {
    let gpu = Gpu::new();
    for width in [1200, 4000, 4200] {
        let height = if width == 1200 { 800 } else { 2800 };
        let reference = gpu.texture(width, height);
        let target = gpu.texture(width, height);
        let mut renderer = Renderer::new(&gpu.device, RendererOptions::default()).unwrap();
        let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
        let mut resources = Resources::default();
        let mut reference_times = Vec::new();
        let mut composite_times = Vec::new();
        let mut resident_times = Vec::new();
        let mut missing = 0;
        for frame in 0..8 {
            // The pane width changes each frame; large backing textures do not
            // enter Vello's atlas on the composition path.
            let image_width = width - frame * 3;
            let image = image(image_width, height, |_, _| [220 + frame as u8, 50, 80, 255]);
            let mut original = Scene::new();
            preview(&mut VelloCanvas(&mut original), &image, width, height);
            let mut split = SplitCanvas::default();
            preview(&mut split, &image, width, height);
            let split = split.finish();
            gpu.wait();
            let start = Instant::now();
            vello(&gpu, &mut renderer, &original, &reference);
            gpu.wait();
            let reference_time = start.elapsed();
            let start = Instant::now();
            let output = compositor
                .render(
                    &gpu.device,
                    &gpu.queue,
                    &split,
                    &mut resources,
                    &target,
                    Color::TRANSPARENT,
                )
                .unwrap();
            gpu.wait();
            let composite_time = start.elapsed();
            let offset = ((height / 2 * width + width / 2) * 4) as usize;
            let pixels = gpu.read(&output.texture);
            assert_eq!(
                &pixels[offset..offset + 4],
                &[220 + frame as u8, 50, 80, 255]
            );
            if gpu.read(&reference)[offset] < 200 {
                missing += 1;
            }
            // Simulate an independent GPU producer, without CPU image upload
            // or readback. Producer work is outside the presentation timing.
            let source = gpu.texture(image_width, height);
            gpu.clear(
                &source,
                vello::wgpu::Color {
                    r: f64::from(220 + frame as u8) / 255.0,
                    g: 50.0 / 255.0,
                    b: 80.0 / 255.0,
                    a: 1.0,
                },
            );
            let resident = replace_image(split, &source);
            gpu.wait();
            let start = Instant::now();
            let output = compositor
                .render(
                    &gpu.device,
                    &gpu.queue,
                    &resident,
                    &mut resources,
                    &target,
                    Color::TRANSPARENT,
                )
                .unwrap();
            gpu.wait();
            let resident_time = start.elapsed();
            let pixels = gpu.read(&output.texture);
            assert_eq!(
                &pixels[offset..offset + 4],
                &[220 + frame as u8, 50, 80, 255]
            );
            assert_eq!(compositor.counts.uploads, 0);
            if frame > 1 {
                reference_times.push(reference_time);
                composite_times.push(composite_time);
                resident_times.push(resident_time);
            }
        }
        reference_times.sort();
        composite_times.sort();
        resident_times.sort();
        eprintln!(
            "{width}x{height}: reference_missing={missing}/8 median_reference={:?} median_composite={:?} median_gpu_resident={:?} {:?}",
            reference_times[reference_times.len() / 2],
            composite_times[composite_times.len() / 2],
            resident_times[resident_times.len() / 2],
            compositor.counts
        );
        assert_eq!(missing > 0, width == 4200);
    }
}

fn replace_image(layers: Vec<Layer>, texture: &vello::wgpu::Texture) -> Vec<Layer> {
    layers
        .into_iter()
        .map(|layer| match layer {
            Layer::Image(_, transform) => Layer::Texture(texture.clone(), transform),
            Layer::Clip(shape, transform, children) => {
                Layer::Clip(shape, transform, replace_image(children, texture))
            }
            other => other,
        })
        .collect()
}

fn panes(canvas: &mut dyn CanvasSink, images: &[ImageData], width: u32, height: u32) {
    canvas.fill(
        Rect::new(0.0, 0.0, width as f64, height as f64),
        Color::from_rgb8(35, 40, 50),
        Affine::IDENTITY,
    );
    let columns = if images.len() == 1 { 1 } else { 2 };
    for (i, image) in images.iter().enumerate() {
        let x = (i % columns) as f64 * (width / columns as u32) as f64;
        let y = (i / columns) as f64 * (height / images.len().div_ceil(columns) as u32) as f64;
        let rect = Rect::new(x, y, x + image.width as f64, y + image.height as f64);
        canvas.clip(rect, Affine::IDENTITY, |canvas| {
            canvas.image(image.clone(), Affine::translate((x, y)));
            canvas.fill(
                Rect::new(x + 20.0, rect.y1 - 70.0, rect.x1 - 20.0, rect.y1 - 20.0),
                Color::new([0.2, 0.25, 0.3, 0.7]),
                Affine::IDENTITY,
            );
        });
    }
}

#[test]
#[ignore = "headless GPU measurement; requires explicit GPU access"]
fn compositor_multiple_previews() {
    let gpu = Gpu::new();
    let (width, height) = (2400, 1600);
    let reference = gpu.texture(width, height);
    let scratch = gpu.texture(width, height);
    for count in [1, 2, 4] {
        for changing in [false, true] {
            let mut renderer = Renderer::new(&gpu.device, RendererOptions::default()).unwrap();
            let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
            let mut resources = Resources::default();
            let make_images = |frame| {
                (0..count)
                    .map(|n| {
                        image(
                            width / if count == 1 { 1 } else { 2 },
                            height / if count == 4 { 2 } else { 1 },
                            |_, _| [150 + n as u8 * 20, 50 + frame as u8, 80, 255],
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let mut images = make_images(0);
            let mut direct = Vec::new();
            let mut composed = Vec::new();
            for frame in 0..16 {
                if changing {
                    images = make_images(frame);
                }
                let mut original = Scene::new();
                panes(&mut VelloCanvas(&mut original), &images, width, height);
                let mut split = SplitCanvas::default();
                panes(&mut split, &images, width, height);
                let layers = split.finish();
                let mut durations = [Default::default(); 2];
                let mut output = None;
                for index in if frame % 2 == 0 { [0, 1] } else { [1, 0] } {
                    gpu.wait();
                    let start = Instant::now();
                    if index == 0 {
                        vello(&gpu, &mut renderer, &original, &reference);
                    } else {
                        output = Some(
                            compositor
                                .render(
                                    &gpu.device,
                                    &gpu.queue,
                                    &layers,
                                    &mut resources,
                                    &scratch,
                                    Color::TRANSPARENT,
                                )
                                .unwrap(),
                        );
                    }
                    gpu.wait();
                    durations[index] = start.elapsed();
                }
                assert_eq!(
                    compositor.counts.uploads,
                    if changing || frame == 0 { count } else { 0 }
                );
                assert_eq!(resources.uploaded_images(), count);
                assert_eq!(resources.scratch_textures(), 1);
                if frame >= 4 {
                    direct.push(durations[0]);
                    composed.push(durations[1]);
                }
                if frame == 15 {
                    let expected = gpu.read(&reference);
                    let actual = gpu.read(&output.unwrap().texture);
                    assert!(
                        expected
                            .iter()
                            .zip(&actual)
                            .all(|(a, b)| a.abs_diff(*b) <= 2)
                    );
                }
            }
            direct.sort();
            composed.sort();
            eprintln!(
                "panes={count} changing={changing}: direct={:?} composed={:?} {:?}",
                direct[direct.len() / 2],
                composed[composed.len() / 2],
                compositor.counts
            );
        }
    }
}

#[test]
#[ignore = "headless GPU regression; requires explicit GPU access"]
fn compositor_resource_lifetimes_and_formats() {
    let gpu = Gpu::new();
    let mut compositor = Compositor::new(&gpu.device, &gpu.queue).unwrap();
    let mut resources = Resources::default();
    let mut other_window = Resources::default();
    let a = image(16, 16, |_, _| [64, 32, 16, 128]);
    let mut b = a.clone();
    b.format = ImageFormat::Bgra8;
    let mut c = a.clone();
    c.alpha_type = ImageAlphaType::AlphaPremultiplied;
    for (i, (w, h)) in [(32, 32), (48, 40), (24, 20)].into_iter().enumerate() {
        let scratch = gpu.texture(w, h);
        let layers = [
            Layer::Image(a.clone(), Affine::IDENTITY),
            Layer::Image(b.clone(), Affine::translate((16.0, 0.0))),
            Layer::Image(c.clone(), Affine::translate((0.0, 16.0))),
        ];
        let output = compositor
            .render(
                &gpu.device,
                &gpu.queue,
                &layers,
                &mut resources,
                &scratch,
                Color::BLACK,
            )
            .unwrap();
        assert_eq!(compositor.counts.uploads, if i == 0 { 2 } else { 0 });
        assert_eq!(resources.uploaded_images(), 2);
        assert_eq!(resources.scratch_textures(), 1);
        let bytes = gpu.read(&output.texture);
        assert_eq!(&bytes[..4], &[32, 16, 8, 255]);
        let p = (16 * 4) as usize;
        assert_eq!(&bytes[p..p + 4], &[8, 16, 32, 255]);
        let p = (16 * w * 4) as usize;
        assert_eq!(&bytes[p..p + 4], &[64, 32, 16, 255]);
        let other = [Layer::Image(
            image(8, 8, |_, _| [200, 0, 0, 255]),
            Affine::IDENTITY,
        )];
        compositor
            .render(
                &gpu.device,
                &gpu.queue,
                &other,
                &mut other_window,
                &scratch,
                Color::BLACK,
            )
            .unwrap();
        compositor
            .render(
                &gpu.device,
                &gpu.queue,
                &layers,
                &mut resources,
                &scratch,
                Color::BLACK,
            )
            .unwrap();
        assert_eq!(compositor.counts.uploads, 0);
        assert_eq!(other_window.uploaded_images(), 1);
    }
    let scratch = gpu.texture(32, 32);
    let mut clipped = SplitCanvas::default();
    clipped.clip(
        Circle::new((16.0, 16.0), 10.0),
        Affine::IDENTITY,
        |canvas| canvas.image(a.clone(), Affine::IDENTITY),
    );
    compositor
        .render(
            &gpu.device,
            &gpu.queue,
            &clipped.finish(),
            &mut resources,
            &scratch,
            Color::BLACK,
        )
        .unwrap();
    assert_eq!(resources.scratch_textures(), 3);
    compositor
        .render(
            &gpu.device,
            &gpu.queue,
            &[Layer::Image(a.clone(), Affine::IDENTITY)],
            &mut resources,
            &scratch,
            Color::BLACK,
        )
        .unwrap();
    assert_eq!(resources.scratch_textures(), 1);
    assert_eq!(resources.uploaded_images(), 1);
    let mut vectors = SplitCanvas::default();
    vectors.fill(
        Rect::new(0.0, 0.0, 32.0, 32.0),
        Color::WHITE,
        Affine::IDENTITY,
    );
    compositor
        .render(
            &gpu.device,
            &gpu.queue,
            &vectors.finish(),
            &mut resources,
            &scratch,
            Color::BLACK,
        )
        .unwrap();
    assert_eq!(compositor.counts.vector_passes, 1);
    assert_eq!(compositor.counts.blends, 0);
    assert_eq!(resources.uploaded_images(), 0);
    assert_eq!(resources.scratch_textures(), 0);

    let output = compositor
        .render(
            &gpu.device,
            &gpu.queue,
            &[Layer::Image(a, Affine::IDENTITY)],
            &mut resources,
            &scratch,
            Color::TRANSPARENT,
        )
        .unwrap();
    assert_eq!(output.alpha_type, ImageAlphaType::AlphaPremultiplied);
    assert_eq!(&gpu.read(&output.texture)[..4], &[32, 16, 8, 128]);

    let mut drawing = puri::draw::DrawList::new();
    drawing.fill(
        Rect::new(0.0, 0.0, 32.0, 32.0),
        Color::BLACK,
        Affine::IDENTITY,
    );
    drawing.fill(
        Rect::new(0.0, 0.0, 32.0, 32.0),
        Color::new([1.0, 0.0, 0.0, 0.5]),
        Affine::IDENTITY,
    );
    drawing.image(image(16, 16, |_, _| [0, 64, 128, 128]), Affine::IDENTITY);
    let mut split = SplitCanvas::default();
    puri::draw::replay(&drawing, &mut split);
    let output = compositor
        .render(
            &gpu.device,
            &gpu.queue,
            &split.finish(),
            &mut resources,
            &scratch,
            Color::BLACK,
        )
        .unwrap();
    assert_eq!(compositor.counts.vector_passes, 1);
    assert_eq!(compositor.counts.blends, 1);
    let mut scene = Scene::new();
    puri::draw::replay(&drawing, &mut VelloCanvas(&mut scene));
    let reference = gpu.texture(32, 32);
    let mut renderer = Renderer::new(&gpu.device, RendererOptions::default()).unwrap();
    vello(&gpu, &mut renderer, &scene, &reference);
    assert!(
        gpu.read(&reference)
            .iter()
            .zip(gpu.read(&output.texture))
            .all(|(a, b)| a.abs_diff(b) <= 2)
    );

    let invalid = image(4, 4, |_, _| [0; 4]);
    let invalid = ImageData {
        width: 10,
        ..invalid
    };
    assert!(
        compositor
            .render(
                &gpu.device,
                &gpu.queue,
                &[Layer::Image(invalid, Affine::IDENTITY)],
                &mut resources,
                &scratch,
                Color::BLACK
            )
            .is_err()
    );
}
