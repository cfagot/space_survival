use accesskit::TreeUpdate;
use masonry::{app::{MasonryState, WindowState}, widgets::RootWidget};
use vello::{peniko::color::AlphaColor, wgpu::{BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, BlendState, Buffer, Device, Queue, RenderPass, TextureFormat}, Scene};
use xilem::Affine;

use crate::{game_view::GamePortal, render_mgr::Renderer, vello_ext, GameState};

pub struct XilemRenderer {
    tree_update: Option<TreeUpdate>,
    target_texture: Option<vello_ext::TargetTexture>,
    blit: Option<vello_ext::BlitPipeline>,
    blit_bind_group: Option<BindGroup>,
    renderer: vello::Renderer,
}

impl XilemRenderer {
    pub fn setup(device: &Device, _queue: &Queue, _global_buffer: &Buffer, surface_format: TextureFormat) -> Self {
        let blit =vello_ext::BlitPipeline::new_with_blend(device, surface_format, Some(BlendState::ALPHA_BLENDING));
        let renderer = vello::Renderer::new(device, vello::RendererOptions {
            surface_format: Some(surface_format),
            use_cpu: false,
            antialiasing_support: vello::AaSupport {
                area: true,
                msaa8: false,
                msaa16: false,
            },
            num_init_threads: std::num::NonZeroUsize::new(1),
        }).unwrap();

        Self {
            tree_update: None,
            target_texture: None,
            blit: Some(blit),
            blit_bind_group: None,
            renderer,
        }
    }

    fn set_blit_bind_group(&mut self, device: &Device, target_texture: &vello_ext::TargetTexture) {
        self.blit_bind_group = Some(device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &self.blit.as_ref().unwrap().get_bind_group_layout(),
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(target_texture.get_view()),
            }],
        }));
    }
}

impl Renderer for XilemRenderer {
    fn prepare(&mut self, masonry_state: &mut MasonryState, _game_state: &GameState, width: u32, height: u32) {
        let scale_factor = if let WindowState::Rendering { window, .. } = masonry_state.get_window_state() {
            window.scale_factor()
        }
        else {
            // should never get here when window state is not rendering
            return;
        };

        masonry_state.get_root().edit_root_widget(|mut root| {
            let mut game_portal = root.downcast::<RootWidget<GamePortal>>();
            let mut game_portal = RootWidget::child_mut(&mut game_portal);
            game_portal.ctx.request_paint_only();
        });

        let (scene, tree_update) = masonry_state.get_root().redraw();
        self.tree_update = Some(tree_update);

        let Some((device, queue)) = masonry_state.get_render_device_and_queue() else {
            unreachable!("Failed to get render device and queue");
        };

        // fiddle with target texture
        if self.target_texture.as_ref().map(|t| t.need_resize(width, height)).unwrap_or(true) {
            let target_texture = vello_ext::TargetTexture::new(device, width, height);
            self.set_blit_bind_group(device, &target_texture);
            self.target_texture = Some(target_texture);
        }

        let render_params = vello::RenderParams {
            base_color: AlphaColor::new([0.0, 0.0, 0.0, 0.0]),
            width,
            height,
            antialiasing_method: vello::AaConfig::Area,
        };

        // get surface scale and scale scene by it
        let transformed_scene = if scale_factor == 1.0 {
            None
        } else {
            let mut new_scene = Scene::new();
            new_scene.append(&scene, Some(Affine::scale(scale_factor)));
            Some(new_scene)
        };
        let scene_ref = transformed_scene.as_ref().unwrap_or(&scene);

        // Note: this performas a compute render pass. Might be worth holding onto the encoder and re-using for remaining passes
        self.renderer.render_to_texture(device, queue, scene_ref, self.target_texture.as_ref().unwrap().get_view(), &render_params).unwrap();
    }

    fn render<'rpass>(&'rpass self, render_pass: &mut RenderPass<'rpass>, _width: u32, _height: u32) {
        if let Some(blit) = &self.blit {
            render_pass.set_pipeline(blit.get_pipeline());
            render_pass.set_bind_group(0, self.blit_bind_group.as_ref().unwrap(), &[]);
            render_pass.draw(0..6, 0..1);
        }
    }

    fn finish_render(&mut self, masonry_state: &mut MasonryState, _: &GameState) {
        if let Some(tree_update) = self.tree_update.take() {
            masonry_state.handle_tree_update(tree_update);
        }
    }
}