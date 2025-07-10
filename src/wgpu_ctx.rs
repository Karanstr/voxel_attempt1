use std::{sync::Arc, u32};
#[allow(unused)]
use crate::graph::prelude::{BasicNode3d, Node, SparseDirectedGraph};
use winit::window::Window;
use crate::app::GameData;

// ALWAYS UPDATE CORESPONDING VALUES IN ./render.wgsl and ./compute.wgsl
const DOWNSCALE: u32 = 1;
const WORKGROUP_SQUARE: u32 = 8;

// Remember that vec3's are extended to 16 bytes
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Data {
    obj_head: u32,
    obj_bounds: u32,
    cam_aspect: f32,
    cam_tan_fov: f32,

    cam_cell: [i32; 3],
    padding1: f32,
    cam_offset: [f32; 3],
    padding2: f32,

    cam_forward: [f32; 3],
    padding3: f32,
    cam_right: [f32; 3],
    padding4: f32,
    cam_up: [f32; 3],
    padding5: f32,
}
impl Data {
    fn new(
      obj_head:u32,
      obj_bounds:u32,
      camera_pos:glam::Vec3,
      basis: [glam::Vec3; 3],
      aspect_ratio: f32,
      fov: f32,
    ) -> Self {
    Self {
      obj_head,
      obj_bounds,
      cam_aspect: aspect_ratio,
      cam_tan_fov: (fov / 2.).tan(),
      cam_cell: camera_pos.floor().as_ivec3().into(),
      padding1: 0.,
      cam_offset: camera_pos.fract_gl().into(),
      padding2: 0.,
      cam_forward: basis[2].into(),
      padding3: 0.,
      cam_right: basis[0].into(),
      padding4: 0.,
      cam_up: basis[1].into(),
      padding5: 0.,
    }
  } 
}

pub struct WgpuCtx<'window> {
  surface: wgpu::Surface<'window>,
  surface_config: wgpu::SurfaceConfiguration,
  device: wgpu::Device,
  queue: wgpu::Queue,

  data_buffer: wgpu::Buffer,
  voxel_buffer: wgpu::Buffer,

  compute_bgl: wgpu::BindGroupLayout,
  compute_pipeline: wgpu::ComputePipeline,
  compute_bind_group: wgpu::BindGroup,
  render_bgl: wgpu::BindGroupLayout,
  render_pipeline: wgpu::RenderPipeline,
  render_bind_group: wgpu::BindGroup,
  sampler: wgpu::Sampler,
}

impl<'window> WgpuCtx<'window> {

  fn new_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
      device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Storage Texture"),
        size: wgpu::Extent3d { width: width / DOWNSCALE, height: height / DOWNSCALE, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
      })
  }

  async fn new_async(window: Arc<Window>) -> WgpuCtx<'window> {
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(Arc::clone(&window)).unwrap();
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
      compatible_surface: Some(&surface),
      ..Default::default()
    }).await.unwrap();
    let (device, queue) = adapter.request_device(&Default::default()).await.unwrap();

    let size = window.inner_size();
    let width = size.width.max(1);
    let height = size.height.max(1);
    let surface_config = surface.get_default_config(&adapter, width, height).unwrap();
    surface.configure(&device, &surface_config);
    
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    let compute_texture = Self::new_texture(&device, width, height);
    let compute_view = compute_texture.create_view(&Default::default());

    let compute_shader = device.create_shader_module(wgpu::include_wgsl!("compute.wgsl"));
    let compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("Compute BGL"),
      entries: &[
        // Output Texture
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
        // Data Buffer
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
        // Voxel Storage Buffer
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
      ],
    });

    // Stores Data {..}
    let data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("Data Buffer"),
      size: 96,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });
    // Stores BasicNode {...}s
    let voxel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("Voxel Buffer"),
      // 32mb
      size: 64_000_000,
      usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false
    });

    let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      layout: &compute_bgl,
      entries: &[
        wgpu::BindGroupEntry {
          binding: 0,
          resource: wgpu::BindingResource::TextureView(&compute_view),
        },
        wgpu::BindGroupEntry {
          binding: 1,
          resource: data_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
          binding: 2,
          resource: voxel_buffer.as_entire_binding(),
        },
      ],
      label: Some("Compute BG"),
    });
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
      layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute Layout"),
        bind_group_layouts: &[&compute_bgl],
        push_constant_ranges: &[]
      })),
      cache: None,
      compilation_options: wgpu::PipelineCompilationOptions::default(),
      module: &compute_shader,
      entry_point: Some("main"),
      label: Some("Compute Pipeline")
    });


    let render_module = device.create_shader_module(wgpu::include_wgsl!("render.wgsl"));
    let render_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: Some("Render BGL"),
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
    let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      layout: &render_bgl,
      entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&compute_view) },
        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
      ],
      label: Some("Render BG"),
    });
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("Render Pipeline"),
      layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Layout"),
        bind_group_layouts: &[&render_bgl],
        push_constant_ranges: &[],
      })),
      cache: None,
      vertex: wgpu::VertexState {
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        module: &render_module,
        entry_point: Some("vs_main"),
        buffers: &[],
      },
      fragment: Some(wgpu::FragmentState {
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        module: &render_module,
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


    WgpuCtx {
      surface,
      surface_config,
      device,
      queue,

      data_buffer,
      voxel_buffer,

      compute_bgl,
      compute_pipeline,
      compute_bind_group,
      render_bgl,
      render_pipeline,
      render_bind_group,
      sampler,
    }
  }

  pub fn new(window: Arc<Window>) -> WgpuCtx<'window> { pollster::block_on(WgpuCtx::new_async(window)) }

  // Windows sets window size to (0,0) when minimized, so we need a minimize check somewhere
  pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
    self.surface_config.width = new_size.width;
    self.surface_config.height = new_size.height;
    self.surface.configure(&self.device, &self.surface_config);
    let compute_texture = Self::new_texture(&self.device, new_size.width, new_size.height);
    let compute_view = compute_texture.create_view(&Default::default());

    self.compute_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
      layout: &self.compute_bgl,
      entries: &[
        wgpu::BindGroupEntry {
          binding: 0,
          resource: wgpu::BindingResource::TextureView(&compute_view),
        },
        wgpu::BindGroupEntry {
          binding: 1,
          resource: wgpu::BindingResource::Buffer(self.data_buffer.as_entire_buffer_binding()),
        },
        wgpu::BindGroupEntry {
          binding: 2,
          resource: wgpu::BindingResource::Buffer(self.voxel_buffer.as_entire_buffer_binding()),
        },
      ],
      label: Some("Compute BG"),
    });

    self.render_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
      layout: &self.render_bgl,
      entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&compute_view) },
        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
      ],
      label: Some("Render BG"),
    });
  }

  pub fn update_voxels(&self, sdg:&SparseDirectedGraph<BasicNode3d>) {
    let voxels = sdg.nodes.safe_data();
    let safe_data: Vec<BasicNode3d> = voxels.iter().map(|node| {
      match node {
        Some(thing) => { **thing }
        None => { [u32::MAX; 8] } // This is trechnically wrong, officially I should be using
                                  // BasicNode3d::new(&vec![u32::MAX; BasicNode3d::Size]) or
                                  // whatever, but that's a massive pain
      }
    }).collect();
    self.queue.write_buffer(&self.voxel_buffer, 0, bytemuck::cast_slice(&safe_data));
  }
  
  pub fn draw(&mut self, game_data: &GameData) {
    let (width, height) = (self.surface_config.width, self.surface_config.height);
    let frame = self.surface.get_current_texture().unwrap();
    let view = frame.texture.create_view(&Default::default());
    let mut encoder = self.device.create_command_encoder(&Default::default());

    let data = Data::new(
      game_data.obj_data.head,
      game_data.obj_data.bounds,
      game_data.camera.position,
      game_data.camera.basis(),
      game_data.camera.aspect_ratio,
      game_data.camera.fov,
    );
    self.queue.write_buffer(&self.data_buffer, 0, bytemuck::cast_slice(&[data]));

    let mut compute_pass = encoder.begin_compute_pass(&Default::default());
    compute_pass.set_pipeline(&self.compute_pipeline);
    compute_pass.set_bind_group(0, &self.compute_bind_group, &[],);
    // This is ugly
    compute_pass.dispatch_workgroups(
      (width / DOWNSCALE + WORKGROUP_SQUARE - 1) / WORKGROUP_SQUARE,
      (height / DOWNSCALE + WORKGROUP_SQUARE - 1) / WORKGROUP_SQUARE,
      1
    );
    drop(compute_pass);

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      label: Some("Render Pass"),
      color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        view: &view,
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
    render_pass.set_pipeline(&self.render_pipeline);
    render_pass.set_bind_group(0, &self.render_bind_group, &[]);
    render_pass.draw(0..3, 0..1);
    drop(render_pass);
    self.queue.submit(Some(encoder.finish()));
    frame.present();
  }
}

