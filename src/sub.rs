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

#[derive(Debug, Component, Default)]
pub struct ParticleEventBuffer(Vec<ParticleEvent>);

/// Parent of the particle, if present will read data/event from their particle buffer.
#[derive(Debug, Component, Clone, Copy)]
pub struct ParticleParent(pub Entity);

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

pub trait SubParticleSystem: ParticleSystem {
    type Parent: Particle;

    /// * For [`ParticleEventType::Always`], determines how many particles to spawn in a frame.
    /// * For others, returns how many to spawn in a burst.
    fn spawn_step_sub(&mut self, parent: &mut Self::Parent, dt: f32) -> usize;

    /// Convert a random seed into a particle with parent information.
    fn into_sub_particle(parent: &Self::Parent, seed: f32) -> Self::Particle;
}

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

pub trait EventParticleSystem: ParticleSystem {
    /// * For [`ParticleEventType::Always`], determines how many particles to spawn in a frame.
    /// * For others, returns how many to spawn in a burst.
    fn spawn_event(&mut self, parent: &ParticleEvent) -> usize;

    /// Convert a random seed into a particle with parent information.
    fn into_sub_particle(parent: &ParticleEvent, seed: f32) -> Self::Particle;
}

pub trait ErasedEventParticleSystem: ErasedParticleSystem {
    fn spawn_from_event(&mut self, buffer: &mut ParticleBuffer, parent: &ParticleEventBuffer);
}

impl<T> ErasedEventParticleSystem for T
where
    T: EventParticleSystem + ErasedParticleSystem,
{
    fn spawn_from_event(&mut self, buffer: &mut ParticleBuffer, parent: &ParticleEventBuffer) {
        for event in parent.iter() {
            let num = self.spawn_event(event);
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
