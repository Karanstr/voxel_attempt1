use std::sync::Arc;
use wgpu::{util::DeviceExt, PipelineCompilationOptions};
use winit::window::Window;
use crate::app::GameData;
use crate::graph::basic_node3d::BasicNode3d;
use crate::graph::sdg::*;

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
  render_root: [u32; 2],
  camera_pos: [f32; 3],
  padding1: f32,
  camera_dir: [f32; 3],
  padding2: f32,
}
// We align every 16 bytes, padding to make sure everything lines up correctly.
// This is forced because of the way Vec3 alignments work in wgsl
impl Data {
  fn new(projection: glam::Mat4, resolution: [f32; 2], render_root: [u32; 2], camera_pos: [f32; 3], camera_dir: [f32; 3]) -> Self {
    Self {
      model: glam::Mat4::IDENTITY.to_cols_array_2d(),
      projection: projection.to_cols_array_2d(),
      resolution,
      render_root,
      camera_pos,
      padding1: 0.0,
      camera_dir,
      padding2: 0.0
    }
  }
  fn dummy() -> Self {
    Self {
      model: [[0.0; 4]; 4],
      projection: [[0.0; 4]; 4],
      resolution: [0.0; 2],
      render_root: [0; 2],
      camera_pos: [0.0; 3],
      padding1: 0.0,
      camera_dir: [0.0; 3],
      padding2: 0.0,
    }
  }
}

pub struct WgpuCtx<'window> {
  surface: wgpu::Surface<'window>,
  surface_config: wgpu::SurfaceConfiguration,
  device: wgpu::Device,
  queue: wgpu::Queue,
  render_pipeline: wgpu::RenderPipeline,
  vertex_buffer: wgpu::Buffer,
  index_buffer: wgpu::Buffer,
  data_buffer: wgpu::Buffer,
  voxel_buffer: wgpu::Buffer,
  data_bind_group: wgpu::BindGroup,
}

impl<'window> WgpuCtx<'window> {
  pub async fn new_async(window: Arc<Window>) -> WgpuCtx<'window> {
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(Arc::clone(&window))
      .expect("Failed to create surface from window!");
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
      power_preference: wgpu::PowerPreference::default(),
      force_fallback_adapter: false,
      compatible_surface: Some(&surface),
    }).await.expect("Failed to find an appropriate adapter");
    // Create the logical device and command queue
    let (device, queue) = adapter.request_device(
      &wgpu::DeviceDescriptor {
        label: None,
        required_features: wgpu::Features::default(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
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

    // Create uniform buffer for camera and scene data, filling it with dummy data
    // Yes we could go and just set contents: [0.0; buffersize], but that silently fails as soon as
    // I change Data and I'd rather a loud failure
    let data_buffer = device.create_buffer_init(
      &wgpu::util::BufferInitDescriptor {
        label: Some("Data Buffer"),
        contents: bytemuck::cast_slice(&[Data::dummy()]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      }
    );

    // Create storage buffer for voxel data, allocating room for up to 64 nodes
    let voxel_buffer = device.create_buffer_init(
      &wgpu::util::BufferInitDescriptor {
        label: Some("Voxel Storage Buffer"),
        contents: bytemuck::cast_slice(&Vec::from( [BasicNode3d::new(&[0; 8]); 64] )),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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
          wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
              ty: wgpu::BufferBindingType::Storage { read_only: true },
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
        wgpu::BindGroupEntry {
          binding: 1,
          resource: voxel_buffer.as_entire_binding(),
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
      voxel_buffer,
      data_bind_group,
    }
  }

  pub fn new(window: Arc<Window>) -> WgpuCtx<'window> { pollster::block_on(WgpuCtx::new_async(window)) }

  // REMEMBER TO UPDATE THIS IF WE MOVE THE RESOLUTION FIELD
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

  pub fn draw(&mut self, game_data: &GameData) {
    let grid_origin = glam::Vec3::new(0.0, 0.0, 0.0);

    // Update data buffer with new values
    let data = Data::new(
      game_data.camera.projection_matrix(),
      [self.surface_config.width as f32, self.surface_config.height as f32],
      [game_data.render_root.idx, game_data.render_root.height],
      (game_data.camera.position() - grid_origin).into(),
      game_data.camera.forward().into(),
    );

    // Update voxel buffer
    let voxels = game_data.sdg.nodes.data().iter().map(|x|
      x.clone().unwrap_or(BasicNode3d::new(&[u32::MAX; 8]))
    ).collect::<Vec<_>>();

    self.queue.write_buffer(&self.voxel_buffer, 0, bytemuck::cast_slice(&voxels));
    self.queue.write_buffer(&self.data_buffer, 0, bytemuck::cast_slice(&[data]));
    
    // Acquires next texture to be drawn to screen
    let surface_texture = self.surface.get_current_texture()
      .expect("Failed to acquire next swap chain texture");
    // Allows the texture to be run through a render_pass
    let texture_view = surface_texture.texture
      .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = self.device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      label: Some("Render Pass"),
      color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        view: &texture_view, // We want to operate on this texture
        resolve_target: None,
        ops: wgpu::Operations {
          load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), // Clear the texture when loaded
          store: wgpu::StoreOp::Store, // Store output to this texture
        },
      })],
      depth_stencil_attachment: None,
      timestamp_writes: None,
      occlusion_query_set: None,
    });
    render_pass.set_pipeline(&self.render_pipeline);
    render_pass.set_bind_group(0, &self.data_bind_group, &[]);
    render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
    render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
    render_pass.draw_indexed(0..6, 0, 0..1);
    drop(render_pass);

    self.queue.submit(Some(encoder.finish()));
    surface_texture.present();
  }
}

