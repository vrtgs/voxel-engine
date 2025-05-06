use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use bytemuck::Pod;
use wgpu::{BufferAddress, BufferUsages, Device, Queue};
use wgpu::util::DeviceExt;

pub struct Buffer<T> {
    gpu_buffer: wgpu::Buffer,
    _marker: PhantomData<[T]>
}

impl<T: Pod> Buffer<T> {
    pub fn new(device: &Device, data: &[T], usage: BufferUsages, label: Option<&str>) -> Self {
        let gpu_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::cast_slice(data),
                usage,
            }
        );

        Buffer {
            gpu_buffer,
            _marker: PhantomData
        }
    }

    pub fn len(&self) -> BufferAddress {
        self.gpu_buffer.size() / size_of::<T>() as BufferAddress
    }

    pub fn len_u32(&self) -> u32 {
        self.len().try_into().expect("buffer too large, cannot fit in u32")
    }


    pub fn write(&mut self, queue: &Queue, data: &[T]) {
        debug_assert_eq!(self.len(), data.len().try_into().expect("buffers not equal"));
        
        queue.write_buffer(
            &self,
            0,
            bytemuck::cast_slice(data)
        )
    }
}

impl<T> Deref for Buffer<T> {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.gpu_buffer
    }
}

impl<T> DerefMut for Buffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.gpu_buffer
    }
}
