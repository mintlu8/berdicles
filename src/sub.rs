use bevy::{
    math::Vec3,
    prelude::{Component, Entity},
};
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

use crate::{
    ErasedParticleSystem, ExpirationState, Projectile, ProjectileBuffer, ProjectileSystem,
};

/// Event on individual particle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub struct ProjectileEvent {
    pub event: ParticleEventType,
    pub seed: f32,
    pub index: u32,
    pub lifetime: f32,
    pub position: Vec3,
    pub tangent: Vec3,
}

/// Parent of the particle, if present will read data/event from the parent's particle buffer.
#[derive(Debug, Component, Clone, Copy)]
pub struct ProjectileParent(pub Entity);

impl Default for ProjectileParent {
    fn default() -> Self {
        ProjectileParent(Entity::PLACEHOLDER)
    }
}

impl From<Entity> for ProjectileParent {
    fn from(value: Entity) -> Self {
        ProjectileParent(value)
    }
}

/// A buffer of particle events. If added to a particle bundle, will record particle events happened
/// in this frame. Also enables [`EventProjectileSystem`].
#[derive(Debug, Component, Default)]
pub struct ProjectileEventBuffer(Vec<ProjectileEvent>);

impl Deref for ProjectileEventBuffer {
    type Target = Vec<ProjectileEvent>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ProjectileEventBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A [`ProjectileSystem`] that spawns projectiles from a parent
/// `ProjectileSystem`'s alive particles.
pub trait SubProjectileSystem: ProjectileSystem {
    type Parent: Projectile;

    /// Determines how many particles to spawn in a frame **per particle**.
    ///
    /// You might want to keep track of this on a field in the parent's particle.
    fn spawn_step_sub(&mut self, parent: &mut Self::Parent, dt: f32) -> usize;

    /// Convert a random seed into a particle with parent information.
    fn build_sub_projectile(parent: &Self::Parent, seed: f32) -> Self::Projectile;
}

/// An erased [`SubProjectileSystem`].
pub trait ErasedSubParticleSystem: ErasedParticleSystem {
    fn spawn_from_parent(
        &mut self,
        dt: f32,
        buffer: &mut ProjectileBuffer,
        parent: &mut ProjectileBuffer,
    );
}

impl<T> ErasedSubParticleSystem for T
where
    T: SubProjectileSystem + ErasedParticleSystem,
{
    fn spawn_from_parent(
        &mut self,
        dt: f32,
        buffer: &mut ProjectileBuffer,
        parent: &mut ProjectileBuffer,
    ) {
        for parent in parent.get_mut::<T::Parent>() {
            if parent.is_expired() {
                continue;
            }
            let num = self.spawn_step_sub(parent, dt);
            buffer.extend(
                (0..num)
                    .map(|_| self.rng())
                    .map(|seed| Self::build_sub_projectile(parent, seed)),
            )
        }
    }
}

impl Debug for dyn ErasedSubParticleSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_debug().fmt(f)
    }
}

/// A [`ProjectileSystem`] that spawns particles on parent's emitted events.
///
/// You must add [`ProjectileEventBuffer`] to the parent for this to function.
pub trait EventProjectileSystem: ProjectileSystem {
    /// Returns how many to spawn in a burst on an event.
    fn spawn_on_event(&mut self, parent: &ProjectileEvent) -> usize;

    /// Convert a random seed into a particle with parent information.
    fn build_sub_projectile(parent: &ProjectileEvent, seed: f32) -> Self::Projectile;
}

/// Type erased [`EventProjectileSystem`].
pub trait ErasedEventParticleSystem: ErasedParticleSystem {
    /// Spawn particles on event.
    fn spawn_on_event(&mut self, buffer: &mut ProjectileBuffer, parent: &ProjectileEventBuffer);
}

impl<T> ErasedEventParticleSystem for T
where
    T: EventProjectileSystem + ErasedParticleSystem,
{
    fn spawn_on_event(&mut self, buffer: &mut ProjectileBuffer, parent: &ProjectileEventBuffer) {
        for event in parent.iter() {
            let num = self.spawn_on_event(event);
            buffer.extend(
                (0..num)
                    .map(|_| self.rng())
                    .map(|seed| Self::build_sub_projectile(event, seed)),
            )
        }
    }
}

impl Debug for dyn ErasedEventParticleSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_debug().fmt(f)
    }
}
