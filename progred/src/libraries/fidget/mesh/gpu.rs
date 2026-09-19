use super::{Geometry, Mesh, Surface, View};
use fidget_engine::wgpu::{Gpu, wgpu};
use std::sync::{Arc, Weak};

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SAMPLES: u32 = 4;

fn vertex_bytes(vertices: &[super::Vertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() * 28);
    for vertex in vertices {
        for component in vertex.position.iter().chain(&vertex.color) {
            bytes.extend_from_slice(&component.to_ne_bytes());
        }
        bytes.extend(vertex.normal.0.map(|n| n as u8));
    }
    bytes
}

fn packed_rows(mapped: &[u8], stride: usize, row_bytes: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(mapped.len() / stride * row_bytes);
    for row in mapped.chunks_exact(stride) {
        bytes.extend_from_slice(&row[..row_bytes]);
    }
    bytes
}

#[derive(Default)]
pub(super) enum Backend {
    #[default]
    Uninitialized,
    Ready(Box<Renderer>),
    Unavailable,
}

impl Backend {
    pub(super) fn render(
        &mut self,
        geometry: &Mesh,
        view: &View,
        surface: Option<Surface<'_>>,
    ) -> Option<Vec<u8>> {
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
            Self::Ready(renderer) => renderer.render(geometry, view, surface),
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

struct UploadedMesh {
    source: Weak<Geometry>,
    index_count: usize,
}

impl UploadedMesh {
    fn covers(&self, geometry: &Mesh, index_count: usize) -> bool {
        self.source.ptr_eq(&Arc::downgrade(geometry)) && self.index_count >= index_count
    }
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
    uploaded: Option<UploadedMesh>,
    uniform: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    target: Option<Target>,
    surface_pipeline: wgpu::RenderPipeline,
    surface_image: Option<(u64, wgpu::BindGroup)>,
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
                        array_stride: 28,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Snorm8x4],
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
        let surface_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("surface color and depth"),
                    entries: &[0, 1].map(|binding| wgpu::BindGroupLayoutEntry {
                        binding,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    }),
                });
        let surface_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("surface color and depth"),
                source: wgpu::ShaderSource::Wgsl(include_str!("surface.wgsl").into()),
            });
        let surface_pipeline =
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("replace completed surface pixels"),
                    layout: Some(&gpu.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: None,
                            bind_group_layouts: &[Some(&surface_layout)],
                            immediate_size: 0,
                        },
                    )),
                    vertex: wgpu::VertexState {
                        module: &surface_shader,
                        entry_point: Some("vertex"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: Default::default(),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(wgpu::CompareFunction::Always),
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: SAMPLES,
                        ..Default::default()
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &surface_shader,
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
        Self {
            vertices: StreamBuffer::new(&gpu.device, wgpu::BufferUsages::VERTEX),
            indices: StreamBuffer::new(&gpu.device, wgpu::BufferUsages::INDEX),
            uploaded: None,
            gpu,
            pipeline,
            uniform,
            bindings,
            target: None,
            surface_pipeline,
            surface_image: None,
        }
    }

    fn upload_surface(&mut self, frame: &super::super::raster::Frame) -> Option<()> {
        if self
            .surface_image
            .as_ref()
            .is_some_and(|(id, _)| *id == frame.image.data.id())
        {
            return Some(());
        }
        let image = &frame.image;
        let limit = self.gpu.device.limits().max_texture_dimension_2d;
        if image.width > limit || image.height > limit {
            return None;
        }
        let texture = |format, bytes: &[u8]| {
            let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("surface raster"),
                size: wgpu::Extent3d {
                    width: image.width,
                    height: image.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.gpu.queue.write_texture(
                texture.as_image_copy(),
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width * 4),
                    rows_per_image: Some(image.height),
                },
                texture.size(),
            );
            texture.create_view(&Default::default())
        };
        let color = texture(COLOR_FORMAT, image.data.data());
        let bytes: Vec<_> = frame.depth.iter().flat_map(|v| v.to_ne_bytes()).collect();
        let depth = texture(wgpu::TextureFormat::R32Float, &bytes);
        self.surface_image = Some((
            image.data.id(),
            self.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("surface raster"),
                    layout: &self.surface_pipeline.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&color),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&depth),
                        },
                    ],
                }),
        ));
        Some(())
    }

    fn upload_mesh(&mut self, geometry: &Mesh, index_count: usize) -> Option<()> {
        if !self
            .uploaded
            .as_ref()
            .is_some_and(|upload| upload.covers(geometry, index_count))
        {
            self.uploaded = None;
            let indices = &geometry.indices[..index_count];
            let vertex_count = indices.iter().max().map_or(0, |i| *i as usize + 1);
            self.vertices
                .write(&self.gpu, &vertex_bytes(&geometry.vertices[..vertex_count]))?;
            self.indices.write(
                &self.gpu,
                &indices
                    .iter()
                    .flat_map(|i| i.to_ne_bytes())
                    .collect::<Vec<_>>(),
            )?;
            self.uploaded = Some(UploadedMesh {
                source: Arc::downgrade(geometry),
                index_count,
            });
        }
        Some(())
    }

    pub(super) fn render(
        &mut self,
        geometry: &Mesh,
        view: &View,
        surface: Option<Surface<'_>>,
    ) -> Option<Vec<u8>> {
        let limits = self.gpu.device.limits();
        if view.width == 0
            || view.height == 0
            || view.width > limits.max_texture_dimension_2d
            || view.height > limits.max_texture_dimension_2d
        {
            return None;
        }
        let visible = super::visible_indices(geometry, surface);
        let index_count = u32::try_from(visible.len()).ok()?;
        if let Some(surface) = surface {
            self.upload_surface(surface.frame)?;
        }
        if u64::from((view.width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
            * u64::from(view.height)
            > limits.max_buffer_size
        {
            return None;
        }
        self.upload_mesh(geometry, visible.len())?;
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
            }
            if let Some(surface) = surface {
                if index_count > surface.mesh_start as u32 && surface.frame.is_partial() {
                    pass.draw_indexed(surface.mesh_start as u32..index_count, 0, 0..1);
                }
                pass.set_pipeline(&self.surface_pipeline);
                pass.set_bind_group(0, &self.surface_image.as_ref()?.1, &[]);
                pass.draw(0..3, 0..1);
                if surface.mesh_start > 0 {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.bindings, &[]);
                    pass.draw_indexed(0..surface.mesh_start as u32, 0, 0..1);
                }
            } else if index_count > 0 {
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
        let mut rgba = packed_rows(&mapped, target.stride as usize, view.width as usize * 4);
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

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector3;

    #[test]
    fn upload_reuse_requires_the_same_unchanged_mesh_and_an_uploaded_prefix() {
        let mut geometry = Mesh::default();
        let uploaded = UploadedMesh {
            source: Arc::downgrade(&geometry),
            index_count: 6,
        };
        assert!(uploaded.covers(&geometry.clone(), 6));
        assert!(uploaded.covers(&geometry, 3));
        assert!(!uploaded.covers(&geometry, 9));
        assert!(!uploaded.covers(&Mesh::default(), 6));
        assert!(Arc::get_mut(&mut geometry).is_none());
        Arc::make_mut(&mut geometry).indices.push(0);
        assert!(!uploaded.covers(&geometry, 1));
        assert!(uploaded.source.upgrade().is_none());

        let uploaded = UploadedMesh {
            source: Arc::downgrade(&geometry),
            index_count: 1,
        };
        drop(geometry);
        assert!(
            uploaded.source.upgrade().is_none(),
            "uploads don't retain CPU geometry"
        );
        assert!(!uploaded.covers(&Mesh::default(), 0));
    }

    #[test]
    #[ignore = "requires a GPU; does not use the CPU fallback"]
    fn retained_uploads_follow_camera_and_mesh_edits() {
        let mut geometry = Mesh::new(Geometry {
            vertices: [[-0.8, -0.8, 0.0], [0.8, -0.8, 0.0], [0.0, 0.8, 0.0]]
                .map(|p| super::super::Vertex {
                    position: Vector3::from(p),
                    color: [1.0, 0.0, 0.0],
                    normal: Default::default(),
                })
                .into(),
            indices: vec![0, 1, 2],
        });
        let mut view = View {
            model_to_view: nalgebra::Matrix4::identity(),
            projection: [1.0, 1.0, -0.5, 0.5],
            width: 40,
            height: 32,
        };
        let mut renderer = Renderer::new(pollster::block_on(Gpu::init_basic()).unwrap());
        let first = renderer.render(&geometry, &view, None).unwrap();
        assert_eq!(first, renderer.render(&geometry, &view, None).unwrap());
        view.projection[0] = 0.5;
        let camera = renderer.render(&geometry, &view, None).unwrap();
        assert_ne!(first, camera);
        assert!(renderer.uploaded.as_ref().unwrap().covers(&geometry, 3));

        for vertex in &mut Arc::make_mut(&mut geometry).vertices {
            vertex.color = [0.0, 1.0, 0.0];
        }
        assert!(!renderer.uploaded.as_ref().unwrap().covers(&geometry, 3));
        let edited = renderer.render(&geometry, &view, None).unwrap();
        assert!(edited.chunks_exact(4).any(|p| p[1] > 0));
        assert!(edited.chunks_exact(4).all(|p| p[0] == 0));
        renderer.uploaded = None;
        assert_eq!(edited, renderer.render(&geometry, &view, None).unwrap());

        Arc::make_mut(&mut geometry).indices.clear();
        assert!(
            renderer
                .render(&geometry, &view, None)
                .unwrap()
                .iter()
                .all(|v| *v == 0)
        );
    }

    fn iterator_vertices(vertices: &[super::super::Vertex]) -> Vec<u8> {
        vertices
            .iter()
            .flat_map(|v| {
                v.position
                    .iter()
                    .copied()
                    .chain(v.color)
                    .flat_map(f32::to_ne_bytes)
                    .chain(v.normal.0.map(|n| n as u8))
            })
            .collect()
    }

    fn iterator_rows(mapped: &[u8], stride: usize, row_bytes: usize) -> Vec<u8> {
        mapped
            .chunks_exact(stride)
            .flat_map(|row| row[..row_bytes].iter().copied())
            .collect()
    }

    #[test]
    fn packed_mesh_data_preserves_shader_layout_and_all_bits() {
        let vertices = [
            super::super::Vertex {
                position: Vector3::new(1.0, -2.0, 3.0),
                color: [0.25, 0.5, 1.0],
                normal: super::super::Normal::new(-Vector3::z()),
            },
            super::super::Vertex {
                position: Vector3::new(-0.0, f32::INFINITY, f32::from_bits(0x7fc01234)),
                color: [1.0, 0.0, 0.0],
                normal: super::super::Normal::default(),
            },
        ];
        let bytes = vertex_bytes(&vertices);
        assert_eq!(bytes.len(), vertices.len() * 28);
        assert_eq!(bytes, iterator_vertices(&vertices));
        assert_eq!(&bytes[..4], &1.0_f32.to_ne_bytes());
        assert_eq!(&bytes[12..16], &0.25_f32.to_ne_bytes());
        assert_eq!(&bytes[24..28], &[0, 0, 129, 0]);
        assert!(vertex_bytes(&[]).is_empty());
    }

    #[test]
    fn readback_rows_remove_padding_without_modifying_pixels() {
        let mapped: Vec<_> = (0..32).collect();
        for row_bytes in [4, 12, 16] {
            assert_eq!(
                packed_rows(&mapped, 16, row_bytes),
                iterator_rows(&mapped, 16, row_bytes)
            );
            assert_eq!(packed_rows(&mapped, 16, row_bytes).len(), 2 * row_bytes);
        }
        assert!(packed_rows(&[], 256, 4).is_empty());
    }

    #[test]
    #[ignore = "CPU packing benchmark; no GPU or application launch"]
    fn mesh_gpu_packing_profile() {
        use std::{hint::black_box, time::Instant};
        let vertices: Vec<_> = (0..500_000)
            .map(|i| super::super::Vertex {
                position: Vector3::new(i as f32 * 0.001, 1.0, -1.0),
                color: [0.2, 0.5, 0.8],
                normal: super::super::Normal::default(),
            })
            .collect();
        let mapped = vec![127; 5120 * 1600];
        for trial in 0..6 {
            for bulk in [trial % 2 == 0, trial % 2 != 0] {
                let start = Instant::now();
                let v = if bulk {
                    vertex_bytes(black_box(&vertices))
                } else {
                    iterator_vertices(black_box(&vertices))
                };
                let v_time = start.elapsed();
                let start = Instant::now();
                let r = if bulk {
                    packed_rows(black_box(&mapped), 5120, 5000)
                } else {
                    iterator_rows(black_box(&mapped), 5120, 5000)
                };
                let r_time = start.elapsed();
                black_box((&v, &r));
                eprintln!("trial {trial}, bulk={bulk}: vertices {v_time:?}, readback {r_time:?}");
            }
        }
    }
}
