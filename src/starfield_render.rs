use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use masonry::event_loop_runner::MasonryState;
use vello::wgpu::{self, BindGroup, BlendState, Buffer, Device, Queue, RenderPass, RenderPipeline, TextureFormat};

use crate::{game::HashRand, render_mgr::{GlobalRenderData, Renderer}, GameState};


#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct StarVertex {
    offset: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct StarInstance {
    position: [f32; 2],
    color: [f32; 3],
    radius: f32,
    depth: f32,
}

pub struct StarfieldRenderer {
    instance_buffer: Buffer,
    vertex_buffer: Buffer,
    instance_count: u32,

    bind_group: BindGroup,

    render_pipeline: RenderPipeline,
}

impl Renderer for StarfieldRenderer {
    fn prepare(&mut self, _: &mut MasonryState, _: &GameState,_width: u32, _height: u32) {
    }

    fn render<'rpass>(&'rpass self, render_pass: &mut RenderPass<'rpass>, _width: u32, _height: u32) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);

        // render starfield
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.draw(0..3, 0..self.instance_count);
    }

    fn finish_render(&mut self, _masonry_state: &mut MasonryState, _: &GameState) {
    }
}

impl StarfieldRenderer {
    pub fn setup(device: &Device, queue: &Queue, global_buffer: &Buffer, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("starfield shaders"),
            source: wgpu::ShaderSource::Wgsl(STARFIELD_VERTEX_SHADER.into()),
        });

        // Create vertices -- same triangle for each star instance
        let vertices = [
           StarVertex { offset: [ 0.0, -2.0]},
           StarVertex { offset: [-3.0f32.sqrt(), 1.0]},
           StarVertex { offset: [ 3.0f32.sqrt(), 1.0]},
        ];

        // create the star instance data
        let seed = 2828;
        let num_stars = 4000;
        let size_range: Range<f64> = 10.0..20.0;
        let dim_range: Range<f64> = -2000.0..2000.0;
        let max_depth_ratio = 3.0;
        let mut instances: Vec<StarInstance> = Vec::with_capacity(num_stars);
        for i in 0..num_stars {
            let depth = 1.0 + (max_depth_ratio-1.0) * (i as f64 / num_stars as f64) as f32;
            let size = size_range.clone().hash_rand(seed, ("size",i)) as f32;
            let x = depth * dim_range.clone().hash_rand(seed, ("x",i)) as f32;
            let y = depth * dim_range.clone().hash_rand(seed, ("y",i)) as f32;

            let select = (0.0..1.0).hash_rand(seed, ("shape",i)) as f32;

            let color = star_creator(depth, size, select);
            instances.push( StarInstance {
                position: [x, y],
                color,
                radius: size/depth,
                depth,
            });
        }

        // Create buffer descriptors here and clone them for each tilemap
        let vertex_buffer_desc = wgpu::BufferDescriptor {
            label: Some("StarfieldVertexBuffer"),
            size: vertices.len() as u64 * std::mem::size_of::<StarVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };

        let instance_buffer_desc = wgpu::BufferDescriptor {
            label: Some("StarfieldInstanceBuffer"),
            size: instances.len() as u64 * std::mem::size_of::<StarInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };

        let vertex_buffer = device.create_buffer(&vertex_buffer_desc);
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices[..]));

        let instance_buffer = device.create_buffer(&instance_buffer_desc);
        queue.write_buffer(&instance_buffer, 0, bytemuck::cast_slice(&instances[..]));

        let (bind_group_layout, bind_group) = StarfieldRenderer::create_bind_group(&device, &global_buffer);

        let pipeline_layout =
            device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                    label: None,
                });

        let render_pipeline =
            device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: None,
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[
                            // vertex buffer
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<StarVertex>() as u64,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &[
                                    // offset
                                    wgpu::VertexAttribute {
                                        offset: 0,
                                        format: wgpu::VertexFormat::Float32x2,
                                        shader_location: 0,
                                    },
                                ],
                            },
                            // instance buffer
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<StarInstance>() as u64,
                                step_mode: wgpu::VertexStepMode::Instance,
                                attributes: &[
                                    // position
                                    wgpu::VertexAttribute {
                                        offset: 0,
                                        format: wgpu::VertexFormat::Float32x2,
                                        shader_location: 1,
                                    },
                                    // color
                                    wgpu::VertexAttribute {
                                        offset: 8,
                                        format: wgpu::VertexFormat::Float32x3,
                                        shader_location: 2,
                                    },
                                    // radius
                                    wgpu::VertexAttribute {
                                        offset: 20,
                                        format: wgpu::VertexFormat::Float32,
                                        shader_location: 3,
                                    },
                                    // depth
                                    wgpu::VertexAttribute {
                                        offset: 24,
                                        format: wgpu::VertexFormat::Float32,
                                        shader_location: 4,
                                    },
                                ],
                            },
                        ],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: surface_format,
                            blend: Some(BlendState {
                                color: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::One,
                                    dst_factor: wgpu::BlendFactor::One,
                                    operation: wgpu::BlendOperation::Add,
                                },
                                alpha: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::One,
                                    dst_factor: wgpu::BlendFactor::One,
                                    operation: wgpu::BlendOperation::Add,
                                },
                            }),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        front_face: wgpu::FrontFace::Ccw,
                        strip_index_format: None,
                        cull_mode: None,
                        conservative: false,
                        unclipped_depth: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        Self {
            vertex_buffer,
            instance_buffer,
            instance_count: instances.len() as u32,
            bind_group,
            render_pipeline,
        }
    }


    fn create_bind_group(device: &Device, global_buffer: &Buffer) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
        let glob_size = std::mem::size_of::<GlobalRenderData>() as u64;
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Starfield bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(glob_size),
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Starfield bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(global_buffer.as_entire_buffer_binding()),
                },
            ],
        });
        (bind_group_layout, bind_group)
    }
}

fn star_creator(_dist: f32, _size: f32, select: f32) -> [f32;3] {
    // select some colors using select param but ignore dist and size
    if select < 0.2 {
        [0.8, select, select * 0.5]
    }
    else if select < 0.4 {
        [1.0, 1.0, 0.0]
    }
    else if select < 0.6 {
        let select= select - 0.6;
        [select, select*0.5, 0.8]
    }
    else if select < 0.8 {
        [1.0, 1.0, 1.0]
    }
    else {
        let select = select - 0.8;
        [2.0*select, 0.5, select * 0.5]
    }
}

const STARFIELD_VERTEX_SHADER: &str = r#"
struct GlobalRenderData {
    cam_pos: vec2<f32>,
    screen_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u_global: GlobalRenderData;

struct VertexInput {
    @location(0) offset: vec2<f32>,
};

struct InstanceInput {
    @location(1) position: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,
    @location(4) depth: f32,
};

struct VertexOutput {
    @location(0) color: vec4<f32>,
    @location(1) offset: vec2<f32>,
    @builtin(position) position: vec4<f32>
};

struct FragmentOutput {
    @location(0) out_color: vec4<f32>
};

//-------------------------------------------------
// Vertex shader
//-------------------------------------------------

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var local_pos = vec2<f32>(1.0, -1.0) * (instance.position - u_global.cam_pos)/instance.depth;
    let window = 2000.0;
    let twice_window = 2.0 * window;

    // this is position of star center
    local_pos = twice_window * fract((local_pos + window) / twice_window) - window;

    // apply offsets (scaled by radius)
    local_pos += instance.radius/instance.depth * vertex.offset;

    var position = vec4<f32>(2.0*local_pos.x/u_global.screen_size.x, 2.0*local_pos.y/u_global.screen_size.y, 0.1, 1.0);
    return VertexOutput(instance.color, vertex.offset, position);
}

//-------------------------------------------------
// Fragment shader
//-------------------------------------------------

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var k1 = 1.0-abs(in.offset.x);
    k1 = clamp(k1, 0.0, 1.0);
    k1 *= k1;
    var k2 = 1.0-abs(in.offset.y);
    k2 = clamp(k2, 0.0, 1.0);
    k2 *= k2;
    let k = k1*k2*clamp(1.0-dot(in.offset, in.offset), 0.0, 1.0);
    return FragmentOutput(k*mix(in.color, vec4<f32>(1.0,1.0,1.0, 1.0), k*k));
}
"#;