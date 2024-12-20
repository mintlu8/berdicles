use bevy::{color::Srgba, transform::components::Transform};

use crate::{Projectile, ProjectileSystem};

#[derive(Debug, Clone, Copy)]
pub struct NoopParticleSystem;

impl Projectile for NoopParticleSystem {
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

impl ProjectileSystem for NoopParticleSystem {
    type Projectile = NoopParticleSystem;

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

    fn build_particle(&self, _: f32) -> Self::Projectile {
        NoopParticleSystem
    }
}
