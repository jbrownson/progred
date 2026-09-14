use super::{Geometry, View};
use fidget_engine::wgpu::{Gpu, wgpu};

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SAMPLES: u32 = 4;

#[derive(Default)]
pub(super) enum Backend {
    #[default]
    Uninitialized,
    Ready(Box<Renderer>),
    Unavailable,
}

impl Backend {
    pub(super) fn render(&mut self, geometry: &Geometry, view: &View) -> Option<Vec<u8>> {
        if matches!(self, Self::Uninitialized) {
            *self = match pollster::block_on(Gpu::init_basic()) {
                Ok(gpu) => Self::Ready(Box::new(Renderer::new(gpu))),
                Err(error) => {
                    eprintln!(
                        "Fidget mesh GPU unavailable: {error}; using CPU triangle rasterizer"
                    );
                    Self::Unavailable
                }
            };
        }
        let result = match self {
            Self::Ready(renderer) => renderer.render(geometry, view),
            Self::Uninitialized | Self::Unavailable => None,
        };
        if result.is_none() && matches!(self, Self::Ready(_)) {
            eprintln!("Fidget mesh GPU render failed; using CPU triangle rasterizer");
            *self = Self::Unavailable;
        }
        result
    }
}

struct StreamBuffer {
    buffer: wgpu::Buffer,
    usage: wgpu::BufferUsages,
}

impl StreamBuffer {
    fn new(device: &wgpu::Device, usage: wgpu::BufferUsages) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mesh stream"),
                size: 4,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            usage,
        }
    }

    fn write(&mut self, gpu: &Gpu, data: &[u8]) -> Option<()> {
        let size = data.len() as u64;
        if size > gpu.device.limits().max_buffer_size {
            return None;
        }
        if self.buffer.size() < size {
            self.buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mesh stream"),
                size,
                usage: self.usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !data.is_empty() {
            gpu.queue.write_buffer(&self.buffer, 0, data);
        }
        Some(())
    }
}

struct Target {
    output: wgpu::Texture,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    read: wgpu::Buffer,
    stride: u32,
}

impl Target {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = |format, sample_count, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mesh viewport"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let output = texture(
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let color = texture(
            COLOR_FORMAT,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        let depth = texture(
            wgpu::TextureFormat::Depth32Float,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        let stride = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            output,
            color,
            depth,
            read,
            stride,
        }
    }
}

pub(super) struct Renderer {
    gpu: Gpu,
    pipeline: wgpu::RenderPipeline,
    vertices: StreamBuffer,
    indices: StreamBuffer,
    uniform: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    target: Option<Target>,
}

impl Renderer {
    pub(super) fn new(gpu: Gpu) -> Self {
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("mesh shading"),
                source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()),
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mesh viewport"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 24,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                    }],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh camera"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bindings = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh camera"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            vertices: StreamBuffer::new(&gpu.device, wgpu::BufferUsages::VERTEX),
            indices: StreamBuffer::new(&gpu.device, wgpu::BufferUsages::INDEX),
            gpu,
            pipeline,
            uniform,
            bindings,
            target: None,
        }
    }

    pub(super) fn render(&mut self, geometry: &Geometry, view: &View) -> Option<Vec<u8>> {
        let limits = self.gpu.device.limits();
        if view.width == 0
            || view.height == 0
            || view.width > limits.max_texture_dimension_2d
            || view.height > limits.max_texture_dimension_2d
        {
            return None;
        }
        let index_count = u32::try_from(geometry.indices.len()).ok()?;
        if u64::from((view.width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
            * u64::from(view.height)
            > limits.max_buffer_size
        {
            return None;
        }
        let vertices: Vec<u8> = geometry
            .vertices
            .iter()
            .flat_map(|v| {
                v.position
                    .iter()
                    .copied()
                    .chain(v.color)
                    .flat_map(f32::to_ne_bytes)
            })
            .collect();
        let indices: Vec<u8> = geometry
            .indices
            .iter()
            .flat_map(|i| i.to_ne_bytes())
            .collect();
        self.vertices.write(&self.gpu, &vertices)?;
        self.indices.write(&self.gpu, &indices)?;
        let uniform: Vec<u8> = view
            .model_to_view
            .as_slice()
            .iter()
            .copied()
            .chain(view.projection)
            .flat_map(f32::to_ne_bytes)
            .collect();
        self.gpu.queue.write_buffer(&self.uniform, 0, &uniform);
        if !self
            .target
            .as_ref()
            .is_some_and(|t| t.output.width() == view.width && t.output.height() == view.height)
        {
            self.target = Some(Target::new(&self.gpu.device, view.width, view.height));
        }
        let target = self.target.as_ref()?;
        let output = target.output.create_view(&Default::default());
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mesh viewport"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.color,
                    depth_slice: None,
                    resolve_target: Some(&output),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            if index_count > 0 {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bindings, &[]);
                pass.set_vertex_buffer(0, self.vertices.buffer.slice(..));
                pass.set_index_buffer(self.indices.buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..index_count, 0, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            target.output.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &target.read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(target.stride),
                    rows_per_image: Some(view.height),
                },
            },
            target.output.size(),
        );
        self.gpu.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        target
            .read
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        self.gpu
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .ok()?;
        receive.recv().ok()?.ok()?;
        let mapped = target.read.slice(..).get_mapped_range();
        let mut rgba: Vec<u8> = mapped
            .chunks_exact(target.stride as usize)
            .flat_map(|row| row[..view.width as usize * 4].iter().copied())
            .collect();
        drop(mapped);
        target.read.unmap();
        // Resolving multisampled transparent edges produces premultiplied RGBA;
        // image_layout consumes straight-alpha images, like the other previews.
        for pixel in rgba.chunks_exact_mut(4) {
            let alpha = u32::from(pixel[3]);
            if alpha > 0 && alpha < 255 {
                for channel in &mut pixel[..3] {
                    *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
        }
        Some(rgba)
    }
}
