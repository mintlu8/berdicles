#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![doc = include_str!("../README.md")]
use std::{
    any::Any,
    fmt::Debug,
    ops::{Deref, DerefMut, Range},
};

use bevy::{
    app::{Plugin, Update},
    asset::Assets,
    color::{ColorToComponents, Srgba},
    math::{Quat, Vec3},
    prelude::{Component, DetectChanges, Entity, IntoSystemConfigs, Query, Ref, Res, Visibility},
    render::{render_resource::Shader, ExtractSchedule, RenderApp},
    time::{Time, Virtual},
    transform::components::{GlobalTransform, Transform},
};
use noop::NoopParticleSystem;

mod extract;
pub use extract::*;
mod material;
pub use material::*;
mod pipeline;
pub use pipeline::ProjectileMaterialPlugin;
pub mod shader;
mod sub;
pub use sub::*;
mod buffer;
pub mod trail;
pub mod util;
pub use buffer::*;
use trail::{trail_rendering, TrailParticleSystem};
mod noop;
mod ring_buffer;
pub use ring_buffer::RingBuffer;
// mod billboard;
// pub use billboard::*;
pub mod templates;

/// Plugin for `berdicle`.
///
/// Adds support for [`StandardParticle`],
/// other particle materials must be manually added via
/// [`ParticleMaterialPlugin`].
pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.world_mut().resource_mut::<Assets<Shader>>().insert(
            &shader::PARTICLE_VERTEX,
            Shader::from_wgsl(
                include_str!("./shader.wgsl"),
                "berdicle/particle_vertex.wgsl",
            ),
        );
        app.world_mut().resource_mut::<Assets<Shader>>().insert(
            &shader::PARTICLE_FRAGMENT,
            Shader::from_wgsl(
                include_str!("./shader.wgsl"),
                "berdicle/particle_fragment.wgsl",
            ),
        );
        app.add_plugins(ProjectileMaterialPlugin::<StandardProjectile>::default());
        app.add_systems(Update, projectile_simulation_system);
        app.add_systems(Update, trail_rendering.after(projectile_simulation_system));
        app.sub_app_mut(RenderApp)
            .add_systems(ExtractSchedule, (extract_clean, extract_buffers).chain());
    }
}

/// The main system of `berdicle`, runs in [`Update`].
pub fn projectile_simulation_system(
    time: Res<Time<Virtual>>,
    mut particles: Query<(
        Entity,
        &mut ProjectileCluster,
        &mut ParticleBuffer,
        Ref<GlobalTransform>,
        Option<&mut ParticleEventBuffer>,
        Option<&ProjectileParent>,
    )>,
) {
    let dt = time.delta_secs();
    particles
        .par_iter_mut()
        .for_each(|(_, mut system, mut buffer, transform, events, _)| {
            if buffer.is_uninit() {
                *buffer = system.spawn_particle_buffer();
            }
            if transform.is_changed() && system.is_world_space() {
                system.update_position(&transform)
            }
            if let Some(mut events) = events {
                events.clear();
                system.update_with_event_buffer(dt, &mut buffer, &mut events);
            } else {
                system.update(dt, &mut buffer);
            }
        });

    // Safety: parent is checked to not be the same entity.
    for (entity, mut system, mut buffer, _, _, parent) in unsafe { particles.iter_unsafe() } {
        let Some(ProjectileParent(parent)) = parent else {
            continue;
        };
        if entity == *parent {
            panic!("ParticleSystem's parent cannot be itself.")
        }
        if let Some(sub) = system.as_sub_particle_system() {
            // Safety: parent is checked to not be the same entity.
            let Ok((_, _, mut parent, _, _, _)) = (unsafe { particles.get_unchecked(*parent) })
            else {
                continue;
            };
            sub.spawn_from_parent(dt, &mut buffer, &mut parent);
        }
        if let Some(sub) = system.as_event_particle_system() {
            let Ok((_, _, _, _, Some(parent), _)) = particles.get(*parent) else {
                continue;
            };
            sub.spawn_on_event(&mut buffer, parent);
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

/// If and how a particle has expired.
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

    /// Returns [`ExpirationState::Fizzle`] if true.
    pub const fn fizzle_if(cond: bool) -> Self {
        if cond {
            Self::Fizzle
        } else {
            Self::None
        }
    }

    /// Returns [`ExpirationState::Explode`] if true.
    pub const fn explode_if(cond: bool) -> Self {
        if cond {
            Self::Explode
        } else {
            Self::None
        }
    }
}

/// A [`Particle`]. Must be [`Copy`] and have alignment less than `16`.
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
    fn get_color(&self) -> Srgba {
        Srgba::WHITE
    }

    /// Advance time on this particle.
    fn update(&mut self, dt: f32);

    /// Update and write events to a buffer.
    fn update_with_event_buffer(&mut self, dt: f32, buffer: &mut ParticleEventBuffer) {
        let is_expired = self.is_expired();
        self.update(dt);
        if is_expired {
            return;
        }
        let expr = self.expiration_state();
        if expr.is_expired() {
            buffer.push(ParticleEvent {
                event: expr.into(),
                seed: self.get_seed(),
                index: self.get_index(),
                lifetime: self.get_lifetime(),
                position: self.get_position(),
                tangent: self.get_tangent(),
            })
        }
    }

    /// Obtain if and how this particle has expired.
    /// If rendering trails, this does not affect the aliveness of the trail.
    ///
    /// If paired with an [`ParticleEventBuffer`], the type of expiration will be written there.
    fn expiration_state(&self) -> ExpirationState;

    /// Returns true if the particle has expired.
    fn is_expired(&self) -> bool {
        self.expiration_state().is_expired()
    }
}

/// A particle spawner type.
pub trait ParticleSystem {
    /// If true, ignore [`Transform`] and [`GlobalTransform`].
    const WORLD_SPACE: bool = false;

    /// Changes what strategy to use when cleaning up used particles.
    ///
    /// * Retain(default): Remove expired particles by moving alive particles in front.
    /// * RingBuffer: Particles are not removed explicitly, but can expire and be reused later.
    ///   Should only be used if lifetime is constant and capacity is well predicted.
    ///
    /// If rendering trails using ring buffer, capacity for detached trails should be reserved.
    const STRATEGY: ParticleBufferStrategy = ParticleBufferStrategy::Retain;

    /// If not 0, render ribbons.
    const RIBBON_LENGTH: usize = 0;

    /// Particle type of the system.
    ///
    /// # Panics
    ///
    /// If alignment is not in `1`, `2`, `4`, `8` or `16`.
    type Particle: Particle;

    /// Obtain debug information.
    ///
    /// If not specified, `Debug` will only print a generic struct.
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
    /// If used as a sub particle system, this step will still be called,
    /// set this to 0 if not needed.
    fn spawn_step(&mut self, time: f32) -> usize;

    /// Convert a random seed into a particle.
    ///
    /// If `spawn_step` is always `0`,
    /// it's safe to implement with [`unreachable!`].
    fn build_particle(&self, seed: f32) -> Self::Particle;

    /// Additional actions to perform during update.
    ///
    /// if rendering trails using `Retain`, we should call [`ParticleBuffer::update_detached`] here.
    #[allow(unused_variables)]
    fn on_update(&mut self, dt: f32, buffer: &mut ParticleBuffer) {}

    /// If rendering trails using `Retain`, call [`ParticleBuffer::detach_slice`].
    #[allow(unused_variables)]
    fn detach_slice(&mut self, detached: Range<usize>, buffer: &mut ParticleBuffer) {}

    /// Perform a meta action on the ParticleSystem.
    ///
    /// Since [`ParticleInstance`] is type erased, this is the standard way to modify
    /// a [`ParticleSystem`] at runtime.
    ///
    /// # Example
    ///
    /// Change the spawner's transform matrix when `GlobalTransform` is changed.
    /// (functionalities for [`update_position`](ParticleSystem::update_position))
    ///
    /// ```
    /// # /*
    /// fn apply_meta(&mut self, command: &dyn Any) {
    ///     if let Some(transform) = command.downcast_ref::<GlobalTransform>() {
    ///         self.transform_matrix = (*transform).into();
    ///     }
    /// }
    ///
    /// fn system_sync_transform_with_emitter(
    ///     mut query: Query<(&mut ParticleInstance, &GlobalTransform), Changed<GlobalTransform>>
    /// ) {
    ///     for (mut particle, transform) in query {
    ///         particle.apply_meta(transform);
    ///     }
    /// }
    /// # */
    /// ```
    #[allow(unused_variables)]
    fn apply_meta(&mut self, command: &dyn Any, buffer: &mut ParticleBuffer) {}

    /// Optionally update the position of the spawner,
    /// by default only called if `WORLD_SPACE` and [`GlobalTransform`] is changed.
    #[allow(unused_variables)]
    fn update_position(&mut self, transform: &GlobalTransform) {}

    /// Downcast into a [`SubParticleSystem`].
    fn as_sub_particle_system(&mut self) -> Option<&mut dyn ErasedSubParticleSystem> {
        None
    }

    /// Downcast into a [`EventParticleSystem`].
    fn as_event_particle_system(&mut self) -> Option<&mut dyn ErasedEventParticleSystem> {
        None
    }

    /// Downcast into a [`TrailParticleSystem`], requires `Self::Particle` to be a [`TrailedParticle`](trail::TrailedParticle).
    fn as_trail_particle_system(&mut self) -> Option<&mut dyn TrailParticleSystem> {
        None
    }
}

/// Type erased version of [`ParticleSystem`].
pub trait ErasedParticleSystem: Send + Sync {
    /// Obtain debug information.
    ///
    /// If not specified, `Debug` will only print a generic struct.
    fn as_debug(&self) -> &dyn Debug;
    /// Convert to an [`Any`].
    fn as_any(&self) -> &dyn Any;
    /// Convert to a mutable [`Any`].
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Returns [`ParticleSystem::WORLD_SPACE`].
    fn is_world_space(&self) -> bool;
    /// Advance by time.
    fn update(&mut self, dt: f32, buffer: &mut ParticleBuffer);
    /// Advance by time, write to an event buffer.
    fn update_with_event_buffer(
        &mut self,
        dt: f32,
        buffer: &mut ParticleBuffer,
        events: &mut ParticleEventBuffer,
    );
    /// Create an empty [`ParticleBuffer`].
    fn spawn_particle_buffer(&self) -> ParticleBuffer;
    /// Update the global position of the spawner.
    #[allow(unused_variables)]
    fn update_position(&mut self, transform: &GlobalTransform);
    /// Perform a meta action on the ParticleSystem.
    fn apply_meta(&mut self, command: &dyn Any, buffer: &mut ParticleBuffer);
    fn extract(
        &self,
        buffer: &ParticleBuffer,
        transform: &GlobalTransform,
        billboard: Option<Quat>,
        vec: &mut Vec<ExtractedProjectile>,
    );
    /// Downcast into a [`SubParticleSystem`];
    fn as_sub_particle_system(&mut self) -> Option<&mut dyn ErasedSubParticleSystem>;
    /// Downcast into a [`EventParticleSystem`];
    fn as_event_particle_system(&mut self) -> Option<&mut dyn ErasedEventParticleSystem>;
    /// Downcast into a [`TrailParticleSystem`];
    fn as_trail_particle_system(&mut self) -> Option<&mut dyn TrailParticleSystem>;
    /// Checks if all particles and trails are despawned.
    ///
    /// Be careful this is usually true on the first frame as well.
    fn should_despawn(&mut self, buffer: &ParticleBuffer) -> bool;
}

/// Component form of a type erased [`ParticleSystem`].
#[derive(Debug, Component)]
#[require(ParticleBuffer, Transform, Visibility)]
pub struct ProjectileCluster(Box<dyn ErasedParticleSystem>);

impl Default for ProjectileCluster {
    fn default() -> Self {
        ProjectileCluster::new(NoopParticleSystem)
    }
}

impl ProjectileCluster {
    pub fn new<P: ParticleSystem + Send + Sync + 'static>(particles: P) -> Self {
        Self(Box::new(particles))
    }

    /// Try obtain a [`ParticleSystem`] by downcasting.
    pub fn downcast_ref<P: ParticleSystem + Send + Sync + 'static>(&self) -> Option<&P> {
        self.0.as_any().downcast_ref()
    }

    /// Try obtain a mutable [`ParticleSystem`] by downcasting.
    ///
    /// Alternatively use [`ParticleSystem::apply_meta`].
    pub fn downcast_mut<P: ParticleSystem + Send + Sync + 'static>(&mut self) -> Option<&mut P> {
        self.0.as_any_mut().downcast_mut()
    }
}

impl Deref for ProjectileCluster {
    type Target = dyn ErasedParticleSystem;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl DerefMut for ProjectileCluster {
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
    T: ParticleSystem + Send + Sync + 'static,
{
    fn as_debug(&self) -> &dyn Debug {
        ParticleSystem::as_debug(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_world_space(&self) -> bool {
        T::WORLD_SPACE
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
                    self.detach_slice(len..original_len, buffer)
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
        self.on_update(dt, buffer)
    }

    fn update_with_event_buffer(
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
                    self.detach_slice(len..original_len, buffer)
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
        self.on_update(dt, buffer)
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

    fn update_position(&mut self, transform: &GlobalTransform) {
        ParticleSystem::update_position(self, transform)
    }

    fn apply_meta(&mut self, command: &dyn Any, buffer: &mut ParticleBuffer) {
        ParticleSystem::apply_meta(self, command, buffer)
    }

    #[allow(clippy::collapsible_else_if)]
    fn extract(
        &self,
        buffer: &ParticleBuffer,
        transform: &GlobalTransform,
        billboard: Option<Quat>,
        vec: &mut Vec<ExtractedProjectile>,
    ) {
        vec.clear();
        vec.extend(
            buffer
                .get::<T::Particle>()
                .iter()
                .filter(|x| !x.is_expired())
                .map(|x| {
                    let transform = if let Some(bb) = billboard {
                        if T::WORLD_SPACE {
                            x.get_transform().with_rotation(bb).compute_matrix()
                        } else {
                            let (scale, _, translation) = transform
                                .mul_transform(x.get_transform())
                                .to_scale_rotation_translation();
                            Transform {
                                translation,
                                rotation: bb,
                                scale,
                            }
                            .compute_matrix()
                        }
                    } else {
                        if T::WORLD_SPACE {
                            x.get_transform().compute_matrix()
                        } else {
                            transform.mul_transform(x.get_transform()).compute_matrix()
                        }
                    };
                    ExtractedProjectile {
                        index: x.get_index(),
                        lifetime: x.get_lifetime(),
                        seed: x.get_seed(),
                        fac: x.get_fac(),
                        color: x.get_color().to_vec4(),
                        transform_x: transform.row(0),
                        transform_y: transform.row(1),
                        transform_z: transform.row(2),
                    }
                }),
        )
    }

    fn as_sub_particle_system(&mut self) -> Option<&mut dyn ErasedSubParticleSystem> {
        ParticleSystem::as_sub_particle_system(self)
    }

    fn as_event_particle_system(&mut self) -> Option<&mut dyn ErasedEventParticleSystem> {
        ParticleSystem::as_event_particle_system(self)
    }

    fn as_trail_particle_system(&mut self) -> Option<&mut dyn TrailParticleSystem> {
        ParticleSystem::as_trail_particle_system(self)
    }

    fn should_despawn(&mut self, buffer: &ParticleBuffer) -> bool {
        if buffer.len != 0 {
            return false;
        }
        if let Some(trails) = self.as_trail_particle_system() {
            trails.should_despawn(buffer)
        } else {
            true
        }
    }
}

impl Debug for dyn ErasedParticleSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_debug().fmt(f)
    }
}
