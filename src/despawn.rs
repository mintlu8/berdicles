use bevy::prelude::{Commands, Component, DespawnRecursiveExt, Entity, Query};

use crate::{ProjectileBuffer, ProjectileCluster};

/// Remove the associated entity if all projectiles are despawned.
///
/// Simple ways to use this component are trigger one-shot channels on [`Drop`],
/// use the remove component hook or an observer to send events.
///
/// This component will not be triggered if no projectile has been alive since this component is added.
#[derive(Debug, Clone, Copy, Component, Default)]
pub struct DespawnProjectileCluster {
    at_least_one_spawned: bool,
}

impl DespawnProjectileCluster {
    pub const fn new() -> Self {
        Self {
            at_least_one_spawned: false,
        }
    }
}

pub fn despawn_projectiles(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut DespawnProjectileCluster,
        &ProjectileCluster,
        &ProjectileBuffer,
    )>,
) {
    for (entity, mut despawn, projectiles, buffer) in &mut query {
        if despawn.at_least_one_spawned {
            if projectiles.should_despawn(buffer) {
                commands.entity(entity).despawn_recursive();
            }
        } else if !buffer.is_empty() {
            despawn.at_least_one_spawned = true;
        }
    }
}
