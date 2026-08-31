pub mod render;
pub mod theme;
pub mod window;

use std::ffi::CString;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use egui_glow::Painter;
use egui_winit::State as EguiWinitState;
use glow::HasContext;
use glutin::config::ConfigTemplateBuilder;
use glutin::context::ContextAttributesBuilder;
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::config::OverlayConfig;
use crate::game::state::SharedGameState;
use crate::overlay::render::{draw_overlay, OverlayAction};
use crate::overlay::window::{
    apply_stream_proof_styles, set_stream_proof, set_window_visible, SystemTray,
};

pub struct OverlayApp;

impl OverlayApp {
    pub fn run(config: OverlayConfig, shared: Arc<SharedGameState>) {
        let event_loop = EventLoop::new().expect("event loop");
        let mut app = OverlayHandler::new(config, shared);
        event_loop.run_app(&mut app).expect("event loop failed");
    }
}

struct GlBundle {
    window: Window,
    surface: Surface<WindowSurface>,
    context: glutin::context::PossiblyCurrentContext,
    gl: Arc<glow::Context>,
    painter: Painter,
    egui_winit: EguiWinitState,
    _tray: Option<SystemTray>,
}

struct OverlayHandler {
    config: OverlayConfig,
    shared: Arc<SharedGameState>,
    gl: Option<GlBundle>,
    last_frame: Instant,
    last_generation: u64,
    window_visible: bool,
    toggle_key_down: bool,
    vk_code: i32,
    is_editing_key: bool,
    key_buffer: String,
    radar_state: crate::overlay::render::RadarState,
}

impl OverlayHandler {
    fn new(config: OverlayConfig, shared: Arc<SharedGameState>) -> Self {
        let vk_code = config.toggle_key_vk();
        let key_buffer = config.toggle_key.clone();
        Self {
            config,
            shared,
            gl: None,
            last_frame: Instant::now(),
            last_generation: 0,
            window_visible: true,
            toggle_key_down: false,
            vk_code,
            is_editing_key: false,
            key_buffer,
            radar_state: crate::overlay::render::RadarState::default(),
        }
    }

    fn check_hotkey(&mut self) {
        if self.is_editing_key {
            return;
        }
        let key_state =
            unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(self.vk_code) };
        let is_down = (key_state as u16 & 0x8000) != 0;
        if is_down && !self.toggle_key_down {
            self.toggle_key_down = true;
            self.window_visible = !self.window_visible;
            if let Some(bundle) = self.gl.as_ref() {
                set_window_visible(&bundle.window, self.window_visible);
            }
        } else if !is_down && self.toggle_key_down {
            self.toggle_key_down = false;
        }
    }

    fn save_key_setting(key: &str) {
        // Try updating offsets.toml if it exists
        if let Ok(content) = std::fs::read_to_string("offsets.toml") {
            let lines: Vec<String> = content
                .lines()
                .map(|line| {
                    if line.trim_start().starts_with("toggle_key") {
                        format!("toggle_key = \"{key}\"")
                    } else {
                        line.to_string()
                    }
                })
                .collect();
            let _ = std::fs::write("offsets.toml", lines.join("\r\n"));
        }
    }

    fn init(&mut self, event_loop: &ActiveEventLoop) {
        if self.gl.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("Among Us Overlay")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
            ))
            .with_position(winit::dpi::LogicalPosition::new(
                self.config.position_x,
                self.config.position_y,
            ))
            .with_transparent(true)
            .with_resizable(true)
            .with_decorations(false);

        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(true);

        let (window, gl_config) = DisplayBuilder::new()
            .with_window_attributes(Some(attrs))
            .build(event_loop, template, |mut configs| {
                configs.next().expect("gl config")
            })
            .expect("window/display");

        let window = window.expect("window");
        apply_stream_proof_styles(&window);

        let tray = SystemTray::create(&window);

        let raw = window.window_handle().expect("handle").as_raw();
        let context_attrs = ContextAttributesBuilder::new().build(Some(raw));

        let not_current = unsafe {
            gl_config
                .display()
                .create_context(&gl_config, &context_attrs)
                .expect("context")
        };

        let (width, height): (u32, u32) = window.inner_size().into();
        let surface_attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw,
            NonZeroU32::new(width.max(1)).unwrap(),
            NonZeroU32::new(height.max(1)).unwrap(),
        );

        let surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &surface_attrs)
                .expect("surface")
        };

        let context = not_current.make_current(&surface).expect("make current");

        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|symbol| {
                let name = CString::new(symbol).expect("gl symbol");
                gl_config.display().get_proc_address(name.as_c_str()) as *const _
            })
        });

        unsafe {
            surface
                .set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
                .ok();
            gl.disable(glow::DEPTH_TEST);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        }

        let painter = Painter::new(gl.clone(), "", None, true).expect("painter");
        let egui_winit = EguiWinitState::new(
            egui::Context::default(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        self.gl = Some(GlBundle {
            window,
            surface,
            context,
            gl,
            painter,
            egui_winit,
            _tray: tray,
        });
    }

    fn render(&mut self) {
        if !self.window_visible {
            return;
        }

        let bundle = match self.gl.as_mut() {
            Some(g) => g,
            None => return,
        };

        let window = &bundle.window;
        let raw_input = bundle.egui_winit.take_egui_input(window);
        let game = self.shared.snapshot();
        let (full_output, action) = draw_overlay(
            bundle.egui_winit.egui_ctx(),
            raw_input,
            &game,
            &self.config.toggle_key,
            &mut self.is_editing_key,
            &mut self.key_buffer,
            &mut self.radar_state,
        );

        match action {
            OverlayAction::Close => {
                std::process::exit(0);
            }
            OverlayAction::DragWindow => {
                let _ = window.drag_window();
            }
            OverlayAction::ToggleStreamProof => {
                let new_val = self.shared.toggle_stream_proof();
                set_stream_proof(window, new_val);
            }
            OverlayAction::ToggleLogKills => {
                self.shared.toggle_log_kills();
            }
            OverlayAction::ToggleLogGameState => {
                self.shared.toggle_log_game_state();
            }
            OverlayAction::ToggleLogPlayerList => {
                self.shared.toggle_log_player_list();
            }
            OverlayAction::ResizeWindow => {
                let _ = window.drag_resize_window(winit::window::ResizeDirection::SouthEast);
            }
            OverlayAction::ExportMatchLog => {
                let _ = self.shared.export_match_log();
            }
            OverlayAction::ClearLogs => {
                self.shared.clear_logs();
            }
            OverlayAction::ChangeToggleKey(new_key) => {
                self.config.toggle_key = new_key.clone();
                self.vk_code = self.config.toggle_key_vk();
                Self::save_key_setting(&new_key);
            }
            OverlayAction::None => {}
        }

        bundle
            .egui_winit
            .handle_platform_output(window, full_output.platform_output);

        let clipped = bundle
            .egui_winit
            .egui_ctx()
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let size = window.inner_size();
        bundle.context.make_current(&bundle.surface).ok();

        unsafe {
            bundle
                .gl
                .viewport(0, 0, size.width as i32, size.height as i32);
            bundle.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            bundle.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        bundle.painter.paint_and_update_textures(
            [size.width, size.height],
            full_output.pixels_per_point,
            &clipped,
            &full_output.textures_delta,
        );

        bundle.surface.swap_buffers(&bundle.context).ok();
    }
}

impl ApplicationHandler for OverlayHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        self.init(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let repaint = if let Some(bundle) = self.gl.as_mut() {
            let window = &bundle.window;
            let res = bundle.egui_winit.on_window_event(window, &event);
            res.repaint || matches!(event, WindowEvent::RedrawRequested)
        } else {
            false
        };

        if repaint && self.window_visible {
            self.render();
        }

        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
            return;
        }

        if let WindowEvent::Resized(size) = event {
            if size.width > 0 && size.height > 0 {
                if let Some(bundle) = self.gl.as_mut() {
                    bundle.context.make_current(&bundle.surface).ok();
                    bundle.surface.resize(
                        &bundle.context,
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    );
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        self.check_hotkey();

        if !self.window_visible {
            std::thread::sleep(std::time::Duration::from_millis(20));
            return;
        }

        let current_gen = self.shared.generation();
        let gen_changed = current_gen != self.last_generation;

        if gen_changed || self.last_frame.elapsed().as_millis() >= 16 {
            self.last_generation = current_gen;
            if let Some(bundle) = self.gl.as_ref() {
                bundle.window.request_redraw();
            }
            self.last_frame = Instant::now();
        }
    }
}
