use bytemuck::{Pod, Zeroable};
use masonry::{event_loop_runner::{MasonryState, WindowState}, Vec2};
use vello::wgpu::{self, Buffer, Device, RenderPass};

use crate::GameState;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GlobalRenderData {
    pub pos: [f32; 2],
    pub screen_size: [f32; 2],
}
impl GlobalRenderData {
    pub fn setup(device: &Device) -> Buffer {
        let global_render_desc = wgpu::BufferDescriptor {
            label: Some("GlobalBuffer"),
            size: std::mem::size_of::<GlobalRenderData>() as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };

        device.create_buffer(&global_render_desc)
    }
}

pub trait Renderer {
    fn prepare(&mut self,masonry_state: &mut MasonryState, game_state: &GameState, width: u32, height: u32);
    fn render<'rpass>(&'rpass self, render_pass: &mut RenderPass<'rpass>, width: u32, height: u32);
    fn finish_render(&mut self, masonry_state: &mut MasonryState, game_state: &GameState);
}

pub struct RenderManager {
    renderers: Vec<Box<dyn Renderer>>,
    global_render_data_buffer: Option<Buffer>,
}

impl RenderManager {
    pub fn new() -> Self {
        Self {
            renderers: Vec::new(),
            global_render_data_buffer: None,
        }
    }

    pub fn setup(&mut self, device: &Device) {
        self.global_render_data_buffer = Some(GlobalRenderData::setup(device));
    }

    pub fn clear(&mut self) {
        self.global_render_data_buffer = None;
        self.renderers.clear();
    }

    pub fn get_global_buffer(&self) -> Option<&Buffer> {
        self.global_render_data_buffer.as_ref()
    }

    pub fn add_renderer(&mut self, renderer: Box<dyn Renderer>) {
        self.renderers.push(renderer);
    }

    pub fn render(&mut self, masonry_state: &mut MasonryState, game_state: &GameState) {
        let (width, height) = if let WindowState::Rendering {
            window, ..
        } = &mut masonry_state.get_window_state() {
            let size = window.inner_size();
            (size.width, size.height)
        }
        else {
            return ;
        };

        if let Some((_device, queue)) = masonry_state.get_render_device_and_queue() {
            let game_world = game_state.lock().unwrap();
            let cam_pos = if let Some(control_obj) = game_world.get_control_object() {
                let control_obj = &game_world.get_entities().get(control_obj);
                control_obj.render_transform.translation()
            }
            else {
                // no control object, put camera at origin
                Vec2::ZERO
            };

            // fill global buffer
            if let Some(global_buffer) = self.global_render_data_buffer.as_ref() {
                let global_render_data = GlobalRenderData { pos: [cam_pos.x as f32, cam_pos.y as f32], screen_size: [width as f32, height as f32] };
                queue.write_buffer(global_buffer, 0, bytemuck::cast_slice(&[global_render_data]));
            }    
        }
        else {
            unreachable!()
        }

        for renderer in &mut self.renderers {
            renderer.prepare(masonry_state, &game_state, width, height);
        }

        let surface_texture = masonry_state.get_next_frame();
        let Ok(surface_texture) = surface_texture else {
            log::error!("Failed to get surface texture for next frame: {:?}", surface_texture);
            return;
        };


        // get encoder and surface view in order to render next frame
        let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let Some((device, queue)) =  masonry_state.get_render_device_and_queue() else {
            unreachable!();
        };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &surface_view,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
            resolve_target: None,
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wgpu render pass"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        for renderer in &self.renderers {
            renderer.render(&mut render_pass, width, height);
        }
        drop(render_pass);

        queue.submit(Some(encoder.finish()));
        surface_texture.present();

        for renderer in &mut self.renderers {
            renderer.finish_render(masonry_state, game_state);
        }
    }
}

