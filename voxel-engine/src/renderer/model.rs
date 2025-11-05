use std::ops::Range;
use std::path::Path;
use glam::{Vec2, Vec3};
use wgpu::{BindGroupLayout, BufferUsages, Device, IndexFormat, Queue, RenderPass};
use crate::renderer::buffer::Buffer;
use crate::renderer::buffer_size_of;
use crate::renderer::texture::Texture;
use anyhow::{ensure, Context, Result};

// model.rs
pub trait VertexComponent {
    const DESC: wgpu::VertexBufferLayout<'static>;
}

#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct ModelVertex {
    pub position: Vec3,
    pub tex_coords: Vec2,
    pub normal: Vec3,
}

impl VertexComponent for ModelVertex {
    const DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: buffer_size_of::<Self>(),
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &const { wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3] },
    };
}

pub struct Material {
    pub bind_group: wgpu::BindGroup,
}

pub struct Mesh {
    pub vertex_buffer: Buffer<ModelVertex>,
    pub index_buffer: Buffer<u32>,
    pub material: usize,
}

pub struct Model {
    pub meshes: Box<[Mesh]>,
    pub materials: Box<[Material]>,
}

impl Model {
    fn load_inner(file_name: &Path, device: &Device, queue: &Queue, layout: &BindGroupLayout) -> Result<Self> {
        let (models, materials) = tobj::load_obj(file_name, &tobj::GPU_LOAD_OPTIONS)?;
        let parent_file = file_name.parent();
        
        let materials = materials?.into_iter().map(|material| {
            let texture_file = material.diffuse_texture.context("no texture file found in material")?;
            
            let owned_path;
            let path = match parent_file {
                None => file_name,
                Some(parent) => {
                    owned_path = parent.join(texture_file);
                    &owned_path
                }
            };
            
            let diffuse_texture = Texture::from_file(device, queue, path)?;
            
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                    },
                ],
                label: Some(&material.name),
            });

            Ok(Material {
                bind_group,
            })
        }).collect::<Result<Box<[_]>>>()?;

        let meshes = models
            .into_iter()
            .map(|model| {
                let positions = bytemuck::try_cast_slice::<f32, Vec3>(&model.mesh.positions)
                    .ok()
                    .context("invalid positions decoded, form needs to be in  [x, y, z]")?;
                
                let tex_coords = bytemuck::try_cast_slice::<f32, Vec2>(&model.mesh.texcoords)
                    .ok()
                    .context("invalid texture coordinates decoded, form needs to be in [x, y]")?;
                
                let normals = bytemuck::try_cast_slice::<f32, Vec3>(&model.mesh.normals)
                    .ok()
                    .context("invalid mesh normals decoded, form needs to be in [x, y, z]")?;
                
                ensure!(
                    tex_coords.len() == positions.len(),
                    "expected {vertex_count} texture coordinates found {texture_count}, malformed obj file",
                    vertex_count = positions.len(),
                    texture_count = tex_coords.len()
                );
                
                ensure!(
                    normals.is_empty() || normals.len() == positions.len(),
                    "expected either no normals, or {vertex_count} normals but found {normal_count}, malformed obj file",
                    vertex_count = positions.len(),
                    normal_count = normals.len()
                );
                
                let iter = positions.iter().copied().zip(tex_coords.iter().copied());
                
                let vertices = match normals.is_empty() {
                    true => iter.map(|(position, tex_coords)| ModelVertex {
                        position,
                        tex_coords,
                        normal: Vec3::ZERO,
                    }).collect::<Vec<_>>(),
                    false => iter.zip(normals.iter().copied()).map(|((position, tex_coords), normal)| ModelVertex {
                        position,
                        tex_coords,
                        normal,
                    }).collect::<Vec<_>>()
                }; 

                let vertex_buffer = Buffer::with_init(
                    device,
                    &vertices,
                    BufferUsages::VERTEX,
                    Some(&format!("{:?} vertex buffer", file_name))
                );
                
                let index_buffer = Buffer::with_init(
                    device,
                    &model.mesh.indices,
                    BufferUsages::INDEX,
                    Some(&format!("{:?} index buffer", file_name))
                );

                Ok(Mesh {
                    vertex_buffer,
                    index_buffer,
                    material: model.mesh.material_id.context("no material found for model")?,
                })
            })
            .collect::<Result<Box<[_]>>>()?;
        
        Ok(Self { meshes, materials })
    }
    
    pub fn load<P: AsRef<Path>>(file_name: P, device: &Device, queue: &Queue, layout: &BindGroupLayout) -> Result<Self> {
        Self::load_inner(file_name.as_ref(), device, queue, layout)
    }
}


pub trait DrawObjExt<T> {
    fn draw_obj_instanced(&mut self, obj: &T, range: Range<u32>);
}


pub trait DrawLightExt<T> {
    fn draw_light_instanced(&mut self, obj: &T, range: Range<u32>);
}

impl DrawObjExt<(&Mesh, &Material)> for RenderPass<'_> {
    fn draw_obj_instanced(&mut self, &(mesh, material): &(&Mesh, &Material), range: Range<u32>) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_bind_group(0, &material.bind_group, &[]);
        self.set_index_buffer(mesh.index_buffer.slice(..), IndexFormat::Uint32);
        self.draw_indexed(0..mesh.index_buffer.len_u32(), 0, range);
    }
}


impl DrawLightExt<Mesh> for RenderPass<'_> {
    fn draw_light_instanced(&mut self, mesh: &Mesh, range: Range<u32>) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), IndexFormat::Uint32);
        self.draw_indexed(0..mesh.index_buffer.len_u32(), 0, range);
    }
}

impl DrawObjExt<Model> for RenderPass<'_> {
    fn draw_obj_instanced(&mut self, model: &Model, range: Range<u32>) {
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            self.draw_obj_instanced(&(mesh, material), range.clone())
        }
    }
}

impl DrawLightExt<Model> for RenderPass<'_> {
    fn draw_light_instanced(&mut self, model: &Model, range: Range<u32>) {
        for mesh in &model.meshes {
            self.draw_light_instanced(mesh, range.clone())
        }
    }
}
