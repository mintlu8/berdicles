use std::sync::Arc;

use bevy::{
    asset::{AssetId, Assets},
    color::ColorToComponents,
    ecs::{
        component::{ComponentHooks, StorageType},
        query::QueryItem,
    },
    prelude::{
        AlphaMode, Commands, Component, Deref, DerefMut, Entity, GlobalTransform, Query, Res,
        Resource, World,
    },
    render::{
        extract_component::ExtractComponent,
        render_resource::{BufferInitDescriptor, BufferUsages},
        renderer::RenderDevice,
        sync_world::MainEntity,
        view::RenderLayers,
        Extract,
    },
    utils::HashMap,
};

use crate::{
    pipeline::{InstanceBuffer, InstancedPipelineKey},
    DefaultInstanceBuffer, ExtractedParticleBuffer, InstancedMaterial, InstancedMaterial3d,
    Projectile, ProjectileBuffer, ProjectileCluster, ProjectileSystem,
};

#[derive(Resource)]
pub struct ExtractedProjectileMeta<M: InstancedMaterial> {
    pub(crate) mode: HashMap<AssetId<M>, (AlphaMode, InstancedPipelineKey)>,
    pub(crate) entity_material: HashMap<MainEntity, AssetId<M>>,
}

#[derive(Resource, Default)]
pub struct ExtractedProjectileBuffers {
    pub(crate) extracted_buffers: HashMap<MainEntity, ExtractedParticleBuffer>,
    pub(crate) particle_ref: HashMap<MainEntity, MainEntity>,
    pub(crate) compiled_buffers: HashMap<MainEntity, InstanceBuffer>,
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct ExtractedTransforms(HashMap<MainEntity, GlobalTransform>);

#[derive(Resource, Default, Deref, DerefMut)]
pub struct ExtractedRenderLayers(HashMap<MainEntity, RenderLayers>);

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
    pub(crate) buffers: HashMap<MainEntity, InstanceBuffer>,
}

impl<M: InstancedMaterial> ExtractedProjectileMeta<M> {
    pub fn get_alpha(&self, entity: &MainEntity) -> Option<AlphaMode> {
        self.entity_material
            .get(entity)
            .and_then(|x| self.mode.get(x))
            .map(|x| x.0)
    }
}

pub(crate) fn extract_buffers(
    buffers: Extract<Query<(Entity, &ProjectileCluster, &ProjectileBuffer)>>,
    references: Extract<Query<(Entity, &ProjectileRef)>>,
    one_shot: Extract<Query<(Entity, &CompiledHairBuffer)>>,
    transforms: Extract<Query<(Entity, &GlobalTransform)>>,
    layers: Extract<Query<(Entity, &RenderLayers)>>,
    mut commands: Commands,
) {
    let buffers = ExtractedProjectileBuffers {
        extracted_buffers: buffers
            .iter()
            .filter_map(|(entity, system, buffer)| {
                if buffer.is_uninit() {
                    return None;
                }
                let entity = MainEntity::from(entity);
                let mut lock = buffer.extracted_allocation.lock().unwrap();
                if let Some(vec) = Arc::get_mut(&mut lock) {
                    system.extract(buffer, vec);
                    Some((entity, ExtractedParticleBuffer(lock.clone())))
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
    };

    commands.insert_resource(ExtractedTransforms(
        transforms
            .iter_many(buffers.entities().map(|x| x.id()))
            .map(|(entity, transform)| (MainEntity::from(entity), *transform))
            .collect(),
    ));

    commands.insert_resource(ExtractedRenderLayers(
        layers
            .iter_many(buffers.entities().map(|x| x.id()))
            .map(|(entity, layers)| (MainEntity::from(entity), layers.clone()))
            .collect(),
    ));

    commands.insert_resource(buffers);
}

// Since we are relying on `Arc::get_mut` this is needed to remove duplicated references.
pub(crate) fn extract_clean(world: &mut World) {
    world.remove_resource::<ExtractedProjectileBuffers>();
}

pub(crate) fn extract_meta<M: InstancedMaterial>(
    materials: Extract<Res<Assets<M>>>,
    query: Extract<Query<(Entity, &InstancedMaterial3d<M>)>>,
    mut commands: Commands,
) {
    commands.insert_resource(ExtractedProjectileMeta {
        mode: materials
            .iter()
            .map(|(id, mat)| (id, (mat.alpha_mode(), mat.pipeline_key())))
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

/// A [`ProjectileSystem`] that spawns once and maintains a GPU side instance buffer, i.e. grass.
pub struct HairParticles(Vec<DefaultInstanceBuffer>);

impl HairParticles {
    pub fn new<P: ProjectileSystem>(mut particles: P) -> Self {
        let count = particles.spawn_step(0.);
        let mut buf = Vec::with_capacity(count);
        for _ in 0..count {
            let seed = particles.rng();
            let particle = particles.build_particle(seed);
            let mat = particle.get_transform().compute_matrix();
            buf.push(DefaultInstanceBuffer {
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

impl Component for HairParticles {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, entity, _| {
            let render_device = world.resource::<RenderDevice>();
            let Some(item) = world.entity(entity).get::<HairParticles>() else {
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
                .insert(CompiledHairBuffer(InstanceBuffer { buffer, length }))
                .remove::<HairParticles>();
        });
    }
}

/// Handle for a spawned GPU side instance buffer.
#[derive(Component)]
pub struct CompiledHairBuffer(InstanceBuffer);
