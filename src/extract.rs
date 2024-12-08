use std::sync::Arc;

use bevy::{
    asset::{AssetId, Assets},
    color::ColorToComponents,
    ecs::{
        component::{ComponentHooks, StorageType},
        query::QueryItem,
    },
    prelude::{
        AlphaMode, Commands, Component, Entity, GlobalTransform, Query, Res, Resource, World,
    },
    render::{
        extract_component::ExtractComponent,
        render_resource::{BufferInitDescriptor, BufferUsages},
        renderer::RenderDevice,
        sync_world::MainEntity,
        Extract,
    },
    utils::HashMap,
};

use crate::{
    pipeline::ParticleInstanceBuffer, ExtractedParticleBuffer, ExtractedProjectile, Particle,
    ParticleBuffer, ParticleSystem, ProjectileCluster, ProjectileMat, ProjectileMaterial,
};

#[derive(Resource)]
pub struct ExtractedProjectileMeta<M: ProjectileMaterial> {
    pub(crate) alpha_modes: HashMap<AssetId<M>, AlphaMode>,
    pub(crate) entity_material: HashMap<MainEntity, AssetId<M>>,
}

#[derive(Resource, Default)]
pub struct ExtractedProjectileBuffers {
    pub(crate) extracted_buffers: HashMap<MainEntity, ExtractedParticleBuffer>,
    pub(crate) particle_ref: HashMap<MainEntity, MainEntity>,
    pub(crate) compiled_buffers: HashMap<MainEntity, ParticleInstanceBuffer>,
}

impl ExtractedProjectileBuffers {
    pub fn entities(&self) -> impl Iterator<Item = &MainEntity> {
        self.extracted_buffers
            .keys()
            .chain(self.particle_ref.keys())
            .chain(self.compiled_buffers.keys())
    }
}

#[derive(Resource, Default)]
pub struct PreparedInstanceBuffers {
    pub(crate) buffers: HashMap<MainEntity, ParticleInstanceBuffer>,
}

impl<M: ProjectileMaterial> ExtractedProjectileMeta<M> {
    fn get_alpha(&self, entity: &MainEntity) -> Option<AlphaMode> {
        self.entity_material
            .get(entity)
            .and_then(|x| self.alpha_modes.get(x))
            .copied()
    }
}

pub(crate) fn extract_buffers(
    buffers: Extract<
        Query<(
            Entity,
            &ProjectileCluster,
            &ParticleBuffer,
            &GlobalTransform,
        )>,
    >,
    references: Extract<Query<(Entity, &ProjectileRef)>>,
    one_shot: Extract<Query<(Entity, &OneShotParticleBuffer)>>,
    mut commands: Commands,
) {
    commands.insert_resource(ExtractedProjectileBuffers {
        extracted_buffers: buffers
            .iter()
            .filter_map(|(entity, system, buffer, transform)| {
                if buffer.is_uninit() {
                    return None;
                }
                let mut lock = buffer.extracted_allocation.lock().unwrap();
                if let Some(vec) = Arc::get_mut(&mut lock) {
                    system.extract(buffer, transform, None, vec);
                    Some((
                        MainEntity::from(entity),
                        ExtractedParticleBuffer(lock.clone()),
                    ))
                } else {
                    None
                }
            })
            .collect(),
        particle_ref: references
            .iter()
            .map(|(entity, p_ref)| (MainEntity::from(entity), MainEntity::from(p_ref.0)))
            .collect(),
        compiled_buffers: one_shot
            .iter()
            .map(|(entity, buffer)| (MainEntity::from(entity), buffer.0.clone()))
            .collect(),
    });
}

// Since we are relying on `Arc::get_mut` this is needed to remove duplicated references.
pub(crate) fn extract_clean(world: &mut World) {
    world.remove_resource::<ExtractedProjectileBuffers>();
}

pub(crate) fn extract_meta<M: ProjectileMaterial>(
    materials: Extract<Res<Assets<M>>>,
    query: Extract<Query<(Entity, &ProjectileMat<M>)>>,
    mut commands: Commands,
) {
    commands.insert_resource(ExtractedProjectileMeta {
        alpha_modes: materials
            .iter()
            .map(|(id, mat)| (id, mat.alpha_mode()))
            .collect(),
        entity_material: query
            .iter()
            .map(|(entity, mat)| (MainEntity::from(entity), mat.0.id()))
            .collect(),
    });
}

impl ExtractComponent for ProjectileRef {
    type QueryData = &'static ProjectileRef;
    type QueryFilter = ();
    type Out = ProjectileRef;

    fn extract_component(r: QueryItem<'_, Self::QueryData>) -> Option<Self::Out> {
        Some(*r)
    }
}

/// Create a cheap copy of a [`ProjectileCluster`]'s output
/// to use with a different set of material and mesh.
#[derive(Debug, Component, Clone, Copy)]
pub struct ProjectileRef(pub Entity);

impl Default for ProjectileRef {
    fn default() -> Self {
        ProjectileRef(Entity::PLACEHOLDER)
    }
}

impl From<Entity> for ProjectileRef {
    fn from(value: Entity) -> Self {
        ProjectileRef(value)
    }
}

/// A [`ParticleSystem`] that spawns once and maintains a GPU side instance buffer [`OneShotParticleBuffer`], i.e. grass.
pub struct OneShotParticleInstance(Vec<ExtractedProjectile>);

impl OneShotParticleInstance {
    pub fn new<P: ParticleSystem>(mut particles: P) -> Self {
        let count = particles.spawn_step(0.);
        let mut buf = Vec::with_capacity(count);
        for _ in 0..count {
            let seed = particles.rng();
            let particle = particles.build_particle(seed);
            let mat = particle.get_transform().compute_matrix();
            buf.push(ExtractedProjectile {
                index: particle.get_index(),
                lifetime: particle.get_lifetime(),
                fac: particle.get_fac(),
                seed,
                transform_x: mat.row(0),
                transform_y: mat.row(1),
                transform_z: mat.row(2),
                color: particle.get_color().to_vec4(),
            })
        }
        Self(buf)
    }
}

impl Component for OneShotParticleInstance {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, entity, _| {
            let render_device = world.resource::<RenderDevice>();
            let Some(item) = world.entity(entity).get::<OneShotParticleInstance>() else {
                return;
            };
            let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("particle instance buffer"),
                contents: bytemuck::cast_slice(&item.0),
                usage: BufferUsages::VERTEX,
            });
            let length = item.0.len();
            world
                .commands()
                .entity(entity)
                .insert(OneShotParticleBuffer(ParticleInstanceBuffer {
                    buffer,
                    length,
                }))
                .remove::<OneShotParticleInstance>();
        });
    }
}

/// Handle for a spawned GPU side particle instance buffer.
#[derive(Component)]
pub struct OneShotParticleBuffer(ParticleInstanceBuffer);

/// Marker for [`OneShotParticleBuffer`] to use the correct render pipeline.
#[doc(hidden)]
#[derive(Component)]
pub struct OneShotParticleMarker;
