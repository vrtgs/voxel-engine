use std::num::NonZero;
use std::sync::Arc;
use bytemuck::{Pod, Zeroable};
use glam::{vec3a, Mat4, Quat, Vec3, Vec3A};
use wgpu::{Instance as WGPUInstance, Device, DeviceDescriptor, MemoryHints, PowerPreference, Queue, RequestAdapterOptions, Surface, TextureFormat, Trace, InstanceDescriptor, SurfaceConfiguration, TextureUsages, CompositeAlphaMode, PresentMode, TextureViewDescriptor, Operations, RenderPassColorAttachment, LoadOp, StoreOp, RenderPassDescriptor, BufferAddress, BufferUsages, BindGroup, CommandEncoder, VertexBufferLayout};
use wgpu::util::StagingBelt;
use winit::window::Window;
use voxel_maths::Transform;
use crate::game_state::GameState;
use crate::renderer::buffer::Buffer;
use crate::renderer::camera::{Camera, Projection};
use crate::renderer::model::{DrawObjExt, Model, ModelVertex, VertexComponent};
use crate::renderer::texture::Texture;
use crate::settings::{GameSettings, GameSettingsHandle, Vsync};

mod texture;
mod buffer;
mod camera;

pub mod model;

const fn buffer_size_of<T>() -> BufferAddress {
    const {
        let addr = size_of::<T>();
        if addr as BufferAddress as usize != addr {
            panic!("invalid size of buffer, struct too large to fit in GPU memory")
        }
        
        addr as BufferAddress
    }
}

macro_rules! buffer_size_of {
    ($t:ty) => {
        const { buffer_size_of::<$t>() }
    };
}


pub(super) struct Renderer {
    window: Arc<Window>,
    settings: GameSettingsHandle,
    device: Device,
    queue: Queue,
    size: winit::dpi::PhysicalSize<u32>,
    surface: Surface<'static>,
    surface_format: TextureFormat,
    render_pipeline: wgpu::RenderPipeline,
    staging_belt: StagingBelt,
    projection: Projection,
    last_camera_uniform: CameraUniform,
    camera_buffer: Buffer<CameraUniform>,
    camera_bind_group: BindGroup,
    depth_texture: Texture,
    
    model: Model,
    instance_buffer: Buffer<InstanceRaw>
}

#[derive(Copy, Clone)]
struct Instance(Transform);

impl Instance {
    fn to_raw(self) -> InstanceRaw {
        InstanceRaw(Mat4::from_rotation_translation(
            self.0.rotation,
            self.0.position.into()
        ))
    }
}


#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(transparent)]
struct InstanceRaw(Mat4);

impl VertexComponent for InstanceRaw {
    const DESC: VertexBufferLayout<'static> = VertexBufferLayout {
        array_stride: buffer_size_of!(InstanceRaw),
        // We need to switch from using a step mode of Vertex to Instance
        // This means that our shaders will only change to use the next
        // instance when the shader starts processing a new instance
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
            // for each vec4. We'll have to reassemble the mat4 in the shader.
            wgpu::VertexAttribute {
                offset: 0,
                // While our vertex shader only uses locations 0, and 1 now, in later tutorials, we'll
                // be using 2, 3, and 4, for Vertex. We'll start at slot 5, not conflict with them later
                shader_location: 5,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: buffer_size_of!([f32; 4]),
                shader_location: 6,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: buffer_size_of!([f32; 8]),
                shader_location: 7,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: buffer_size_of!([f32; 12]),
                shader_location: 8,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };
}


#[derive(Copy, Clone, Pod, Zeroable, PartialEq)]
#[repr(transparent)]
struct CameraUniform {
    view_proj: Mat4
}

impl CameraUniform {
    fn new(camera: &Camera, projection: &Projection) -> Self {
        let view_proj = projection.calc_matrix() * camera.calc_matrix();
        Self { view_proj }
    }
}


impl Renderer {
    pub async fn new(window: Arc<Window>, settings: GameSettingsHandle) -> Renderer {
        let instance = WGPUInstance::new(&InstanceDescriptor::from_env_or_default());

        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                required_features: adapter.features(),
                required_limits: adapter.limits(),
                label: Some("game window"),
                memory_hints: MemoryHints::default(),
                trace: Trace::Off,
            },)
            .await
            .unwrap();


        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps.formats.iter()
            .find(|&&f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);


        let size = window.inner_size();

        let loaded_settings = settings.load();
        let projection = Projection::new(
            size.width,
            size.height,
            loaded_settings.fov
        );
        let config = Self::make_config_with_settings(&loaded_settings, size, surface_format);
        drop(loaded_settings);
        surface.configure(&device, &config);
        
        let depth_texture = Texture::create_depth_texture(&device, &config, "depth texture");
        
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });
        
        let camera_uniform = CameraUniform {
            view_proj: Mat4::ZERO
        };
        let camera_buffer = Buffer::with_init(
            &device,
            &[camera_uniform],
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            Some("camera buffer")
        );

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZero::new(size_of::<CameraUniform>() as BufferAddress),
                    },
                    count: None,
                }
            ],
            label: Some("camera bind group layout"),
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }
            ],
            label: Some("camera_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("./shaders/main_shader.wgsl"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &camera_bind_group_layout
                ],
                push_constant_ranges: &[],
            });


        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"), // 1.
                buffers: &[ModelVertex::DESC, InstanceRaw::DESC], // 2.
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState { // 3.
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { // 4.
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw, // 2.
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default()
            }), 
            multisample: wgpu::MultisampleState {
                count: 1, // 2. We dont currently need multi sampling
                mask: !0, // 3. using all samples
                alpha_to_coverage_enabled: false, //  has to do with anti-aliasing.  We're not doing that yet
            },
            multiview: None, // 5. no textures yet
            cache: None, // 6. allows wgpu to cache shader compilation data. Only really useful for Android build targets.
        });

        
        const STAGING_BELT_SIZE: BufferAddress = 64 * 1024 * 1024; // 64 Mib


        const NUM_INSTANCES_PER_ROW: u32 = 10;

        const SPACE_BETWEEN: f32 = 3.0;
        let instances = (0..NUM_INSTANCES_PER_ROW).flat_map(|z| {
            (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

                let position = vec3a(x, 0.0, z);

                let rotation = if position.cmpeq(Vec3A::ZERO).all() {
                    Quat::from_axis_angle(Vec3::Z, 0.0)
                } else {
                    Quat::from_axis_angle(position.normalize().into(), 45.0_f32.to_radians())
                };

                Instance(Transform {
                    position,
                    rotation
                })
            })
        }).collect::<Vec<_>>();


        let instance_buffer = Buffer::with_init(
            &device,
            // TODO: get rid of collect and collect directly into buffer
            &instances.iter().map(|instance: &Instance| instance.to_raw()).collect::<Vec<_>>(),
            BufferUsages::VERTEX,
            Some("instance buffer")
        );

        let model = Model::load(
            "./voxel-engine/assets/cube/cube.obj",
            &device,
            &queue,
            &texture_bind_group_layout
        ).unwrap();
        
        Renderer {
            settings,
            window,
            device,
            queue,
            size,
            surface,
            surface_format,
            render_pipeline,
            staging_belt: StagingBelt::new(STAGING_BELT_SIZE),
            projection,
            last_camera_uniform: camera_uniform,
            camera_buffer,
            camera_bind_group,
            depth_texture,
            
            model,
            instance_buffer,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn make_config_with_settings(
        settings: &GameSettings,
        size: winit::dpi::PhysicalSize<u32>,
        surface_format: TextureFormat
    ) -> SurfaceConfiguration {
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            view_formats: vec![surface_format.add_srgb_suffix()],
            alpha_mode: CompositeAlphaMode::Auto,
            width: size.width,
            height: size.height,
            desired_maximum_frame_latency: 2,
            present_mode: match settings.vsync {
                Vsync::On => PresentMode::AutoVsync,
                Vsync::Off => PresentMode::AutoNoVsync
            },
        };

        tracing::info!("new surface {:#?}", surface_config);
        surface_config
    }
    
    fn render_camera(&mut self, camera: Camera, encoder: &mut CommandEncoder) {
        let new_uniform = CameraUniform::new(
            &camera,
            &self.projection
        );

        if new_uniform != self.last_camera_uniform {
            self.last_camera_uniform = new_uniform;
            self.camera_buffer.write(
                &mut self.staging_belt,
                encoder,
                &self.device,
                std::slice::from_ref(&new_uniform)
            );
        }
    }
    
    pub fn reconfigure(&mut self) {
        let settings = self.settings.load();
        let config = Self::make_config_with_settings(&settings, self.size, self.surface_format);
        self.surface.configure(&self.device, &config);
        self.depth_texture = Texture::create_depth_texture(&self.device, &config, "depth texture");
        self.projection.resize(self.size.width, self.size.height);
        self.projection.change_fov(settings.fov);
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.reconfigure();
    }

    pub fn render(&mut self, game: &GameState) {
        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("failed to acquire next swap-chain texture");

        let texture_view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor {
                // Without add_srgb_suffix() the image we will be working with
                // might not be "gamma correct".
                format: Some(self.surface_format.add_srgb_suffix()),
                ..Default::default()
            });

        
        let camera = Camera::new(game.player());
        
        let mut encoder = self.device.create_command_encoder(&Default::default());       
        self.render_camera(camera, &mut encoder);
        
        {
            // we need the render pass to drop before we can move out of encoder
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &texture_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(game.background_color()),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store
                    }),
                    stencil_ops: None
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.draw_obj_instanced(&self.model, 0..self.instance_buffer.len_u32())
        }

        // Submit the command in the queue to execute
        self.staging_belt.finish();
        self.queue.submit(std::iter::once(encoder.finish()));
        self.staging_belt.recall();
        
        self.window.pre_present_notify();
        surface_texture.present();
    }
}