use bevy::{
    math::Vec3,
    prelude::{Component, Entity},
};
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

use crate::{ErasedParticleSystem, ExpirationState, Particle, ParticleBuffer, ParticleSystem};

/// Event on individual particle.
#[derive(Debug, Clone, Copy)]
pub enum ParticleEventType {
    Explode,
    Fizzle,
    Collide,
}

impl From<ExpirationState> for ParticleEventType {
    fn from(value: ExpirationState) -> Self {
        match value {
            ExpirationState::None => panic!(),
            ExpirationState::Fizzle => ParticleEventType::Fizzle,
            ExpirationState::Explode => ParticleEventType::Explode,
        }
    }
}

/// Event and data on an individual particle.
#[derive(Debug, Clone, Copy)]
pub struct ParticleEvent {
    pub event: ParticleEventType,
    pub seed: f32,
    pub lifetime: f32,
    pub position: Vec3,
    pub tangent: Vec3,
}

/// Parent of the particle, if present will read data/event from the parent's particle buffer.
#[derive(Debug, Component, Clone, Copy)]
pub struct ParticleParent(pub Entity);

/// A buffer of particle events. If added to a particle bundle, will record particle events happened
/// in this frame. Also enables [`EventParticleSystem`].
#[derive(Debug, Component, Default)]
pub struct ParticleEventBuffer(Vec<ParticleEvent>);

impl Deref for ParticleEventBuffer {
    type Target = Vec<ParticleEvent>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ParticleEventBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A [`ParticleSystem`] that spawns particles from a parent
/// `ParticleSystem`'s alive particles.
pub trait SubParticleSystem: ParticleSystem {
    type Parent: Particle;

    /// Determines how many particles to spawn in a frame **per particle**.
    ///
    /// You might want to keep track of this on a field in the parent's particle.
    fn spawn_step_sub(&mut self, parent: &mut Self::Parent, dt: f32) -> usize;

    /// Convert a random seed into a particle with parent information.
    fn into_sub_particle(parent: &Self::Parent, seed: f32) -> Self::Particle;
}

/// An erased [`SubParticleSystem`].
pub trait ErasedSubParticleSystem: ErasedParticleSystem {
    fn spawn_from_parent(
        &mut self,
        dt: f32,
        buffer: &mut ParticleBuffer,
        parent: &mut ParticleBuffer,
    );
}

impl<T> ErasedSubParticleSystem for T
where
    T: SubParticleSystem + ErasedParticleSystem,
{
    fn spawn_from_parent(
        &mut self,
        dt: f32,
        buffer: &mut ParticleBuffer,
        parent: &mut ParticleBuffer,
    ) {
        for parent in parent.get_mut::<T::Parent>() {
            if parent.is_expired() {
                continue;
            }
            let num = self.spawn_step_sub(parent, dt);
            buffer.extend(
                (0..num)
                    .map(|_| self.rng())
                    .map(|seed| Self::into_sub_particle(parent, seed)),
            )
        }
    }
}

impl Debug for dyn ErasedSubParticleSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_debug().fmt(f)
    }
}

/// A [`ParticleSystem`] that spawns particles on parent's emitted events.
///
/// You must add [`ParticleEventBuffer`] to the parent for this to function.
pub trait EventParticleSystem: ParticleSystem {
    /// Returns how many to spawn in a burst on an event.
    fn spawn_on_event(&mut self, parent: &ParticleEvent) -> usize;

    /// Convert a random seed into a particle with parent information.
    fn into_sub_particle(parent: &ParticleEvent, seed: f32) -> Self::Particle;
}

/// Type erased [`EventParticleSystem`].
pub trait ErasedEventParticleSystem: ErasedParticleSystem {
    /// Spawn particles on event.
    fn spawn_on_event(&mut self, buffer: &mut ParticleBuffer, parent: &ParticleEventBuffer);
}

impl<T> ErasedEventParticleSystem for T
where
    T: EventParticleSystem + ErasedParticleSystem,
{
    fn spawn_on_event(&mut self, buffer: &mut ParticleBuffer, parent: &ParticleEventBuffer) {
        for event in parent.iter() {
            let num = self.spawn_on_event(event);
            buffer.extend(
                (0..num)
                    .map(|_| self.rng())
                    .map(|seed| Self::into_sub_particle(event, seed)),
            )
        }
    }
}

impl Debug for dyn ErasedEventParticleSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_debug().fmt(f)
    }
}
