//! Renders one scene several times with each of Vello's antialiasing methods
//! and counts the pixels that differ from its first render. The scene and
//! renderer never change, so every count should be zero.
//!
//!     cargo run --release --example msaa_repro [CIRCLES [RADIUS_SCALE]]

use vello::kurbo::{Affine, Circle};
use vello::peniko::{Color, Fill};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene, wgpu};

const SIZE: u32 = 1024;
const RENDERS: usize = 6;

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let info = adapter.get_info();
    println!("{} ({:?}, {})", info.name, info.backend, info.driver_info);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits::default().or_worse_values_from(&adapter.limits()),
        ..Default::default()
    }))
    .unwrap();
    let mut renderer = Renderer::new(&device, RendererOptions::default()).unwrap();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let circles = std::env::args()
        .nth(1)
        .map_or(20_000, |count| count.parse().unwrap());
    let scale = std::env::args()
        .nth(2)
        .map_or(1.0, |scale| scale.parse().unwrap());
    println!("{circles} circles, radii scaled by {scale}");
    let scene = scene(circles, scale);
    for (name, method) in [
        ("area", AaConfig::Area),
        ("msaa8", AaConfig::Msaa8),
        ("msaa16", AaConfig::Msaa16),
    ] {
        let renders: Vec<_> = (0..RENDERS)
            .map(|_| {
                renderer
                    .render_to_texture(
                        &device,
                        &queue,
                        &scene,
                        &view,
                        &RenderParams {
                            base_color: Color::WHITE,
                            width: SIZE,
                            height: SIZE,
                            antialiasing_method: method,
                        },
                    )
                    .unwrap();
                read(&device, &queue, &texture)
            })
            .collect();
        let (first, rest) = renders.split_first().unwrap();
        let differing: Vec<_> = rest.iter().map(|render| differing(first, render)).collect();
        println!(
            "{name:>6}: pixels differing from the first render: {:?}",
            differing.iter().map(Vec::len).collect::<Vec<_>>()
        );
        for (x, y, a, b) in differing.iter().flatten().take(4) {
            println!("        at ({x}, {y}): {a:?} then {b:?}");
        }
    }
}

/// `circles` opaque circles at pseudo-random places, 1.5 to 6 pixels in
/// radius times `scale`; twenty thousand small ones give most tiles many
/// fills with antialiased edges.
fn scene(circles: usize, scale: f64) -> Scene {
    let mut scene = Scene::new();
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..circles {
        let center = (next() * f64::from(SIZE), next() * f64::from(SIZE));
        let radius = (1.5 + 4.5 * next()) * scale;
        let color = Color::from_rgba8(
            (next() * 255.0) as u8,
            (next() * 255.0) as u8,
            (next() * 255.0) as u8,
            255,
        );
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            color,
            None,
            &Circle::new(center, radius),
        );
    }
    scene
}

/// The pixels whose channels differ between `a` and `b`: where each is, and
/// its value in each.
fn differing<'a>(a: &'a [u8], b: &'a [u8]) -> Vec<(u32, u32, &'a [u8], &'a [u8])> {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(index, (a, b))| (index as u32 % SIZE, index as u32 / SIZE, a, b))
        .collect()
}

/// `texture`'s RGBA pixels, row by row.
fn read(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let row_bytes = texture.width() * 4;
    let stride = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(stride) * u64::from(texture.height()),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(texture.height()),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| result.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = buffer.slice(..).get_mapped_range().unwrap();
    mapped
        .chunks_exact(stride as usize)
        .flat_map(|row| &row[..row_bytes as usize])
        .copied()
        .collect()
}
