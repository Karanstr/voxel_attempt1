use std::sync::Arc;
use wgpu::{util::DeviceExt, MemoryHints::Performance, PipelineCompilationOptions, Trace};
use winit::window::Window;
use glam::Mat4;
use crate::camera::Camera;

// Data required to create buffers for full screen quad
const FULL_SCREEN_INDICIES: [u16; 6] = [0, 1, 2, 0, 2, 3];
const VERTICES : [Vertex; 4] = [
    Vertex { position: [-1.0, -1.0, 0.0], tex_coords: [0.0, 0.0] },
    Vertex { position: [1.0, -1.0, 0.0], tex_coords: [1.0, 0.0] },
    Vertex { position: [1.0, 1.0, 0.0], tex_coords: [1.0, 1.0] },
    Vertex { position: [-1.0, 1.0, 0.0], tex_coords: [0.0, 1.0] },
];
#[repr(C, align(16))]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Data {
    model: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    resolution: [f32; 2],
    padding1: [f32; 2],
    camera_pos: [f32; 3],
    padding2: f32,
    camera_dir: [f32; 3],
    padding3: f32,
    // Using vec4 alignment for uniform buffer requirements
    voxels: [[u32; 4]; (8 * 8 * 8) / 4]
}
impl Data {
    fn new(projection: Mat4, resolution: [f32; 2], voxels: VoxelWorld) -> Self {
        let mut aligned_voxels = [[0u32; 4]; (8 * 8 * 8) / 4];
        
        // Convert flat array to vec4 aligned array
        for i in 0..voxels.voxels.len() {
            let vec_index = i / 4;
            let component_index = i % 4;
            aligned_voxels[vec_index][component_index] = voxels.voxels[i];
        }
        
        Self {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            projection: projection.to_cols_array_2d(),
            resolution,
            padding1: [0.0, 0.0],
            camera_pos: [4.0, 4.0, 12.0], // Default camera position
            padding2: 0.0,
            camera_dir: [4.0, 4.0, 0.0],  // Default look at center of voxel grid
            padding3: 0.0,
            voxels: aligned_voxels
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VoxelWorld {
    // 8 ^ 3 world
    voxels: [u32; 8 * 8 * 8]
}
impl Default for VoxelWorld {
    fn default() -> Self {
        let mut world = Self {
            voxels: [0; 8 * 8 * 8]
        };
        
        // Create a small structure in the center
        for x in 3..6 {
            for y in 3..6 {
                for z in 3..6 {
                    // Make a hollow cube
                    if x == 3 || x == 5 || y == 3 || y == 5 || z == 3 || z == 5 {
                        world.set_voxel(x, y, z, true);
                    }
                }
            }
        }
        
        // Add a floor
        for x in 1..7 {
            for z in 1..7 {
                world.set_voxel(x, 1, z, true);
            }
        }
        
        // Add a pillar
        for y in 1..4 {
            world.set_voxel(1, y, 1, true);
        }
        
        world
    }
}
impl VoxelWorld {
    fn set_voxel(&mut self, x: u8, y: u8, z: u8, filled:bool) {
        let voxel = if filled { u32::MAX } else { 0 };
        self.voxels[x as usize * 8 * 8 + y as usize * 8 + z as usize] = voxel;
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}
impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

pub struct WgpuCtx<'window> {
    surface: wgpu::Surface<'window>,
    surface_config: wgpu::SurfaceConfiguration,
    // We don't need to bind this, but it does need to be constructed.
    // adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,

    data_buffer: wgpu::Buffer,
    data_bind_group: wgpu::BindGroup,

    voxel_world: VoxelWorld,
}

impl<'window> WgpuCtx<'window> {
    pub async fn new_async(window: Arc<Window>, voxel_world: VoxelWorld) -> WgpuCtx<'window> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(Arc::clone(&window)).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");
        // Create the logical device and command queue
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: Performance,
                trace: Trace::Off,
            },
        ).await.expect("Failed to create device");

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        let surface_config = surface.get_default_config(&adapter, width, height).unwrap();
        surface.configure(&device, &surface_config);
        
        // Load the shader from disk
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );
        let index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&FULL_SCREEN_INDICIES),
                usage: wgpu::BufferUsages::INDEX,
            }
        );
        
        // Create data buffer and bind group layout
        let data_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Data Buffer"),
                contents: bytemuck::cast_slice(&[Data::new(
                    Mat4::IDENTITY,
                    [width as f32, height as f32],
                    voxel_world
                )]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        
        // Creates a blueprint
        let bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        // Basically an id
                        binding: 0,
                        // Visible to both Vertex and Fragment shaders
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        // Type of binding
                        ty: wgpu::BindingType::Buffer {
                            // Uniform: All Data is the same type
                            ty: wgpu::BufferBindingType::Uniform,
                            // Is fixed length
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("bind_group_layout"),
            }
        );
        
        // Creates a bind group following above blueprint
        let data_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: data_buffer.as_entire_binding(),
                },
            ],
            label: Some("data_bind_group"),
        });
        
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            // Include the uniform bind group layout
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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
        
        WgpuCtx {
            surface,
            surface_config,
            device,
            queue,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            data_buffer,
            data_bind_group,
            voxel_world,
        }
    }

    pub fn new(window: Arc<Window>, voxel_world: VoxelWorld) -> WgpuCtx<'window> {
        pollster::block_on(WgpuCtx::new_async(window, voxel_world))
    }

    // REMEMBER TO UPDATE THIS IF WE MOVE RESOLUTION FIELD
    pub fn resize(&mut self, new_size: (u32, u32)) {
        let (width, height) = new_size;
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        
        // Update the resolution in the data buffer
        self.queue.write_buffer(
            &self.data_buffer,
            // Offset to resolution field, it's after 2 64 byte fields
            std::mem::size_of::<[[f32; 4]; 4]>() as u64 * 2,
            bytemuck::cast_slice(&[width as f32, height as f32]),
        );
    }

    pub fn draw(&mut self, camera: &Camera) {
        // Get projection matrix from camera
        let proj = camera.projection_matrix();
        
        // Update data buffer with new values
        let mut data = Data::new(
            proj,
            [self.surface_config.width as f32, self.surface_config.height as f32],
            self.voxel_world
        );
        
        // Update camera position and direction in the data struct
        data.camera_pos = camera.position_array();
        data.camera_dir = camera.forward_array();
        
        self.queue.write_buffer(&self.data_buffer, 0, bytemuck::cast_slice(&[data]));
        
        let surface_texture = self.surface.get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let texture_view = surface_texture.texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        
        // Create a command
        let mut encoder = self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        // Fill instructions
        {
            // Couldn't tell ya
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            
            // Tells GPU to use this pipeline
            render_pass.set_pipeline(&self.render_pipeline);

            // Says we'll be using this bind group
            render_pass.set_bind_group(0, &self.data_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..1);
        }
        
        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }
}

