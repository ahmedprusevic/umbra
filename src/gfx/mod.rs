//! GPU state and the per-frame render cycle. wgpu 30.

use std::sync::Arc;
use winit::window::Window;

pub mod atlas;
pub mod batch;
pub mod camera;

pub struct Gfx {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pub(crate) sprites: batch::SpritePipeline,
}

#[derive(Debug, thiserror::Error)]
pub enum GfxError {
    #[error("surface creation failed: {0}")]
    Surface(#[from] wgpu::CreateSurfaceError),
    #[error("no suitable GPU adapter: {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),
    #[error("device request failed: {0}")]
    Device(#[from] wgpu::RequestDeviceError),
    #[error("surface has no default config for this adapter")]
    NoSurfaceConfig,
}

impl Gfx {
    /// Blocks (pollster) on adapter/device acquisition. Called once, from
    /// the app loop's `resumed`.
    pub fn new(window: Arc<Window>) -> Result<Gfx, GfxError> {
        let size = window.inner_size();
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        // Arc<Window>: the surface must not outlive the window; sharing the
        // Arc gives the surface a 'static handle while App keeps ownership.
        let surface = instance.create_surface(window)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("umbra"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: Default::default(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
        }))?;
        // get_default_config fills every required field (format, color_space,
        // alpha mode…) correctly for this adapter — never build the struct by hand.
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or(GfxError::NoSurfaceConfig)?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);
        let sprites = batch::SpritePipeline::new(&device, config.format);
        Ok(Gfx { device, queue, surface, config, sprites })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return; // minimized; keep old config
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn viewport(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Begin a frame. `None` means "skip this frame" (occluded, timed out,
    /// or the surface needed reconfiguring); render loops just early-return.
    pub fn frame(&mut self) -> Option<Frame<'_>> {
        use wgpu::CurrentSurfaceTexture as Cst;
        let texture = match self.surface.get_current_texture() {
            Cst::Success(t) | Cst::Suboptimal(t) => t,
            Cst::Lost | Cst::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return None;
            }
            Cst::Timeout | Cst::Occluded => return None,
            Cst::Validation => panic!("surface validation error"),
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        Some(Frame { gfx: self, texture, view, encoder })
    }
}

pub struct Frame<'a> {
    gfx: &'a mut Gfx,
    texture: wgpu::SurfaceTexture,
    view: wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
}

impl Frame<'_> {
    pub fn clear(&mut self, [r, g, b, a]: [f64; 4]) {
        self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }

    pub fn draw_sprites(
        &mut self,
        batch: &batch::SpriteBatch,
        atlas: &atlas::Atlas,
        camera: &camera::Camera,
    ) {
        let viewport = (self.gfx.config.width, self.gfx.config.height);
        let Gfx { device, queue, sprites, .. } = &mut *self.gfx;
        sprites.draw(
            device,
            queue,
            &mut self.encoder,
            &self.view,
            batch,
            atlas,
            camera,
            viewport,
        );
    }

    /// Submit and present. Dropping a Frame without calling finish skips the
    /// frame (nothing is submitted).
    pub fn finish(self) {
        self.gfx.queue.submit([self.encoder.finish()]);
        self.gfx.queue.present(self.texture); // wgpu 30: present lives on Queue
    }
}
