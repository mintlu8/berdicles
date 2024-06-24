#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
use std::{
    any::Any, fmt::Debug, ops::{Deref, DerefMut}
};

use bevy::{
    app::{Plugin, Update},
    asset::Assets,
    color::{ColorToComponents, Srgba},
    ecs::query::QueryItem,
    math::Vec3,
    prelude::{Component, Entity, Query, Res},
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_resource::Shader,
    },
    time::Time,
    transform::components::Transform,
};
use material::StandardParticle;
use noop::NoopParticleSystem;
use pipeline::ParticleMaterialPlugin;

pub mod material;
pub mod pipeline;
pub mod shader;
mod sub;
pub use sub::*;
pub mod util;
mod buffer;
pub use buffer::*;

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.world_mut().resource_mut::<Assets<Shader>>().insert(
            &shader::PARTICLE_VERTEX,
            Shader::from_wgsl(
                shader::SHADER_VERTEX,
                "berdicle/particle_vertex.wgsl",
            ),
        );
        app.world_mut().resource_mut::<Assets<Shader>>().insert(
            &shader::PARTICLE_FRAGMENT,
            Shader::from_wgsl(
                shader::SHADER_FRAGMENT,
                "berdicle/particle_fragment.wgsl",
            ),
        );
        app.add_plugins(ExtractComponentPlugin::<ParticleInstance>::default());
        app.add_plugins(ParticleMaterialPlugin::<StandardParticle>::default());
        app.add_systems(Update, particle_system);
    }
}

pub fn particle_system(
    time: Res<Time>,
    mut particles: Query<(
        Entity,
        &mut ParticleInstance,
        &mut ParticleBuffer,
        Option<&mut ParticleEventBuffer>,
        Option<&ParticleParent>,
    )>,
) {
    let dt = time.delta_seconds();
    particles.par_iter_mut().for_each(|(_, mut system, mut buffer, events, _)| {
        if buffer.is_uninit() {
            *buffer = system.spawn_particle_buffer();
        }
        if let Some(mut events) = events {
            events.clear();
            system.update_with_buffer(dt, &mut buffer, &mut events);
        } else {
            system.update(dt, &mut buffer);
        }
    });

    // Safety: parent is checked to not be the same entity.
    for (entity, mut system, mut buffer, _, parent) in unsafe { particles.iter_unsafe() } {
        let Some(ParticleParent(parent)) = parent else {
            continue;
        };
        if entity == *parent {
            panic!("ParticleSystem's parent cannot be itself.")
        }
        if let Some(sub) = system.as_sub_particle_system() {
            // Safety: parent is checked to not be the same entity.
            let Ok((_, _, mut parent, _, _)) = (unsafe { particles.get_unchecked(*parent) }) else {
                continue;
            };
            sub.spawn_from_parent(dt, &mut buffer, &mut parent);
        }
        if let Some(sub) = system.as_event_particle_system() {
            let Ok((_, _, _, Some(parent), _)) = particles.get(*parent) else {
                continue;
            };
            sub.spawn_from_event(&mut buffer, parent);
        }
    }
}



fn sort_unstable<T>(buf: &mut [T], mut key: impl FnMut(&T) -> bool) {
    if buf.len() < 2 {
        return;
    }
    let mut start = 0;
    let mut end = buf.len() - 1;
    while start < end {
        if key(&buf[start]) {
            while key(&buf[end]) && end > 0 {
                end -= 1;
            }
            if start < end {
                buf.swap(start, end)
            }
        }
        start += 1;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParticleBufferStrategy {
    /// Move alive particles to the start of the buffer.
    #[default]
    Retain,
    /// Ignores dead particles when iterating.
    ///
    /// Should be used if lifetimes of particles are constant,
    /// might be awful otherwise.
    RingBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpirationState {
    None,
    Fizzle,
    Explode,
}

impl ExpirationState {
    pub const fn is_expired(&self) -> bool {
        !matches!(self, Self::None)
    }
}

pub trait Particle: Copy + 'static {
    /// Obtain the seed used to generate the particle.
    fn get_seed(&self) -> f32;
    /// Obtain the index of the particle inserted, optional.
    fn get_index(&self) -> u32 {
        0
    }
    /// Obtain the time span for which the particle is alive.
    fn get_lifetime(&self) -> f32;
    /// Obtain a value, usually normalized lifetime in `0.0..=1.0`, but not a requirement.
    fn get_fac(&self) -> f32 {
        self.get_lifetime()
    }
    /// Obtain the transform of the particle.
    fn get_transform(&self) -> Transform;
    /// Obtain the translation of the particle.
    fn get_position(&self) -> Vec3 {
        self.get_transform().translation
    }
    /// Obtain the tangent of the particle. Default to [`Transform::forward`].
    fn get_tangent(&self) -> Vec3 {
        self.get_transform().forward().as_vec3()
    }
    /// Obtain the color of the particle.
    fn get_color(&self) -> Srgba;

    /// Try obtain a curved path from start to current position.
    fn get_curve(&self) -> Option<Box<dyn Fn(f32) -> Vec3>> {
        None
    }

    /// Advance time on this particle.
    fn update(&mut self, dt: f32);

    /// Update and write events to a buffer.
    fn update_with_event_buffer(&mut self, dt: f32, buffer: &mut ParticleEventBuffer) {
        if self.is_expired() {
            return;
        }
        self.update(dt);
        let expr = self.expiration_state();
        if expr.is_expired() {
            buffer.push(ParticleEvent {
                event: expr.into(),
                seed: self.get_seed(),
                lifetime: self.get_lifetime(),
                position: self.get_position(),
                tangent: self.get_tangent(),
            })
        }
    }

    /// Obtain if and how this particle has expired.
    ///
    /// This can be written to a particle event queue.
    fn expiration_state(&self) -> ExpirationState;

    /// Returns true if the particle has expired.
    fn is_expired(&self) -> bool {
        self.expiration_state().is_expired()
    }
}

pub trait ParticleSystem {
    /// If true, use par_iter. Should not be set on smaller particle systems.
    ///
    /// Currently unimplemented.
    const PAR_ITER: bool = false;

    /// Changes what strategy to use when cleaning up used particles.
    const STRATEGY: ParticleBufferStrategy = ParticleBufferStrategy::Retain;

    /// Particle type of the system.
    ///
    /// # Panics
    ///
    /// If alignment is not in `1`, `2`, `4`, `8` or `16`.
    type Particle: Particle;

    /// Obtain debug information.
    fn as_debug(&self) -> &dyn Debug {
        #[derive(Debug)]
        pub struct ErasedParticleSystem;
        &ErasedParticleSystem
    }

    /// Obtain the capacity of the buffer, this value is read once upon initialization
    /// and will not be changed during simulation.
    /// 
    /// We might increment this value by a little bit for alignment.
    fn capacity(&self) -> usize;

    /// Generate a random `0.0..=1.0` number as a seed.
    fn rng(&mut self) -> f32 {
        fastrand::f32()
    }

    /// Determines how many particles to spawn when a time step passes.
    ///
    /// If used as a sub particle system, this will still be called,
    /// set this to 0 if not needed.
    fn spawn_step(&mut self, time: f32) -> usize;

    /// Convert a random seed into a particle.
    /// 
    /// If `spawn_step` is always `0`, consider implementing as [`unreachable!`].
    fn build_particle(&self, seed: f32) -> Self::Particle;

    /// Perform a meta action on the ParticleSystem.
    #[allow(unused_variables)]
    fn apply_command(&mut self, command: &dyn Any) {}

    /// Downcast into a [`SubParticleSystem`].
    fn as_sub_particle_system(&mut self) -> Option<&mut dyn ErasedSubParticleSystem> {
        None
    }

    /// Downcast into a [`EventParticleSystem`].
    fn as_event_particle_system(&mut self) -> Option<&mut dyn ErasedEventParticleSystem> {
        None
    }
}

pub trait ErasedParticleSystem: Send + Sync {
    fn as_debug(&self) -> &dyn Debug;
    fn update(&mut self, dt: f32, buffer: &mut ParticleBuffer);
    fn update_with_buffer(
        &mut self,
        dt: f32,
        buffer: &mut ParticleBuffer,
        events: &mut ParticleEventBuffer,
    );
    fn spawn_particle_buffer(&self) -> ParticleBuffer;
    fn apply_command(&mut self, command: &dyn Any);
    fn extract(&self, buffer: &ParticleBuffer) -> ExtractedParticleBuffer;
    fn as_sub_particle_system(&mut self) -> Option<&mut dyn ErasedSubParticleSystem>;
    fn as_event_particle_system(&mut self) -> Option<&mut dyn ErasedEventParticleSystem>;
}

mod noop {
    use bevy::{color::Srgba, transform::components::Transform};

    use crate::{Particle, ParticleSystem};

    #[derive(Debug, Clone, Copy)]
    pub struct NoopParticleSystem;

    impl Particle for NoopParticleSystem {
        fn get_seed(&self) -> f32 {
            0.
        }

        fn get_lifetime(&self) -> f32 {
            0.
        }

        fn get_transform(&self) -> bevy::prelude::Transform {
            Transform::IDENTITY
        }

        fn get_color(&self) -> bevy::prelude::Srgba {
            Srgba::WHITE
        }

        fn update(&mut self, _: f32) {}

        fn expiration_state(&self) -> crate::ExpirationState {
            crate::ExpirationState::None
        }
    }

    impl ParticleSystem for NoopParticleSystem {
        type Particle = NoopParticleSystem;

        fn as_debug(&self) -> &dyn std::fmt::Debug {
            self
        }

        fn capacity(&self) -> usize {
            1
        }

        fn spawn_step(&mut self, _: f32) -> usize {
            0
        }

        fn rng(&mut self) -> f32 {
            0.
        }

        fn build_particle(&self, _: f32) -> Self::Particle {
            NoopParticleSystem
        }
    }
}

#[derive(Debug, Component)]
pub struct ParticleInstance(Box<dyn ErasedParticleSystem>);

impl Default for ParticleInstance {
    fn default() -> Self {
        ParticleInstance::new(NoopParticleSystem)
    }
}

impl ParticleInstance {
    pub fn new<P: ParticleSystem + Send + Sync + 'static>(particles: P) -> Self {
        Self(Box::new(particles))
    }
}

impl Deref for ParticleInstance {
    type Target = dyn ErasedParticleSystem;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl DerefMut for ParticleInstance {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut()
    }
}

fn spawn_particle<T: ParticleSystem>(particles: &mut T) -> T::Particle {
    let seed = particles.rng();
    particles.build_particle(seed)
}

impl<T> ErasedParticleSystem for T
where
    T: ParticleSystem + Send + Sync,
{
    fn as_debug(&self) -> &dyn Debug {
        ParticleSystem::as_debug(self)
    }

    fn update(&mut self, dt: f32, buffer: &mut ParticleBuffer) {
        match Self::STRATEGY {
            ParticleBufferStrategy::Retain => {
                let original_len = buffer.len;
                let buf = buffer.get_mut::<T::Particle>();
                let mut len = 0;
                for item in buf.iter_mut() {
                    item.update(dt);
                    len += (!item.is_expired()) as usize
                }
                if len != original_len {
                    sort_unstable(buf, |x| x.is_expired());
                }
                buffer.len = len;
                buffer.extend((0..self.spawn_step(dt)).map(|_| spawn_particle(self)))
            }
            ParticleBufferStrategy::RingBuffer => {
                let buf = buffer.get_mut::<T::Particle>();
                let mut len = 0;
                for item in buf {
                    item.update(dt);
                    len += (!item.is_expired()) as usize
                }
                buffer.len = len;
                buffer.extend((0..self.spawn_step(dt)).map(|_| spawn_particle(self)))
            }
        }
    }

    fn update_with_buffer(
        &mut self,
        dt: f32,
        buffer: &mut ParticleBuffer,
        events: &mut ParticleEventBuffer,
    ) {
        match Self::STRATEGY {
            ParticleBufferStrategy::Retain => {
                let original_len = buffer.len;
                let buf = buffer.get_mut::<T::Particle>();
                let mut len = 0;
                for item in buf.iter_mut() {
                    item.update_with_event_buffer(dt, events);
                    len += (!item.is_expired()) as usize
                }
                if len != original_len {
                    sort_unstable(buf, |x| x.is_expired());
                }
                buffer.len = len;
                buffer.extend((0..self.spawn_step(dt)).map(|_| spawn_particle(self)))
            }
            ParticleBufferStrategy::RingBuffer => {
                let buf = buffer.get_mut::<T::Particle>();
                let mut len = 0;
                for item in buf {
                    item.update_with_event_buffer(dt, events);
                    len += (!item.is_expired()) as usize
                }
                buffer.len = len;
                buffer.extend((0..self.spawn_step(dt)).map(|_| spawn_particle(self)))
            }
        }
    }

    fn spawn_particle_buffer(&self) -> ParticleBuffer {
        match Self::STRATEGY {
            ParticleBufferStrategy::Retain => {
                ParticleBuffer::new_retain::<T::Particle>(self.capacity())
            }
            ParticleBufferStrategy::RingBuffer => {
                ParticleBuffer::new_ring::<T::Particle>(self.capacity())
            }
        }
    }

    /// Perform a meta action on the ParticleSystem
    fn apply_command(&mut self, command: &dyn Any) {
        ParticleSystem::apply_command(self, command)
    }

    fn extract(&self, buffer: &ParticleBuffer) -> ExtractedParticleBuffer {
        ExtractedParticleBuffer(
            buffer
                .get::<T::Particle>()
                .iter()
                .filter(|x| !x.is_expired())
                .map(|x| {
                    let transform = x.get_transform().compute_matrix();
                    ExtractedParticle {
                        index: x.get_index(),
                        lifetime: x.get_lifetime(),
                        seed: x.get_seed(),
                        fac: x.get_fac(),
                        color: x.get_color().to_vec4(),
                        transform_x: transform.row(0),
                        transform_y: transform.row(1),
                        transform_z: transform.row(2),
                    }
                })
                .collect(),
        )
    }

    fn as_sub_particle_system(&mut self) -> Option<&mut dyn ErasedSubParticleSystem> {
        ParticleSystem::as_sub_particle_system(self)
    }

    fn as_event_particle_system(&mut self) -> Option<&mut dyn ErasedEventParticleSystem> {
        ParticleSystem::as_event_particle_system(self)
    }
}

impl Debug for dyn ErasedParticleSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_debug().fmt(f)
    }
}

impl ExtractComponent for ParticleInstance {
    type QueryData = (&'static ParticleInstance, &'static ParticleBuffer);
    type QueryFilter = ();
    type Out = ExtractedParticleBuffer;

    fn extract_component((system, buffer): QueryItem<'_, Self::QueryData>) -> Option<Self::Out> {
        Some(system.extract(buffer))
    }
}
