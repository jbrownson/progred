use puri::mesh::{DepthImage, Geometry, Mesh, Surface, Vertex, View};
use std::sync::{Arc, Weak};
use vello::wgpu;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SAMPLES: u32 = 4;

fn vertex_bytes(vertices: &[Vertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() * 28);
    for vertex in vertices {
        for component in vertex.position.iter().chain(&vertex.color) {
            bytes.extend_from_slice(&component.to_ne_bytes());
        }
        bytes.extend(vertex.normal.0.map(|n| n as u8));
    }
    bytes
}

struct StreamBuffer {
    buffer: wgpu::Buffer,
    usage: wgpu::BufferUsages,
}

struct UploadedMesh {
    source: Weak<Geometry>,
    index_count: usize,
}

struct UploadedSurface {
    image: u64,
    size: [u32; 2],
    depth: Weak<[f32]>,
    bindings: wgpu::BindGroup,
}

impl UploadedSurface {
    fn matches(&self, frame: &DepthImage) -> bool {
        self.image == frame.image.data.id()
            && self.size == [frame.image.width, frame.image.height]
            && self.depth.ptr_eq(&Arc::downgrade(&frame.depth))
    }
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
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
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
        Self {
            output,
            color,
            depth,
        }
    }
}

/// Device-owned triangle renderer. Each render submits work without waiting and
/// returns premultiplied RGBA. The texture is scratch storage: consume it on this
/// queue before the next render, or copy it if the result must outlive that call.
pub struct Renderer {
    gpu: Gpu,
    pipeline: wgpu::RenderPipeline,
    vertices: StreamBuffer,
    indices: StreamBuffer,
    uploaded: Option<UploadedMesh>,
    uniform: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    target: Option<Target>,
    surface_pipeline: wgpu::RenderPipeline,
    surface_image: Option<UploadedSurface>,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let gpu = Gpu {
            device: device.clone(),
            queue: queue.clone(),
        };
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

    fn upload_surface(&mut self, frame: &DepthImage) -> Option<()> {
        if self
            .surface_image
            .as_ref()
            .is_some_and(|upload| upload.matches(frame))
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
        self.surface_image = Some(UploadedSurface {
            image: image.data.id(),
            size: [image.width, image.height],
            depth: Arc::downgrade(&frame.depth),
            bindings: self
                .gpu
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
        });
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
            self.vertices.write(
                &self.gpu,
                &vertex_bytes(geometry.vertices.get(..vertex_count)?),
            )?;
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

    pub fn render(
        &mut self,
        geometry: &Mesh,
        view: &View,
        surface: Option<&Surface>,
    ) -> Option<wgpu::Texture> {
        let limits = self.gpu.device.limits();
        if view.width == 0
            || view.height == 0
            || view.width > limits.max_texture_dimension_2d
            || view.height > limits.max_texture_dimension_2d
        {
            return None;
        }
        let visible = puri::mesh::visible_indices(geometry, surface)?;
        let index_count = u32::try_from(visible.len()).ok()?;
        if let Some(surface) = surface {
            self.upload_surface(&surface.frame)?;
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
                pass.set_bind_group(0, &self.surface_image.as_ref()?.bindings, &[]);
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
        self.gpu.queue.submit([encoder.finish()]);
        Some(target.output.clone())
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

    fn iterator_vertices(vertices: &[Vertex]) -> Vec<u8> {
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

    #[test]
    fn packed_mesh_data_preserves_shader_layout_and_all_bits() {
        let vertices = [
            Vertex {
                position: Vector3::new(1.0, -2.0, 3.0),
                color: [0.25, 0.5, 1.0],
                normal: puri::mesh::Normal::new(-Vector3::z()),
            },
            Vertex {
                position: Vector3::new(-0.0, f32::INFINITY, f32::from_bits(0x7fc01234)),
                color: [1.0, 0.0, 0.0],
                normal: puri::mesh::Normal::default(),
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
}
