//! Headless GPU textures and readback for tests that compare rendered pixels.

use crate::compositor::{Compositor, Resources, SplitCanvas};
use crate::mesh::Renderer;
use puri::draw::{DrawCmd, DrawList};
use vello::peniko::{Color, ImageAlphaType, ImageData, ImageFormat};
use vello::wgpu;

pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        eprintln!("adapter: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: adapter.features()
                & (wgpu::Features::CLEAR_TEXTURE | wgpu::Features::PIPELINE_CACHE),
            required_limits: wgpu::Limits::default().or_worse_values_from(&adapter.limits()),
            ..Default::default()
        }))
        .unwrap();
        Self { device, queue }
    }

    pub fn texture(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    pub fn clear(&self, texture: &wgpu::Texture, color: wgpu::Color) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture.create_view(&Default::default()),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        self.queue.submit([encoder.finish()]);
    }

    pub fn wait(&self) {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }

    pub fn read(&self, texture: &wgpu::Texture) -> Vec<u8> {
        let stride = (texture.width() * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("diagnostic readback only"),
            size: u64::from(stride) * u64::from(texture.height()),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
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
        self.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap()
            });
        self.wait();
        receive.recv().unwrap().unwrap();
        let mapped = buffer.slice(..).get_mapped_range().unwrap();
        mapped
            .chunks_exact(stride as usize)
            .flat_map(|row| row[..texture.width() as usize * 4].iter().copied())
            .collect()
    }
}

/// A readback buffer kept from one read to the next, as meshes' image round
/// trip kept one, with rows copied in bulk. Timing that round trip against
/// direct composition needs those optimizations, not a naive readback.
#[derive(Default)]
pub struct Readback(Option<wgpu::Buffer>);

impl Readback {
    pub fn read(&mut self, gpu: &Gpu, texture: &wgpu::Texture) -> Vec<u8> {
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

/// `list` with each mesh rendered and read back into an image: how meshes
/// reached the compositor before it drew them directly.
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
