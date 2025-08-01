use std::{sync::Arc, u32};
use glam::{Mat3, Vec2};
use sdg::prelude::{BasicNode3d, SparseDirectedGraph};
use winit::window::Window;
use crate::{app::{GameData, ObjectData}, camera::Camera};

const SCALE: f32 = 1.0 / 1.0; // ./shaders/upscale.wgsl
const WORKGROUP: u32 = 8; // ./shaders/dda.wgsl

#[repr(C, align(16))]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ObjData {
  pos: [f32; 3],
  padding2: f32,

  inv_right: [f32; 3],
  padding3: f32,
  inv_up: [f32; 3],
  padding4: f32,
  inv_forward: [f32; 3],
  padding5: f32,
  
  head: u32,
  height: u32,
  extent: u32,
  padding: f32,
}
impl ObjData {
  fn new(data: &ObjectData) -> Self {
    let inv_mat = Mat3::from_quat(data.rot.inverse());
    Self {
      pos: data.pos.into(),
      padding2: 0.0,

      inv_right: inv_mat.col(0).into(),
      padding3: 0.0,
      inv_up: inv_mat.col(1).into(),
      padding4: 0.0,
      inv_forward: inv_mat.col(2).into(),
      padding5: 0.0,

      head: data.head,
      height: data.height,
      extent: data.bounds,
      padding: 0.0,
    }
  }
}

// Remember that vec3's are extended to 16 bytes
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CamData {
  pos: [f32; 3],
  padding2: f32,

  right: [f32; 3],
  padding3: f32,
  up: [f32; 3],
  padding4: f32,
  forward: [f32; 3],
  padding5: f32,

  aspect_ratio: f32,
  tan_fov: f32,
  padding1: [f32; 2],
}
impl CamData {
  fn new(camera: &Camera) -> Self {
    Self {
      pos: camera.position.into(),
      padding2: 0.0,

      right: camera.basis()[0].into(),
      padding3: 0.,
      up: camera.basis()[1].into(),
      padding4: 0.,
      forward: camera.basis()[2].into(),
      padding5: 0.,

      aspect_ratio: camera.aspect_ratio,
      tan_fov: (camera.fov / 2.).tan(),
      padding1: [0.0; 2],
   }
  } 
}

struct DdaModule {
  voxel_buffer: wgpu::Buffer,
  cam_buffer: wgpu::Buffer,
  objects_buffer: wgpu::Buffer,
  bind_group_layout: wgpu::BindGroupLayout,
  pipeline: wgpu::ComputePipeline,
  // We can't create the bind group without an associated texture
  bind_group: Option<wgpu::BindGroup>
} 
impl DdaModule {
  fn create(device: &wgpu::Device, bytes_in_voxel_buffer: u64) -> Self {
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("DDA BGL"),
      entries: &[
        // Texture
        wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::COMPUTE,
          ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_dimension: wgpu::TextureViewDimension::D2,
          },
          count: None,
        },
        // Cam Buffer
        wgpu::BindGroupLayoutEntry {
          binding: 1,
          visibility: wgpu::ShaderStages::COMPUTE,
          ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
          },
          count: None,
        },
        // Voxel Buffer
        wgpu::BindGroupLayoutEntry {
          binding: 2, 
          visibility: wgpu::ShaderStages::COMPUTE,
          ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
          },
          count: None,
        },
        wgpu::BindGroupLayoutEntry {
          binding: 3,
          visibility: wgpu::ShaderStages::COMPUTE,
          ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
          },
          count: None,
        },
      ],
    });
    let cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("Cam Buffer"),
      size: std::mem::size_of::<CamData>() as u64,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });
    let voxel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("Voxel Buffer"),
      size: bytes_in_voxel_buffer,
      usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false
    });
    let count = 4;
    let objects_buffer = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("Objects Buffer"),
      size: std::mem::size_of::<ObjData>() as u64 * count,
      usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
      layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("DDA Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[]
      })),
      cache: None,
      compilation_options: wgpu::PipelineCompilationOptions::default(),
      module: &device.create_shader_module(wgpu::include_wgsl!("shaders/dda.wgsl")),
      entry_point: Some("main"),
      label: Some("DDA Pipeline")
    });
    
    Self {
      voxel_buffer,
      cam_buffer,
      objects_buffer,
      pipeline,
      bind_group_layout,
      bind_group: None
    }
  }

  fn set_output_texture(&mut self, device: &wgpu::Device, output: &wgpu::TextureView) {
    self.bind_group = Some( device.create_bind_group(&wgpu::BindGroupDescriptor {
      layout: &self.bind_group_layout,
      entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(output), },
        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Buffer(self.cam_buffer.as_entire_buffer_binding()), },
        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Buffer(self.voxel_buffer.as_entire_buffer_binding()), },
        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Buffer(self.objects_buffer.as_entire_buffer_binding()), },
      ],
      label: Some("Dda BindGroup"),
    }) );
  }
}

struct UpscaleModule {
  bind_group_layout: wgpu::BindGroupLayout,
  pipeline: wgpu::RenderPipeline,
  // We can't create the bind group without an associated texture
  bind_group: Option<wgpu::BindGroup>
}
impl UpscaleModule {
  fn create(device: &wgpu::Device, adapter: &wgpu::Adapter, surface: &wgpu::Surface) -> Self {
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("Upscale BGL"),
      entries: &[
        wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Texture {
            multisampled: false,
            view_dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
          },
          count: None,
        },
        wgpu::BindGroupLayoutEntry {
          binding: 1,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
          count: None,
        },
      ],
    });
    let upscale_module = device.create_shader_module(wgpu::include_wgsl!("shaders/upscale.wgsl"));
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("Upscale Pipeline"),
      layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Upscale Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
      })),
      cache: None,
      vertex: wgpu::VertexState {
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        module: &upscale_module,
        entry_point: Some("vs_main"),
        buffers: &[],
      },
      fragment: Some(wgpu::FragmentState {
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        module: &upscale_module,
        entry_point: Some("fs_main"),
        targets: &[Some(wgpu::ColorTargetState {
          format: surface.get_capabilities(&adapter).formats[0],
          blend: Some(wgpu::BlendState::REPLACE),
          write_mask: wgpu::ColorWrites::ALL,
        })],
      }),
      primitive: wgpu::PrimitiveState::default(),
      depth_stencil: None,
      multisample: wgpu::MultisampleState::default(),
      multiview: None
    });
    Self { bind_group_layout, pipeline, bind_group: None}
  }

  fn set_input_texture(&mut self, device: &wgpu::Device, input: &wgpu::TextureView, sampler: &wgpu::Sampler) {
    self.bind_group = Some( device.create_bind_group(&wgpu::BindGroupDescriptor {
      layout: &self.bind_group_layout,
      entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&input) },
        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
      ],
      label: Some("Upscale BindGroup"),
    }) );
  }
}

pub struct WgpuCtx<'window> {
  surface: wgpu::Surface<'window>,
  surface_config: wgpu::SurfaceConfiguration,
  device: wgpu::Device,
  queue: wgpu::Queue,
  sampler: wgpu::Sampler,
  dda_compute: DdaModule,
  upscale_render: UpscaleModule,
}
impl<'window> WgpuCtx<'window> {
  pub fn new(window: Arc<Window>) -> WgpuCtx<'window> {
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(Arc::clone(&window)).unwrap();
    let adapter = pollster::block_on( instance.request_adapter(&wgpu::RequestAdapterOptions {
      compatible_surface: Some(&surface),
      ..Default::default()
    })).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();

    let size = window.inner_size();
    let surface_config = surface.get_default_config(&adapter, size.width, size.height).unwrap();
    surface.configure(&device, &surface_config);

    let dda_compute = DdaModule::create(&device, 64_000_000);
    let upscale_render = UpscaleModule::create(&device, &adapter, &surface);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    let mut ctx = WgpuCtx {
      surface,
      surface_config,
      device,
      queue,
      sampler,
      dda_compute,
      upscale_render,
    };
    ctx.gen_dda_texture();
    ctx
  }

  fn gen_dda_texture(&mut self) {
    let dda_output = self.device.create_texture(&wgpu::TextureDescriptor {
      label: Some("Dda Texture"),
      size: wgpu::Extent3d {
        width: (self.surface_config.width as f32 * SCALE) as u32,
        height: (self.surface_config.height as f32 * SCALE) as u32,
        depth_or_array_layers: 1
      },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: wgpu::TextureFormat::Rgba8Unorm,
      usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
      view_formats: &[],
    }).create_view(&Default::default());

    self.dda_compute.set_output_texture(&self.device, &dda_output);
    self.upscale_render.set_input_texture(&self.device, &dda_output, &self.sampler);
  }

  pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
    self.surface_config.width = new_size.width;
    self.surface_config.height = new_size.height;
    self.surface.configure(&self.device, &self.surface_config);
    self.gen_dda_texture();
  }

  /// Writes the raw memory of the graph into a GPU buffer
  pub fn update_voxels(&self, sdg:&SparseDirectedGraph<BasicNode3d>) {
    self.queue.write_buffer(
      &self.dda_compute.voxel_buffer,
      0,
      bytemuck::cast_slice(& unsafe { std::slice::from_raw_parts(
        // Pointer to the raw data, converted to a pointer of bytes
        sdg.nodes.unsafe_data().as_ptr() as *const u8,
        // Number of elements * bytes per element
        sdg.nodes.len() * std::mem::size_of::<BasicNode3d>(),
      )})
    );
  }

  fn dda(&mut self, game_data: &GameData, encoder: &mut wgpu::CommandEncoder) {
    let cam = CamData::new(&game_data.camera);
    self.queue.write_buffer(&self.dda_compute.cam_buffer, 0, bytemuck::bytes_of(&cam));
    let world = ObjData::new(&game_data.world_data);
    let block1 = ObjData::new(&game_data.cube1);
    let block2 = ObjData::new(&game_data.cube2);
    let block3 = ObjData::new(&game_data.cube3);
    self.queue.write_buffer(&self.dda_compute.objects_buffer, 0, bytemuck::cast_slice(&[world, block1, block2, block3]));

    let mut compute_pass = encoder.begin_compute_pass(&Default::default());
    compute_pass.set_pipeline(&self.dda_compute.pipeline);
    compute_pass.set_bind_group(0, &self.dda_compute.bind_group, &[]);
    let size = Vec2::new(self.surface_config.width as f32, self.surface_config.height as f32);
    let scaled_size = ((size * SCALE).as_uvec2() + WORKGROUP - 1) / WORKGROUP; // Round up with int math
    compute_pass.dispatch_workgroups(scaled_size.x, scaled_size.y, 1);
  }
  
  fn upscale(&mut self, frame_view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
    let mut upscale_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      label: Some("Render Pass"),
      color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        view: &frame_view,
        resolve_target: None,
        ops: wgpu::Operations {
          load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
          store: wgpu::StoreOp::Store,
        },
      })],
      depth_stencil_attachment: None,
      timestamp_writes: None,
      occlusion_query_set: None,
    });
    upscale_pass.set_pipeline(&self.upscale_render.pipeline);
    upscale_pass.set_bind_group(0, &self.upscale_render.bind_group, &[]);
    upscale_pass.draw(0..3, 0..1);
  }

  pub fn draw(&mut self, game_data: &GameData) {
    let frame = self.surface.get_current_texture().unwrap();
    let view = frame.texture.create_view(&Default::default());
    let mut encoder = self.device.create_command_encoder(&Default::default());

    self.dda(game_data, &mut encoder);
    self.upscale(&view, &mut encoder);

    self.queue.submit(Some(encoder.finish()));
    frame.present();
  }
}

