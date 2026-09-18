mod canvas;
pub use canvas::{Layer, SplitCanvas};

use crate::VelloCanvas;
use puri::draw::{CanvasSink, Shape};
use std::collections::{HashMap, HashSet};
use vello::{
    AaConfig, RenderParams, Renderer, RendererOptions, Scene,
    kurbo::{Affine, Rect},
    peniko::{Brush, Color, ImageAlphaType, ImageData, ImageFormat},
    wgpu,
};

pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Gpu<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
}

impl Gpu<'_> {
    pub fn texture(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Puri image compositor"),
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

    fn upload(&self, image: &ImageData) -> Result<wgpu::Texture, Error> {
        let limit = self.device.limits().max_texture_dimension_2d;
        if image.width == 0
            || image.height == 0
            || image.width > limit
            || image.height > limit
            || image
                .format
                .size_in_bytes(image.width, image.height)
                .is_none_or(|size| image.data.data().len() < size)
        {
            return Err(Error::Image("invalid image dimensions or byte length"));
        }
        let format = match image.format {
            ImageFormat::Rgba8 => wgpu::TextureFormat::Rgba8Unorm,
            ImageFormat::Bgra8 => wgpu::TextureFormat::Bgra8Unorm,
            _ => return Err(Error::Image("unsupported image format")),
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Puri uploaded image"),
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
        self.queue.write_texture(
            texture.as_image_copy(),
            image.data.data(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width * 4),
                rows_per_image: Some(image.height),
            },
            texture.size(),
        );
        Ok(texture)
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
}

#[derive(Debug, Default)]
pub struct Counts {
    pub vector_passes: usize,
    pub mask_passes: usize,
    pub blends: usize,
    pub uploads: usize,
    pub clip_depth: usize,
}

pub struct Output {
    pub texture: wgpu::Texture,
    pub alpha_type: ImageAlphaType,
}

#[derive(Debug)]
pub enum Error {
    Vello(vello::Error),
    Image(&'static str),
}

impl From<vello::Error> for Error {
    fn from(value: vello::Error) -> Self {
        Self::Vello(value)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vello(error) => error.fmt(f),
            Self::Image(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

pub struct Compositor {
    renderer: Renderer,
    pipeline: wgpu::RenderPipeline,
    white: wgpu::Texture,
    pub counts: Counts,
}

/// Per-output GPU resources, independent of widget or document identity.
#[derive(Default)]
pub struct Resources {
    output: Option<wgpu::Texture>,
    mask: Option<wgpu::Texture>,
    groups: Vec<wgpu::Texture>,
    images: HashMap<ImageKey, wgpu::Texture>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ImageKey {
    blob: u64,
    width: u32,
    height: u32,
    format: u8,
}

impl ImageKey {
    fn new(image: &ImageData) -> Self {
        Self {
            blob: image.data.id(),
            width: image.width,
            height: image.height,
            format: image.format as u8,
        }
    }
}

impl Resources {
    pub fn uploaded_images(&self) -> usize {
        self.images.len()
    }

    pub fn scratch_textures(&self) -> usize {
        usize::from(self.output.is_some()) + usize::from(self.mask.is_some()) + self.groups.len()
    }
}

fn image_keys(layers: &[Layer], keys: &mut HashSet<ImageKey>) {
    for layer in layers {
        match layer {
            Layer::Image(image, _) => {
                keys.insert(ImageKey::new(image));
            }
            Layer::Clip(_, _, children) => image_keys(children, keys),
            _ => {}
        }
    }
}

fn texture_at_size(
    slot: &mut Option<wgpu::Texture>,
    gpu: &Gpu<'_>,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    slot.get_or_insert_with(|| gpu.texture(width, height))
        .clone()
}

impl Compositor {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self, vello::Error> {
        let gpu = Gpu { device, queue };
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Puri image compositor"),
                source: wgpu::ShaderSource::Wgsl(include_str!("compositor/blend.wgsl").into()),
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("premultiplied source-over"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        let white = gpu.texture(1, 1);
        gpu.clear(&white, wgpu::Color::WHITE);
        Ok(Self {
            renderer: Renderer::new(gpu.device, RendererOptions::default())?,
            pipeline,
            white,
            counts: Counts::default(),
        })
    }

    fn vello(
        &mut self,
        gpu: &Gpu<'_>,
        scene: &Scene,
        texture: &wgpu::Texture,
        base_color: Color,
    ) -> Result<(), vello::Error> {
        self.renderer.render_to_texture(
            &gpu.device,
            &gpu.queue,
            scene,
            &texture.create_view(&Default::default()),
            &RenderParams {
                base_color,
                width: texture.width(),
                height: texture.height(),
                antialiasing_method: AaConfig::Msaa16,
            },
        )
    }

    fn mask(
        &mut self,
        gpu: &Gpu<'_>,
        resources: &mut Resources,
        shape: Shape,
        transform: Affine,
        target: &wgpu::Texture,
    ) -> Result<wgpu::Texture, vello::Error> {
        let mut scene = Scene::new();
        VelloCanvas(&mut scene).fill_shape(shape, Brush::Solid(Color::WHITE), transform);
        let mask = texture_at_size(&mut resources.mask, gpu, target.width(), target.height());
        self.vello(gpu, &scene, &mask, Color::TRANSPARENT)?;
        self.counts.mask_passes += 1;
        Ok(mask)
    }

    fn blend(
        &mut self,
        gpu: &Gpu<'_>,
        source: &wgpu::Texture,
        mask: Option<&wgpu::Texture>,
        transform: Affine,
        premultiplied: bool,
        target: &wgpu::Texture,
        scissor: Option<[u32; 4]>,
    ) {
        let [a, b, c, d, e, f] = transform.inverse().as_coeffs().map(|v| v as f32);
        let values = [
            a,
            c,
            e,
            0.0,
            b,
            d,
            f,
            0.0,
            u8::from(premultiplied) as f32,
            u8::from(mask.is_some()) as f32,
            target.width() as f32,
            target.height() as f32,
        ];
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        let mut bytes = [0; 48];
        for (chunk, value) in bytes.chunks_exact_mut(4).zip(values) {
            chunk.copy_from_slice(&value.to_ne_bytes());
        }
        uniform
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(&bytes);
        uniform.unmap();
        let bindings = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &source.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &mask.unwrap_or(&self.white).create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.create_view(&Default::default()),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            if let Some([x, y, width, height]) = scissor {
                pass.set_scissor_rect(x, y, width, height);
            }
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
        self.counts.blends += 1;
    }

    /// The supplied RGBA8 texture is Vello's scratch/output target. Mixed frames
    /// return a separate premultiplied target; vector-only frames render directly.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layers: &[Layer],
        resources: &mut Resources,
        vector: &wgpu::Texture,
        base_color: Color,
    ) -> Result<Output, Error> {
        let gpu = Gpu { device, queue };
        self.counts = Counts::default();
        let mut keys = HashSet::new();
        image_keys(layers, &mut keys);
        resources.images.retain(|key, _| keys.contains(key));
        if let [Layer::Vector(scene)] = layers {
            resources.output = None;
            resources.mask = None;
            resources.groups.clear();
            self.vello(&gpu, scene, vector, base_color)?;
            self.counts.vector_passes = 1;
            Ok(Output {
                texture: vector.clone(),
                alpha_type: ImageAlphaType::Alpha,
            })
        } else {
            if resources
                .output
                .as_ref()
                .is_some_and(|t| t.size() != vector.size())
            {
                resources.output = None;
                resources.mask = None;
                resources.groups.clear();
            }
            let target =
                texture_at_size(&mut resources.output, &gpu, vector.width(), vector.height());
            let [r, g, b, a] = base_color.components.map(f64::from);
            let layers = if let [Layer::Vector(scene), rest @ ..] = layers
                && a == 1.0
            {
                // An opaque background makes Vello's straight-alpha output
                // already premultiplied; no intermediate copy is necessary.
                self.vello(&gpu, scene, &target, base_color)?;
                self.counts.vector_passes += 1;
                rest
            } else {
                gpu.clear(
                    &target,
                    wgpu::Color {
                        r: r * a,
                        g: g * a,
                        b: b * a,
                        a,
                    },
                );
                layers
            };
            self.render_group(&gpu, layers, resources, vector, &target, 0, None)?;
            resources.groups.truncate(self.counts.clip_depth);
            if self.counts.mask_passes == 0 {
                resources.mask = None;
            }
            Ok(Output {
                texture: target,
                alpha_type: ImageAlphaType::AlphaPremultiplied,
            })
        }
    }

    fn render_group(
        &mut self,
        gpu: &Gpu<'_>,
        layers: &[Layer],
        resources: &mut Resources,
        vector: &wgpu::Texture,
        target: &wgpu::Texture,
        depth: usize,
        scissor: Option<[u32; 4]>,
    ) -> Result<(), Error> {
        for layer in layers {
            match layer {
                Layer::Vector(scene) => {
                    self.vello(gpu, scene, vector, Color::TRANSPARENT)?;
                    self.counts.vector_passes += 1;
                    self.blend(gpu, &vector, None, Affine::IDENTITY, false, target, scissor);
                }
                Layer::Image(image, transform) => {
                    let source = match resources.images.entry(ImageKey::new(image)) {
                        std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            let texture = gpu.upload(image)?;
                            self.counts.uploads += 1;
                            entry.insert(texture).clone()
                        }
                    };
                    self.image(
                        gpu,
                        resources,
                        &source,
                        *transform,
                        matches!(image.alpha_type, ImageAlphaType::AlphaPremultiplied),
                        target,
                        scissor,
                    )?;
                }
                Layer::Texture(source, transform) => {
                    self.image(gpu, resources, source, *transform, true, target, scissor)?;
                }
                Layer::Clip(shape, transform, children) => {
                    if let Some(rect) = integer_rect(shape, *transform, target, scissor) {
                        if rect[2] > 0 && rect[3] > 0 {
                            self.render_group(
                                gpu,
                                children,
                                resources,
                                vector,
                                target,
                                depth,
                                Some(rect),
                            )?;
                        }
                    } else {
                        self.counts.clip_depth = self.counts.clip_depth.max(depth + 1);
                        if resources.groups.len() <= depth {
                            resources
                                .groups
                                .push(gpu.texture(target.width(), target.height()));
                        }
                        let child = resources.groups[depth].clone();
                        gpu.clear(&child, wgpu::Color::TRANSPARENT);
                        self.render_group(
                            gpu,
                            children,
                            resources,
                            vector,
                            &child,
                            depth + 1,
                            None,
                        )?;
                        let mask = self.mask(gpu, resources, shape.clone(), *transform, target)?;
                        self.blend(
                            gpu,
                            &child,
                            Some(&mask),
                            Affine::IDENTITY,
                            true,
                            target,
                            scissor,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn image(
        &mut self,
        gpu: &Gpu<'_>,
        resources: &mut Resources,
        source: &wgpu::Texture,
        transform: Affine,
        premultiplied: bool,
        target: &wgpu::Texture,
        scissor: Option<[u32; 4]>,
    ) -> Result<(), vello::Error> {
        let shape = Shape::Rect(Rect::new(
            0.0,
            0.0,
            source.width() as f64,
            source.height() as f64,
        ));
        if let Some(rect) = integer_rect(&shape, transform, target, scissor) {
            if rect[2] > 0 && rect[3] > 0 {
                self.blend(
                    gpu,
                    source,
                    None,
                    transform,
                    premultiplied,
                    target,
                    Some(rect),
                );
            }
        } else {
            let mask = self.mask(gpu, resources, shape, transform, target)?;
            self.blend(
                gpu,
                source,
                Some(&mask),
                transform,
                premultiplied,
                target,
                scissor,
            );
        }
        Ok(())
    }
}

fn integer_rect(
    shape: &Shape,
    transform: Affine,
    target: &wgpu::Texture,
    enclosing: Option<[u32; 4]>,
) -> Option<[u32; 4]> {
    match shape {
        Shape::Rect(rect) if transform.as_coeffs()[1] == 0.0 && transform.as_coeffs()[2] == 0.0 => {
            let rect = transform.transform_rect_bbox(*rect);
            if [rect.x0, rect.y0, rect.x1, rect.y1]
                .iter()
                .all(|v| v.is_finite() && v.fract() == 0.0)
            {
                let [x, y, width, height] =
                    enclosing.unwrap_or([0, 0, target.width(), target.height()]);
                let x0 = rect.x0.clamp(x as f64, (x + width) as f64) as u32;
                let y0 = rect.y0.clamp(y as f64, (y + height) as f64) as u32;
                let x1 = rect.x1.clamp(x0 as f64, (x + width) as f64) as u32;
                let y1 = rect.y1.clamp(y0 as f64, (y + height) as f64) as u32;
                Some([x0, y0, x1 - x0, y1 - y0])
            } else {
                None
            }
        }
        _ => None,
    }
}
