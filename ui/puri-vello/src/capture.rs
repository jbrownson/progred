//! Headless rendering and readback, for pixel tests and for pictures taken
//! offscreen: progred's control session's screenshots, and the pictures its
//! structural copies carry.

use crate::compositor::{self, Compositor, Resources, SplitCanvas};
use crate::mesh::Renderer;
use puri::draw::{DrawCmd, DrawList};
use vello::peniko::{Color, ImageAlphaType, ImageData, ImageFormat};
use vello::wgpu;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: adapter.features()
                & (wgpu::Features::CLEAR_TEXTURE | wgpu::Features::PIPELINE_CACHE),
            required_limits: wgpu::Limits::default().or_worse_values_from(&adapter.limits()),
            ..Default::default()
        }))
        .unwrap();
        Self { device, queue }
    }

    fn borrowed(&self) -> compositor::Gpu<'_> {
        compositor::Gpu {
            device: &self.device,
            queue: &self.queue,
        }
    }

    /// A texture like the compositor's own targets.
    pub fn texture(&self, width: u32, height: u32) -> wgpu::Texture {
        self.borrowed().texture(width, height)
    }

    pub fn clear(&self, texture: &wgpu::Texture, color: wgpu::Color) {
        self.borrowed().clear(texture, color);
    }

    pub fn wait(&self) {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }

    /// `texture`'s pixels, read through a new buffer; a `Readback` keeps
    /// one from one read to the next.
    pub fn read(&self, texture: &wgpu::Texture) -> Vec<u8> {
        Readback::default().read(self, texture)
    }
}

/// A readback buffer kept from one read to the next, with rows copied in
/// bulk, so that `mesh_images`' round trip is timed as a fair baseline for
/// direct composition.
#[derive(Default)]
pub struct Readback(Option<wgpu::Buffer>);

impl Readback {
    pub fn read(&mut self, gpu: &Gpu, texture: &wgpu::Texture) -> Vec<u8> {
        let row_bytes = texture.width() * 4;
        let stride = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let size = u64::from(stride) * u64::from(texture.height());
        if self.0.as_ref().is_none_or(|buffer| buffer.size() != size) {
            self.0 = Some(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("capture readback"),
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
        let mapped = buffer.slice(..).get_mapped_range().unwrap();
        let mut bytes = Vec::with_capacity(row_bytes as usize * texture.height() as usize);
        for row in mapped.chunks_exact(stride as usize) {
            bytes.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();
        bytes
    }
}

/// `list` with each mesh rendered and read back into an image: the baseline
/// that drawing meshes directly in the compositor is measured against.
pub fn mesh_images(
    gpu: &Gpu,
    renderer: &mut Renderer,
    readback: &mut Readback,
    list: &DrawList,
) -> DrawList {
    DrawList(
        list.0
            .iter()
            .map(|command| match command {
                DrawCmd::Mesh { scene, transform } => {
                    let texture = renderer
                        .render(&scene.geometry, &scene.view, scene.surface.as_ref())
                        .unwrap();
                    DrawCmd::Image {
                        image: ImageData {
                            data: readback.read(gpu, &texture).into(),
                            width: scene.view.width,
                            height: scene.view.height,
                            format: ImageFormat::Rgba8,
                            alpha_type: ImageAlphaType::AlphaPremultiplied,
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
                    children: mesh_images(gpu, renderer, readback, &DrawList(children.clone())).0,
                },
                other => other.clone(),
            })
            .collect(),
    )
}

/// `list` composited onto `target` by the production compositor, over white.
pub fn composite(
    gpu: &Gpu,
    compositor: &mut Compositor,
    resources: &mut Resources,
    target: &wgpu::Texture,
    list: &DrawList,
) -> wgpu::Texture {
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
