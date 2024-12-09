use std::{
    any::{type_name, TypeId},
    mem::{align_of, size_of, MaybeUninit},
    slice,
    sync::{Arc, Mutex},
};

use bevy::{
    color::ColorToComponents,
    math::Vec4,
    prelude::Component,
    render::{
        mesh::VertexBufferLayout,
        render_resource::{VertexAttribute, VertexFormat, VertexStepMode},
    },
};
use bytemuck::{Pod, Zeroable};

use crate::{Projectile, ProjectileInstanceBuffer};

fn validate<T>() {
    if !matches!(align_of::<T>(), 1 | 2 | 4 | 8 | 16) {
        panic!("Bad alignment for {}.", type_name::<T>())
    }
}

/// Strategy for cleaning up particle buffers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParticleBufferStrategy {
    /// Move alive particles to the start of the buffer.
    #[default]
    Retain,
    /// Ignores dead particles when iterating.
    ///
    /// Should only be used if lifetimes of particles are constant,
    /// and capacity is well predicted.
    RingBuffer,
}

#[doc(hidden)]
/// [`MaybeUninit`] with alignment and size `16`.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
pub struct Align16MaybeUninit(MaybeUninit<[u8; 16]>);

impl Align16MaybeUninit {
    pub const fn uninit() -> Self {
        Align16MaybeUninit(MaybeUninit::uninit())
    }
}

impl Default for Align16MaybeUninit {
    fn default() -> Self {
        Self(MaybeUninit::uninit())
    }
}

/// Instance buffer of a particle.
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct DefaultInstanceBuffer {
    pub index: u32,
    pub lifetime: f32,
    pub fac: f32,
    pub seed: f32,
    pub transform_x: Vec4,
    pub transform_y: Vec4,
    pub transform_z: Vec4,
    pub color: Vec4,
}

impl<T: Projectile> From<&T> for DefaultInstanceBuffer {
    fn from(x: &T) -> Self {
        let transform = x.get_transform().compute_matrix();
        DefaultInstanceBuffer {
            index: x.get_index(),
            lifetime: x.get_lifetime(),
            seed: x.get_seed(),
            fac: x.get_fac(),
            color: x.get_color().to_vec4(),
            transform_x: transform.row(0),
            transform_y: transform.row(1),
            transform_z: transform.row(2),
        }
    }
}

impl ProjectileInstanceBuffer for DefaultInstanceBuffer {
    fn descriptor() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: size_of::<DefaultInstanceBuffer>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: 0,
                    shader_location: 10,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 4,
                    shader_location: 11,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 8,
                    shader_location: 12,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 12,
                    shader_location: 13,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 14,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 15,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 48,
                    shader_location: 16,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 64,
                    shader_location: 17,
                },
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExtractedParticleBuffer(pub(crate) Arc<ErasedExtractBuffer>);

#[derive(Debug, Clone, Default)]
pub struct ErasedExtractBuffer {
    pub bytes: Vec<u8>,
    pub len: usize,
}

impl ExtractedParticleBuffer {
    pub fn is_empty(&self) -> bool {
        self.0.len == 0
    }

    pub fn len(&self) -> usize {
        self.0.len
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0.bytes
    }
}

/// Type of particle buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParticleBufferType {
    #[default]
    Uninit,
    Retain(TypeId),
    RingBuffer(TypeId),
}

/// Type erased buffer for particles.
#[derive(Debug, Component, Default)]
pub struct ProjectileBuffer {
    /// Type of particle, for safety checks.
    pub(crate) particle_type: ParticleBufferType,
    /// Allocated buffer.
    pub(crate) buffer: Box<[Align16MaybeUninit]>,
    /// Tracks number of particles.
    pub(crate) len: usize,
    /// Maximum number of particles possible.
    pub(crate) capacity: usize,
    /// Ring: points to insertion point.
    pub(crate) ptr: usize,
    /// Ring: number of particles initialized, never goes down.
    pub(crate) ring_capacity: usize,
    /// Allocation of extracted particles on the render world.
    pub(crate) extracted_allocation: Mutex<Arc<ErasedExtractBuffer>>,
}

impl ProjectileBuffer {
    /// Return `true` if buffer is uninitialized, usually created by `default()`.
    pub const fn is_uninit(&self) -> bool {
        matches!(self.particle_type, ParticleBufferType::Uninit)
    }

    /// Returns `true` if no particle is alive.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Create a buffer in retain mode.
    pub fn new_retain<T: Projectile>(nominal_capacity: usize) -> Self {
        validate::<T>();
        let real_capacity = (nominal_capacity * size_of::<T>() + 15) / 16;
        let capacity = real_capacity * 16 / size_of::<T>();
        Self {
            particle_type: ParticleBufferType::Retain(TypeId::of::<T>()),
            buffer: vec![Align16MaybeUninit::uninit(); real_capacity].into(),
            len: 0,
            capacity,
            ptr: 0,
            ring_capacity: 0,
            extracted_allocation: Default::default(),
        }
    }

    /// Create a buffer in ring buffer mode.
    pub fn new_ring<T: Projectile>(nominal_capacity: usize) -> Self {
        validate::<T>();
        let real_capacity = (nominal_capacity * size_of::<T>() + 15) / 16;
        let capacity = real_capacity * 16 / size_of::<T>();
        Self {
            particle_type: ParticleBufferType::RingBuffer(TypeId::of::<T>()),
            buffer: vec![Align16MaybeUninit::uninit(); real_capacity].into(),
            len: 0,
            capacity,
            ptr: 0,
            ring_capacity: 0,
            extracted_allocation: Default::default(),
        }
    }

    /// If in `retain` mode, returns `[..len]`,  if in `ring` mode, returns `[..ring_capacity]`.
    ///
    /// # Panics
    ///
    /// If type mismatch or in `uninit` mode.
    pub fn get<T: Projectile>(&self) -> &[T] {
        match self.particle_type {
            ParticleBufferType::Uninit => panic!("Type ID mismatch!"),
            ParticleBufferType::Retain(id) => {
                if id != TypeId::of::<T>() {
                    panic!("Type ID mismatch!")
                }
                unsafe { slice::from_raw_parts(self.buffer.as_ptr() as *const T, self.len) }
            }
            ParticleBufferType::RingBuffer(id) => {
                if id != TypeId::of::<T>() {
                    panic!("Type ID mismatch!")
                }
                unsafe {
                    slice::from_raw_parts(self.buffer.as_ptr() as *const T, self.ring_capacity)
                }
            }
        }
    }

    /// If in `retain` mode, returns `[..len]`,  if in `ring` mode, returns `[..ring_capacity]`.
    ///
    /// # Panics
    ///
    /// If type mismatch or in `uninit` mode.
    pub fn get_mut<T: Projectile>(&mut self) -> &mut [T] {
        match self.particle_type {
            ParticleBufferType::Uninit => panic!("Type ID mismatch!"),
            ParticleBufferType::Retain(id) => {
                if id != TypeId::of::<T>() {
                    panic!("Type ID mismatch!")
                }
                unsafe { slice::from_raw_parts_mut(self.buffer.as_mut_ptr() as *mut T, self.len) }
            }
            ParticleBufferType::RingBuffer(id) => {
                if id != TypeId::of::<T>() {
                    panic!("Type ID mismatch!")
                }
                unsafe {
                    slice::from_raw_parts_mut(
                        self.buffer.as_mut_ptr() as *mut T,
                        self.ring_capacity,
                    )
                }
            }
        }
    }

    /// Extends items into the buffer, overflow will be discarded.
    ///
    /// # Panics
    ///
    /// If type mismatch or in `uninit` mode.
    pub fn extend<T: Projectile>(&mut self, ext: impl IntoIterator<Item = T>) {
        match self.particle_type {
            ParticleBufferType::Uninit => panic!("Type ID mismatch!"),
            ParticleBufferType::Retain(id) => {
                if id != TypeId::of::<T>() {
                    panic!("Type ID mismatch!")
                }
                let slice = unsafe {
                    slice::from_raw_parts_mut(
                        self.buffer.as_mut_ptr() as *mut MaybeUninit<T>,
                        self.capacity,
                    )
                };
                for item in ext {
                    if self.len >= slice.len() {
                        continue;
                    }
                    slice[self.len] = MaybeUninit::new(item);
                    self.len += 1;
                }
            }
            ParticleBufferType::RingBuffer(id) => {
                if id != TypeId::of::<T>() {
                    panic!("Type ID mismatch!")
                }
                let slice = unsafe {
                    slice::from_raw_parts_mut(
                        self.buffer.as_mut_ptr() as *mut MaybeUninit<T>,
                        self.capacity,
                    )
                };
                for item in ext {
                    if self.len == self.capacity {
                        continue;
                    }
                    if self.ring_capacity < slice.len() && self.ptr > self.ring_capacity {
                        self.ring_capacity += 1;
                    }
                    slice[self.ptr] = MaybeUninit::new(item);
                    self.ptr = (self.ptr + 1) % slice.len();
                }
            }
        }
    }
}
