use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowId},
};
use winit::error::ExternalError;
use winit::event::{DeviceEvent, DeviceId, KeyEvent, RawKeyEvent};
use winit::window::CursorGrabMode;
use crate::controls::Controls;
use crate::game_state::GameState;
use crate::renderer::Renderer;
use crate::settings::FullscreenMode;

mod settings;

mod renderer;

mod game_state;

mod controls;

pub(crate) fn attempt_lock_cursor(
    window: &Window,
    grab: bool,
) -> Result<(), ExternalError> {
    window.set_cursor_visible(!grab);
    
    let grab_result = match grab {
        false => window.set_cursor_grab(CursorGrabMode::None),
        true => window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_e| window.set_cursor_grab(CursorGrabMode::Confined)),
    };
    
    if let Err(err) = grab_result {
        let err_desc = match grab {
            true => "grab",
            false => "ungrab",
        };

        tracing::error!("Unable to {} cursor: {}", err_desc, err);
        return Err(err)
    }
    
    Ok(())
}


struct App {
    controls: Controls,
    game_state: GameState,
    cursor_locked: bool,
    render_state: Option<Renderer>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let settings = settings::load();

        let current_settings = settings.load();

        let attrib = Window::default_attributes()
            .with_title(&*current_settings.game_title)
            .with_window_icon(settings::load_icon())
            .with_fullscreen(match current_settings.fullscreen {
                FullscreenMode::On => todo!(),
                FullscreenMode::Off => None,
                FullscreenMode::Borderless => Some(Fullscreen::Borderless(None)),
            });


        let window = event_loop
            .create_window(attrib)
            .unwrap();

        let window = Arc::new(window);
        let state = pollster::block_on(Renderer::new(Arc::clone(&window), settings));
        
        self.render_state = Some(state);
        let _ = attempt_lock_cursor(&window, self.cursor_locked);
        
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let state = self.render_state.as_mut().unwrap();
        assert_eq!(state.window().id(), id);
        
        match event {
            WindowEvent::Focused(focus) => {
                self.cursor_locked = focus;
                let _ = attempt_lock_cursor(state.window(), focus);
                if !focus { 
                    self.controls.lost_focus();
                }
            }
            WindowEvent::KeyboardInput { event: KeyEvent {
                physical_key,
                state,
                ..
            }, .. } => {
                self.controls.update(&DeviceEvent::Key(RawKeyEvent { physical_key, state }))
            }
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                tracing::info!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.game_state.frame_update(&self.controls);
                state.render(&self.game_state);
                self.controls.new_frame();
                state.window().request_redraw();

            },
            WindowEvent::Resized(size) => {
                // Reconfigures the size of the surface. We do not re-render
                // here as this event is always followed up by a redrawn request.
                state.resize(size);
            }
            _ => ()
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        self.controls.update(&event)
    }
}

fn run_app() {
    let event_loop = EventLoop::new().unwrap();

    // When the current loop iteration finishes, immediately begin a new
    // iteration regardless of whether new events are available to
    // process.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        controls: Controls::default(),
        game_state: GameState::new(),
        cursor_locked: true,
        render_state: None,
    };
    event_loop.run_app(&mut app).unwrap();
}

fn setup_logging() {
    tracing_subscriber::fmt::init();
}

pub fn run() {
    setup_logging();
    run_app();
}