// This file contains code copied (and slightly modified) from vello that was private in vello-land

use vello::wgpu::{self, Device, PipelineCompilationOptions, TextureFormat, TextureView};


pub struct TargetTexture {
    view: TextureView,
    width: u32,
    height: u32,
}

impl TargetTexture {
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
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
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            view,
            width,
            height,
        }
    }

    pub fn need_resize(&self, width: u32, height: u32) -> bool {
        self.width != width || self.height != height
    }

    pub fn get_view(&self) -> &TextureView {
        &self.view
    }
}

pub struct BlitPipeline {
    bind_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

impl BlitPipeline {
    #[allow(dead_code)]
    pub fn new(device: &Device, format: TextureFormat) -> Self {
        Self::new_with_blend(device, format, None)
    }

    pub fn new_with_blend(device: &Device, format: TextureFormat, blend_state: Option<wgpu::BlendState>) -> Self {
        const SHADERS: &str = r#"
            @vertex
            fn vs_main(@builtin(vertex_index) ix: u32) -> @builtin(position) vec4<f32> {
                // Generate a full screen quad in normalized device coordinates
                var vertex = vec2(-1.0, 1.0);
                switch ix {
                    case 1u: {
                        vertex = vec2(-1.0, -1.0);
                    }
                    case 2u, 4u: {
                        vertex = vec2(1.0, -1.0);
                    }
                    case 5u: {
                        vertex = vec2(1.0, 1.0);
                    }
                    default: {}
                }
                return vec4(vertex, 0.0, 1.0);
            }

            @group(0) @binding(0)
            var fine_output: texture_2d<f32>;

            @fragment
            fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
                let rgba_sep = textureLoad(fine_output, vec2<i32>(pos.xy), 0);
                return vec4(rgba_sep.rgb * rgba_sep.a, rgba_sep.a);
            }
        "#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit shaders"),
            source: wgpu::ShaderSource::Wgsl(SHADERS.into()),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                visibility: wgpu::ShaderStages::FRAGMENT,
                binding: 0,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: blend_state,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });
        Self {
            bind_layout,
            pipeline,
        }
    }

    pub fn get_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_layout
    }

    pub fn get_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

}