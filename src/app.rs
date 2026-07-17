//! Window + event loop. Games implement [`EngineApp`] and call [`run`].

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowAttributes, WindowId};

use crate::gfx::Gfx;
use crate::input::Input;

pub struct AppConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig { title: "umbra".into(), width: 1280, height: 720 }
    }
}

/// Everything a game touches during one frame.
pub struct FrameCtx<'a> {
    pub gfx: &'a mut Gfx,
    pub input: &'a Input,
    /// Seconds since the previous frame, clamped to 0.25 to survive stalls.
    pub dt: f32,
}

pub trait EngineApp {
    /// Called once, after the window and GPU exist.
    fn init(&mut self, _gfx: &mut Gfx) {}
    /// Called every frame. Do sim ticks and rendering here.
    fn frame(&mut self, ctx: &mut FrameCtx);
}

pub fn run(config: AppConfig, app: impl EngineApp) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let mut runner = Runner {
        config,
        app,
        window: None,
        gfx: None,
        input: Input::default(),
        last_frame: None,
    };
    event_loop.run_app(&mut runner)?;
    Ok(())
}

struct Runner<A: EngineApp> {
    config: AppConfig,
    app: A,
    window: Option<Arc<Window>>,
    gfx: Option<Gfx>,
    input: Input,
    last_frame: Option<Instant>,
}

impl<A: EngineApp> ApplicationHandler for Runner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return; // resumed can fire again on some platforms
        }
        let attrs = WindowAttributes::default()
            .with_title(&self.config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(self.config.width, self.config.height));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let mut gfx = Gfx::new(window.clone()).expect("init gpu");
        self.app.init(&mut gfx);
        self.window = Some(window);
        self.gfx = Some(gfx);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gfx) = &mut self.gfx {
                    gfx.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key: PhysicalKey::Code(code), state, .. },
                ..
            } => self.input.on_key(code, state == ElementState::Pressed),
            WindowEvent::CursorMoved { position, .. } => {
                self.input.on_cursor(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.input.on_mouse_button(button, state == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.input.on_scroll(amount);
            }
            WindowEvent::RedrawRequested => {
                let Some(gfx) = &mut self.gfx else { return };
                let now = Instant::now();
                let dt = self
                    .last_frame
                    .map(|t| (now - t).as_secs_f32().min(0.25))
                    .unwrap_or(0.0);
                self.last_frame = Some(now);
                self.app.frame(&mut FrameCtx { gfx, input: &self.input, dt });
                self.input.end_frame();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw(); // continuous redraw; vsync paces us
        }
    }
}
