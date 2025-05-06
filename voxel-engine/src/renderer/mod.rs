use std::num::NonZero;
use std::sync::Arc;
use bytemuck::{Pod, Zeroable};
use glam::{vec2, vec3, Mat4, Vec2, Vec3};
use wgpu::{Instance as WGPUInstance, Device, DeviceDescriptor, MemoryHints, PowerPreference, Queue, RequestAdapterOptions, Surface, TextureFormat, Trace, InstanceDescriptor, SurfaceConfiguration, TextureUsages, CompositeAlphaMode, PresentMode, TextureViewDescriptor, Operations, RenderPassColorAttachment, LoadOp, StoreOp, RenderPassDescriptor, BufferAddress, BufferUsages, BindGroup};
use winit::window::Window;
use crate::game_state::{GameState, Shape};
use crate::renderer::buffer::Buffer;
use crate::renderer::camera::Camera;
use crate::renderer::texture::include_texture;
use crate::settings::{GameSettings, GameSettingsHandle, Vsync};

mod texture;
mod buffer;
mod camera;

pub(super) struct Renderer {
    window: Arc<Window>,
    settings: GameSettingsHandle,
    device: Device,
    queue: Queue,
    size: winit::dpi::PhysicalSize<u32>,
    camera: Camera,
    surface: Surface<'static>,
    surface_format: TextureFormat,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: Buffer<Vertex>,
    pentagon_index_buffer: Buffer<u16>,
    trapezoid_index_buffer: Buffer<u16>,
    pentagon_bind_group: BindGroup,
    trapezoid_bind_group: BindGroup,
    camera_uniform: CameraUniform,
    camera_buffer: Buffer<CameraUniform>,
    camera_bind_group: BindGroup,
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct Vertex {
    position: Vec3,
    texture_coords: Vec2,
}


impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &const { wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2] },
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex { position: vec3(-0.0868241, 0.49240386, 0.0),   texture_coords: vec2(0.4131759, 0.99240386) }, // A
    Vertex { position: vec3(-0.49513406, 0.06958647, 0.0),  texture_coords: vec2(0.0048659444, 0.56958647) }, // B
    Vertex { position: vec3(-0.21918549, -0.44939706, 0.0), texture_coords: vec2(0.28081453, 0.05060294) }, // C
    Vertex { position: vec3(0.35966998, -0.3473291, 0.0),   texture_coords: vec2(0.85967, 0.1526709) }, // D
    Vertex { position: vec3(0.44147372, 0.2347359, 0.0),    texture_coords: vec2(0.9414737, 0.7347359) }, // E
];


const INDICES_PENTAGON: &[u16] = &[
    0, 1, 4,
    1, 2, 4,
    2, 3, 4,
];


const INDICES_TRAPEZOID: &[u16] = &[
    1, 2, 4,
    2, 3, 4,
];


#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct CameraUniform {
    view_proj: Mat4
}

impl CameraUniform {
    pub fn from_camera(camera: &Camera) -> Self {
        Self {
            view_proj: camera.build_view_projection_matrix()
        }
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
        let camera = Camera::new(
            Vec3::ZERO,
            Vec3::ZERO,
            size,
            loaded_settings.fov
        );
        let config = Self::make_config_with_settings(&loaded_settings, size, surface_format);
        drop(loaded_settings);
        surface.configure(&device, &config);

        let pentagon_texture = include_texture!(device, queue, "../../assets/blocks/tree.png");
        let trapezoid_texture = include_texture!(device, queue, "../../assets/icon/voxel-engine.png");

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

        let pentagon_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&pentagon_texture .view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&pentagon_texture .sampler),
                    }
                ],
                label: Some("pentagon_bind_group"),
            }
        );

        let trapezoid_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&trapezoid_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&trapezoid_texture.sampler),
                    }
                ],
                label: Some("pentagon_bind_group"),
            }
        );

        
        let camera_uniform = CameraUniform::from_camera(&camera);
        let camera_buffer = Buffer::new(
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
                buffers: &[Vertex::desc()], // 2.
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
            depth_stencil: None, // 1. `fake depth`
            multisample: wgpu::MultisampleState {
                count: 1, // 2. We dont currently need multi sampling
                mask: !0, // 3. using all samples
                alpha_to_coverage_enabled: false, //  has to do with anti-aliasing.  We're not doing that yet
            },
            multiview: None, // 5. no textures yet
            cache: None, // 6. allows wgpu to cache shader compilation data. Only really useful for Android build targets.
        });


        let vertex_buffer = Buffer::new(
            &device,
            VERTICES,
            BufferUsages::VERTEX,
            Some("vertex buffer")
        );

        let pentagon_index_buffer = Buffer::new(
            &device,
            INDICES_PENTAGON,
            BufferUsages::INDEX,
            Some("pentagon index buffer")
        );

        let trapezoid_index_buffer = Buffer::new(
            &device,
            INDICES_TRAPEZOID,
            BufferUsages::INDEX,
            Some("trapezoid index buffer")
        );


        Renderer {
            settings,
            window,
            device,
            queue,
            size,
            camera,
            surface,
            surface_format,
            render_pipeline,
            vertex_buffer,
            pentagon_index_buffer,
            trapezoid_index_buffer,
            pentagon_bind_group,
            trapezoid_bind_group,
            camera_uniform,
            camera_buffer,
            camera_bind_group
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

    fn configure_surface(&mut self, settings: &GameSettings) {
        let cfg = Self::make_config_with_settings(settings, self.size, self.surface_format);
        self.surface.configure(&self.device, &cfg);
    }

    
    fn reload_camera(&mut self) {
        let new_uniform = self.camera.build_view_projection_matrix();

        if new_uniform != self.camera_uniform.view_proj {
            self.camera_uniform.view_proj = new_uniform;
            self.camera_buffer.write(&self.queue, &[self.camera_uniform])
        }
    }

    fn reload(&mut self) {
        let settings = self.settings.load();
        self.configure_surface(&settings);
        self.camera = Camera::new(
            self.camera.eye,
            self.camera.target,
            self.size,
            settings.fov
        );
        self.reload_camera()
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.reload();
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

        let mut encoder = self.device.create_command_encoder(&Default::default());
        
        
        self.camera.update_from_player(game.transform());
        self.reload_camera();
        
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);

            let (index_buffer, bind_group) = match game.shape() {
                Shape::Pentagon => (&self.pentagon_index_buffer, &self.pentagon_bind_group),
                Shape::Trapezoid => (&self.trapezoid_index_buffer, &self.trapezoid_bind_group)
            };

            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);

            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..index_buffer.len_u32(), 0, 0..1);
        }

        // Submit the command in the queue to execute
        self.queue.submit(std::iter::once(encoder.finish()));
        self.window.pre_present_notify();
        surface_texture.present();
    }
}