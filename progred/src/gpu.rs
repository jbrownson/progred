// Adapted from Vello's `util::RenderContext`.
// Copyright 2022 the Vello Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use vello::wgpu::SurfaceTarget;
use vello::wgpu::{
    self, Adapter, Device, Instance, Queue, Surface, SurfaceConfiguration, Texture, TextureFormat,
    TextureView, util::TextureBlitter,
};

pub(crate) struct RenderContext {
    pub(crate) instance: Instance,
    pub(crate) devices: Vec<DeviceHandle>,
}

pub(crate) struct DeviceHandle {
    adapter: Adapter,
    pub(crate) device: Device,
    pub(crate) queue: Queue,
}

pub(crate) struct RenderSurface<'a> {
    pub(crate) surface: Surface<'a>,
    pub(crate) config: SurfaceConfiguration,
    pub(crate) dev_id: usize,
    pub(crate) target_view: TextureView,
    pub(crate) blitter: TextureBlitter,
    target_texture: Texture,
}

impl RenderContext {
    pub(crate) fn new() -> Self {
        Self {
            instance: Instance::new(wgpu::InstanceDescriptor {
                display: None,
                backends: wgpu::Backends::from_env().unwrap_or_default(),
                flags: wgpu::InstanceFlags::from_build_config().with_env(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                backend_options: wgpu::BackendOptions::from_env_or_default(),
            }),
            devices: Vec::new(),
        }
    }

    pub(crate) async fn create_surface<'a>(
        &mut self,
        window: impl Into<SurfaceTarget<'a>>,
        width: u32,
        height: u32,
        present_mode: wgpu::PresentMode,
    ) -> Result<RenderSurface<'a>, vello::Error> {
        self.create_render_surface(
            self.instance.create_surface(window.into())?,
            width,
            height,
            present_mode,
        )
        .await
    }

    pub(crate) async fn create_render_surface<'a>(
        &mut self,
        surface: Surface<'a>,
        width: u32,
        height: u32,
        present_mode: wgpu::PresentMode,
    ) -> Result<RenderSurface<'a>, vello::Error> {
        let dev_id = self
            .device(Some(&surface))
            .await
            .ok_or(vello::Error::NoCompatibleDevice)?;
        let device = &self.devices[dev_id];
        let format = surface
            .get_capabilities(&device.adapter)
            .formats
            .into_iter()
            .find(|format| {
                matches!(
                    format,
                    TextureFormat::Rgba8Unorm | TextureFormat::Bgra8Unorm
                )
            })
            .ok_or(vello::Error::UnsupportedSurfaceFormat)?;
        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::new(),
        };
        let (target_texture, target_view) = create_targets(width, height, &device.device);
        let surface = RenderSurface {
            surface,
            config,
            dev_id,
            target_texture,
            target_view,
            blitter: TextureBlitter::new(&device.device, format),
        };
        self.configure_surface(&surface);
        Ok(surface)
    }

    pub(crate) fn resize_surface(&self, surface: &mut RenderSurface<'_>, width: u32, height: u32) {
        let (target_texture, target_view) =
            create_targets(width, height, &self.devices[surface.dev_id].device);
        surface.target_texture = target_texture;
        surface.target_view = target_view;
        surface.config.width = width;
        surface.config.height = height;
        self.configure_surface(surface);
    }

    pub(crate) fn configure_surface(&self, surface: &RenderSurface<'_>) {
        surface
            .surface
            .configure(&self.devices[surface.dev_id].device, &surface.config);
    }

    async fn device(&mut self, surface: Option<&Surface<'_>>) -> Option<usize> {
        let compatible = surface
            .and_then(|surface| {
                self.devices
                    .iter()
                    .position(|device| device.adapter.is_surface_supported(surface))
            })
            .or_else(|| (surface.is_none() && !self.devices.is_empty()).then_some(0));
        match compatible {
            Some(device) => Some(device),
            None => self.new_device(surface).await,
        }
    }

    async fn new_device(&mut self, surface: Option<&Surface<'_>>) -> Option<usize> {
        let adapter = wgpu::util::initialize_adapter_from_env_or_default(&self.instance, surface)
            .await
            .ok()?;
        let required_features =
            adapter.features() & (wgpu::Features::CLEAR_TEXTURE | wgpu::Features::PIPELINE_CACHE);
        let required_limits = wgpu::Limits::default().or_worse_values_from(&adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits,
                ..Default::default()
            })
            .await
            .ok()?;
        self.devices.push(DeviceHandle {
            adapter,
            device,
            queue,
        });
        Some(self.devices.len() - 1)
    }
}

fn create_targets(width: u32, height: u32, device: &Device) -> (Texture, TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
