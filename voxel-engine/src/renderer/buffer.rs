use std::marker::PhantomData;
use std::num::NonZero;
use std::ops::{Deref, DerefMut};
use bytemuck::Pod;
use wgpu::{BufferAddress, BufferSize, BufferSlice, BufferUsages, CommandEncoder, Device};
use wgpu::util::{DeviceExt, StagingBelt};

pub struct Buffer<T> {
    gpu_buffer: wgpu::Buffer,
    _marker: PhantomData<[T]>
}

impl<T: Pod> Buffer<T> {
    pub fn with_init(device: &Device, data: &[T], usage: BufferUsages, label: Option<&str>) -> Self {
        let gpu_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::must_cast_slice(data),
                usage,
            }
        );

        Buffer {
            gpu_buffer,
            _marker: PhantomData
        }
    }
    
    #[expect(dead_code, reason = "still kinda hard coding things")]
    pub fn new(device: &Device, size: BufferAddress, usage: BufferUsages, label: Option<&str>) -> Self {
        let gpu_buffer = device.create_buffer(
            &wgpu::BufferDescriptor {
                label,
                usage,
                size,
                mapped_at_creation: false,
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


    fn prep_send<'s>(&mut self, staging_belt: &'s mut StagingBelt, encoder: &mut CommandEncoder, device: &Device) -> Option<BufferSlice<'s>> {
        let buffer_size = NonZero::new(self.gpu_buffer.size())?;
        let slice_of_belt = staging_belt.allocate(
            buffer_size,
            const { BufferSize::new(align_of::<T>() as u64).unwrap() },
            device,
        );
        encoder.copy_buffer_to_buffer(
            slice_of_belt.buffer(),
            slice_of_belt.offset(),
            &self.gpu_buffer,
            0,
            buffer_size.get(),
        );

        Some(slice_of_belt) 
    }
    
    pub fn write(&mut self, staging_belt: &mut StagingBelt, encoder: &mut CommandEncoder, device: &Device, data: &[T]) {
        let Some(buffer) = self.prep_send(staging_belt, encoder, device) else {
            assert!(data.is_empty(), "buffer length mismatch");
            return;
        };
        
        let mut view = buffer.get_mapped_range_mut();
        let dst = &mut *view;
        match bytemuck::try_cast_slice_mut::<u8, T>(dst) {
            Ok(dst) => dst.copy_from_slice(data),
            Err(_) => dst.copy_from_slice(bytemuck::must_cast_slice(data))
        }
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

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        self.gpu_buffer.destroy()
    }
}