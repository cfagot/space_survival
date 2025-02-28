use std::sync::{Arc, Mutex};

use masonry::{app::{AppDriver, MasonryUserEvent, WindowState}, widgets::RootWidget};
use render_mgr::RenderManager;
use starfield_render::StarfieldRenderer;
use winit::{self, application::ApplicationHandler, error::EventLoopError};

#[cfg(target_os = "linux")]
use winit::platform::wayland::ActiveEventLoopExtWayland;

use xilem::{Color, MasonryProxy, WidgetView, Xilem};

mod game_view;
use game_view::{GamePortal, GameView};

mod game;
use game::GameWorld;
use xilem_render::XilemRenderer;

mod game_shapes;

mod render_mgr;
mod starfield_render;
mod xilem_render;

mod vello_ext;

fn app_logic(data: &mut GameState) -> impl WidgetView<GameState> {
    GameView::new(data.clone())
}

pub type GameState = Arc<Mutex<GameWorld>>;

impl ApplicationHandler<MasonryUserEvent> for AppInterface {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.masonry_state.handle_resumed(event_loop);
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        self.masonry_state
            .set_present_mode(vello::wgpu::PresentMode::AutoNoVsync);

        let surface_format = if let WindowState::Rendering { surface, ..} = &self.masonry_state.get_window_state() {
            surface.format
        }
        else {
            // no window, might as well bail
            return;
        };

        if let Some((device, queue)) = self.masonry_state.get_render_device_and_queue() {
            if let WindowState::Rendering { surface, .. } = self.masonry_state.get_window_state() {
                self.render_mgr.setup(device);

                let global_buffer = self.render_mgr.get_global_buffer().unwrap();
                let starfield = StarfieldRenderer::setup(device, queue, global_buffer, surface.format);
                self.render_mgr.add_renderer(Box::new(starfield));

                let global_buffer = self.render_mgr.get_global_buffer().unwrap();
                let xilem_renderer = XilemRenderer::setup(device, queue, global_buffer, surface_format);
                self.render_mgr.add_renderer(Box::new(xilem_renderer));
            }
        }
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        self.render_mgr.clear();
        self.masonry_state.handle_suspended(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if event == winit::event::WindowEvent::RedrawRequested {
            return;
        }

        // wayland doesn't support keyboard device events so use window events instead
        // Note: on x11 keyboard events have buffering issue with repeat keys, so can't need to
        // use device events there. Also device events seem to arrive slightly earlier than window
        // events so they are preferable.
        #[cfg(target_os = "linux")]
        if event_loop.is_wayland() {
            if let winit::event::WindowEvent::KeyboardInput { .. } = &event{
                self.game_state
                .lock()
                .unwrap()
                .handle_window_key_event(&event);
            }    
        }

        self.masonry_state.handle_window_event(
            event_loop,
            window_id,
            event,
            self.app_driver.as_mut(),
        );
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: MasonryUserEvent,
    ) {
        self.masonry_state.handle_user_event(event_loop, event, self.app_driver.as_mut());
    }

    fn device_event(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, _device_id: winit::event::DeviceId, event: winit::event::DeviceEvent,
    ) {
        self.game_state
            .lock()
            .unwrap()
            .handle_device_event(&event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        {
            let mut game_state = self.game_state.lock().unwrap();
            game_state.update();
            if game_state.is_exit_ready() {
                event_loop.exit();
            }

            if !game_state.ready_for_redraw() {
                return;
            }

            // The rest of this method is rendering
            game_state.interpolate_transforms();

            // Need to let go of mutex because render will need game data
            drop(game_state);

            self.masonry_state.get_root().edit_root_widget(|mut root| {
                let mut game_portal = root.downcast::<RootWidget<GamePortal>>();
                let mut game_portal = RootWidget::child_mut(&mut game_portal);
                game_portal.ctx.request_paint_only();
            });
    
            self.render_mgr.render(&mut self.masonry_state, &self.game_state);

            // TODO: masonry calls poll here. Should we do the same?
//            if let Some((device, _queue)) = self.masonry_state.get_render_device_and_queue() {
//                device.poll(vello::wgpu::Maintain::Wait);
//            }
        }
    }
}

fn create_game_world() -> GameWorld {
    // generate seed from time
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let seed = time.as_secs() as u64 ^ time.subsec_nanos() as u64;

    let mut game_world = GameWorld::new(seed, 4000.0);

    // add the player ship at the origin
    let world_center = xilem::Vec2::new(0.0, 0.0);
    let ship_id = game_world.add_ship(world_center..world_center);
    game_world.set_control_object(ship_id);

    let upper_left = game_world.get_spatial_db().get_min();
    let lower_right = game_world.get_spatial_db().get_max();

    // add some asteroids
    for _ in 0..80 {
        game_world.add_asteroid(upper_left..lower_right, 0.0..10.0, 0.0..0.1);
    }

    game_world.add_air_pod(upper_left..lower_right);

    game_world
}

pub struct AppInterface {
    masonry_state: masonry::app::MasonryState<'static>,
    app_driver: Box<dyn AppDriver>,
    game_state: GameState,
    render_mgr: RenderManager,
}

fn main() -> Result<(), EventLoopError> {
    let game_state = GameState::new(Mutex::new(create_game_world()));

    let window_size = winit::dpi::LogicalSize::new(1200.0, 1200.0);
    let window_attributes = winit::window::Window::default_attributes()
        .with_title("Space Survival".to_string())
        .with_resizable(true)
        .with_min_inner_size(window_size);

    let xilem = Xilem::new(game_state.clone(), app_logic);

    let event_loop = xilem::EventLoop::with_user_event().build().unwrap();
    let proxy = MasonryProxy::new(event_loop.create_proxy());
    let (widget, driver) = xilem.into_driver(Arc::new(proxy));

    let masonry_state =
        masonry::app::MasonryState::new(window_attributes, &event_loop, widget, Color::BLACK);

    let mut app = AppInterface {
        render_mgr: RenderManager::new(),
        masonry_state,
        app_driver: Box::new(driver),
        game_state,
    };
    event_loop.run_app(&mut app)
}
