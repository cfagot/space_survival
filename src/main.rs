use std::sync::{Arc, Mutex};

use masonry::{app_driver::AppDriver, event_loop_runner::WindowState, widget::RootWidget, Vec2};
use render_mgr::RenderManager;
use starfield_render::StarfieldRenderer;
use winit::{self, application::ApplicationHandler, error::EventLoopError};
use xilem::{WidgetView, Xilem};

mod game_view;
use game_view::{GamePortal, GameView};

mod game;
use game::GameWorld;
use xilem_render::XilemRenderer;

mod game_shapes;

mod render_mgr;
mod starfield_render;
mod xilem_render;

fn app_logic(data: &mut GameState) -> impl WidgetView<GameState> {
    GameView::new(data.clone())
}

pub type GameState = Arc<Mutex<GameWorld>>;

impl ApplicationHandler<accesskit_winit::Event> for AppInterface {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.masonry_state.handle_resumed(event_loop);
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        self.masonry_state
            .set_present_mode(vello::wgpu::PresentMode::Immediate);

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
        if event != winit::event::WindowEvent::RedrawRequested {
            self.masonry_state.handle_window_event(
                event_loop,
                window_id,
                event,
                self.app_driver.as_mut(),
            );
        }
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: accesskit_winit::Event,
    ) {
        self.masonry_state.handle_user_event(event_loop, event, self.app_driver.as_mut());
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.game_state
            .lock()
            .unwrap()
            .handle_device_event(device_id, &event);
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
                root.downcast::<RootWidget<GamePortal>>()
                    .get_element()
                    .ctx
                    .request_paint();
            });
    
            self.render_mgr.render(&mut self.masonry_state, &self.game_state);

            if let Some((device, _queue)) = self.masonry_state.get_render_device_and_queue() {
                device.poll(vello::wgpu::Maintain::Wait);
            }
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
    let world_center = Vec2::new(0.0, 0.0);
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
    masonry_state: masonry::event_loop_runner::MasonryState<'static>,
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
    let parts = xilem.split();

    let event_loop = xilem::EventLoop::with_user_event().build().unwrap();
    let masonry_state =
        masonry::event_loop_runner::MasonryState::new(window_attributes, &event_loop, parts.root_widget);

    let mut app = AppInterface {
        render_mgr: RenderManager::new(),
        masonry_state,
        app_driver: Box::new(parts.driver),
        game_state,
    };
    event_loop.run_app(&mut app)
}
