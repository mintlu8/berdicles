use std::{
    any::{type_name, Any, TypeId},
    mem::{align_of, size_of, MaybeUninit},
    ops::Range,
    slice,
    sync::{Arc, Mutex},
};

use bevy::{math::Vec4, prelude::Component};
use bytemuck::{Pod, Zeroable};

use crate::{
    trail::{TrailBuffer, TrailedParticle},
    Particle,
};

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
pub struct ExtractedParticle {
    pub index: u32,
    pub lifetime: f32,
    pub fac: f32,
    pub seed: f32,
    pub transform_x: Vec4,
    pub transform_y: Vec4,
    pub transform_z: Vec4,
    pub color: Vec4,
}

#[derive(Debug, Clone, Component)]
pub(crate) struct ExtractedParticleBuffer(pub(crate) Arc<Vec<ExtractedParticle>>);

impl ExtractedParticleBuffer {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.0.as_ref())
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
pub struct ParticleBuffer {
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
    pub(crate) extracted_allocation: Mutex<Arc<Vec<ExtractedParticle>>>,
    /// Type of this should be `Vec<Trail>`.
    pub(crate) detached_trails: Option<Box<dyn Any + Send + Sync>>,
}

impl ParticleBuffer {
    /// Return `true` if buffer is uninitialized, usually created by `default()`.
    pub const fn is_uninit(&self) -> bool {
        matches!(self.particle_type, ParticleBufferType::Uninit)
    }

    /// Returns `true` if no particle is alive.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Create a buffer in retain mode.
    pub fn new_retain<T: Particle>(nominal_capacity: usize) -> Self {
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
            detached_trails: None,
        }
    }

    /// Create a buffer in ring buffer mode.
    pub fn new_ring<T: Particle>(nominal_capacity: usize) -> Self {
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
            detached_trails: None,
        }
    }

    /// If in `retain` mode, returns `[..len]`,  if in `ring` mode, returns `[..ring_capacity]`.
    ///
    /// # Panics
    ///
    /// If type mismatch or in `uninit` mode.
    pub fn get<T: Particle>(&self) -> &[T] {
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
    pub fn get_mut<T: Particle>(&mut self) -> &mut [T] {
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
    pub fn extend<T: Particle>(&mut self, ext: impl IntoIterator<Item = T>) {
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

    /// Returns a reference to detached curves.
    pub fn detached<T: TrailBuffer>(&self) -> Option<&[T]> {
        self.detached_trails
            .as_ref()
            .and_then(|x| x.downcast_ref::<Vec<T>>())
            .map(|x| x.as_ref())
    }


    /// Returns a reference to detached curves.
    pub fn detached_mut<T: TrailBuffer>(&mut self) -> Option<&mut [T]> {
        self.detached_trails
            .as_mut()
            .and_then(|x| x.downcast_mut::<Vec<T>>())
            .map(|x| x.as_mut())
    }

    /// Detach a slice of particles into trail rendering.
    pub fn detach_slice<T: TrailedParticle>(&mut self, slice: Range<usize>) {
        let buf = match self.particle_type {
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
        };
        if let Some(trails) = self
            .detached_trails
            .as_mut()
            .and_then(|x| x.downcast_mut::<Vec<T::TrailBuffer>>())
        {
            trails.extend(buf[slice].iter().map(|x| x.as_trail_buffer()));
        } else {
            self.detached_trails = Some(Box::new(Vec::from_iter(
                buf[slice].iter().map(|x| x.as_trail_buffer()),
            )))
        }
    }

    /// Update detached trails, this must be added manually to `on_update` of a `ParticleSystem` if needed.
    pub fn update_detached<T: TrailedParticle>(&mut self, dt: f32) {
        if let Some(trails) = self
            .detached_trails
            .as_mut()
            .and_then(|x| x.downcast_mut::<Vec<T::TrailBuffer>>())
        {
            trails.retain_mut(|x| {
                x.update(dt);
                !x.expired()
            })
        }
    }
}
