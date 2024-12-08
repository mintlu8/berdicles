//! A shader that renders a mesh multiple times in one draw call.

use std::marker::PhantomData;

use bevy::{
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    pbr::{
        alpha_mode_pipeline_key, MeshPipeline, MeshPipelineKey, RenderMeshInstances,
        SetMeshBindGroup, SetMeshViewBindGroup,
    },
    prelude::*,
    render::{
        mesh::{
            allocator::MeshAllocator, MeshVertexBufferLayoutRef, RenderMesh, RenderMeshBufferInfo,
        },
        render_asset::{PrepareAssetError, RenderAsset, RenderAssetPlugin, RenderAssets},
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
            RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
        },
        render_resource::*,
        renderer::RenderDevice,
        view::ExtractedView,
        Render, RenderApp, RenderSet,
    },
};

use crate::{
    extract_meta,
    shader::{PARTICLE_FRAGMENT, PARTICLE_VERTEX},
    ExtractedProjectileBuffers, ExtractedProjectileMeta, PreparedInstanceBuffers,
    ProjectileInstanceBuffer, ProjectileMaterial,
};

/// Add particle rendering pipeline for a [`Material`].
///
/// You should **NOT** add the corresponding `MaterialPlugin`,
/// as `ParticleSystemBundle` is also a valid `MaterialMeshBundle`.
#[derive(Clone)]
pub struct ProjectileMaterialPlugin<M: ProjectileMaterial>(PhantomData<M>);

impl<M: ProjectileMaterial> Default for ProjectileMaterialPlugin<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M: ProjectileMaterial> ProjectileMaterialPlugin<M> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<M: ProjectileMaterial> Plugin for ProjectileMaterialPlugin<M> {
    fn build(&self, app: &mut App) {
        app.init_asset::<M>()
            .add_plugins((RenderAssetPlugin::<PreparedProjectile<M>>::default(),));
        app.sub_app_mut(RenderApp)
            .add_systems(ExtractSchedule, extract_meta::<M>)
            .add_render_command::<Transparent3d, RenderParticles<M>>()
            .add_render_command::<Opaque3d, RenderParticles<M>>()
            .add_render_command::<AlphaMask3d, RenderParticles<M>>()
            .init_resource::<SpecializedMeshPipelines<ParticlePipeline<M>>>()
            .add_systems(
                Render,
                (
                    queue_particles::<M>.in_set(RenderSet::QueueMeshes),
                    prepare_instance_buffers.in_set(RenderSet::PrepareResources),
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<ParticlePipeline<M>>();
    }
}
/// Data prepared for a [`Material`] instance.
pub struct PreparedProjectile<T: ProjectileMaterial> {
    pub bind_group: BindGroup,
    pub p: PhantomData<T>,
}

impl<M: ProjectileMaterial> RenderAsset for PreparedProjectile<M> {
    type SourceAsset = M;

    type Param = (SRes<RenderDevice>, SRes<ParticlePipeline<M>>, M::Param);

    fn prepare_asset(
        material: Self::SourceAsset,
        (render_device, pipeline, param): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        match material.as_bind_group(&pipeline.material_layout, render_device, param) {
            Ok(prepared) => Ok(PreparedProjectile::<M> {
                bind_group: prepared.bind_group,
                p: PhantomData,
            }),
            Err(AsBindGroupError::RetryNextUpdate) => {
                Err(PrepareAssetError::RetryNextUpdate(material))
            }
            Err(e) => Err(PrepareAssetError::AsBindGroupError(e)),
        }
    }
}

fn queue_particles<M: ProjectileMaterial>(
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    custom_pipeline: Res<ParticlePipeline<M>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<ParticlePipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    material_instances: Res<ExtractedProjectileMeta<M>>,
    material_meshes: Res<ExtractedProjectileBuffers>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut views: Query<(Entity, &ExtractedView, &Msaa)>,
) {
    let draw_custom = transparent_3d_draw_functions
        .read()
        .id::<RenderParticles<M>>();

    for (view_entity, view, msaa) in &mut views {
        let msaa_key = MeshPipelineKey::from_msaa_samples(msaa.samples());

        let Some(transparent_phase) = transparent_render_phases.get_mut(&view_entity) else {
            continue;
        };

        let view_key = msaa_key | MeshPipelineKey::from_hdr(view.hdr);
        let rangefinder = view.rangefinder3d();
        for entity in material_meshes.entities() {
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*entity) else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let mut key =
                view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology());
            if let Some(mode) = material_instances
                .entity_material
                .get(entity)
                .and_then(|m| material_instances.alpha_modes.get(m))
            {
                key |= alpha_mode_pipeline_key(*mode, msaa);
            }
            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();
            transparent_phase.add(Transparent3d {
                entity: (**entity, *entity),
                pipeline,
                draw_function: draw_custom,
                distance: rangefinder.distance_translation(&mesh_instance.translation),
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::NONE,
            });
        }
    }
}

#[derive(Clone)]
pub struct ParticleInstanceBuffer {
    pub(crate) buffer: Buffer,
    pub(crate) length: usize,
}

fn prepare_instance_buffers(
    mut commands: Commands,
    query: Res<ExtractedProjectileBuffers>,
    render_device: Res<RenderDevice>,
) {
    let mut result = PreparedInstanceBuffers::default();
    for (entity, instance_data) in query.extracted_buffers.iter() {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("particle instance buffer"),
            contents: instance_data.as_bytes(),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });
        result.buffers.insert(
            *entity,
            ParticleInstanceBuffer {
                buffer,
                length: instance_data.len(),
            },
        );
    }
    for (from, to) in query.particle_ref.iter() {
        if let Some(buf) = result.buffers.get(to) {
            result.buffers.insert(*from, buf.clone());
        }
    }
    for (entity, buffer) in query.compiled_buffers.iter() {
        result.buffers.insert(*entity, buffer.clone());
    }
    commands.insert_resource(result);
}

#[derive(Resource)]
pub struct ParticlePipeline<M: ProjectileMaterial> {
    mesh_pipeline: MeshPipeline,
    vertex_shader: Handle<Shader>,
    fragment_shader: Handle<Shader>,
    material_layout: BindGroupLayout,
    p: PhantomData<M>,
}

impl<M: ProjectileMaterial> FromWorld for ParticlePipeline<M> {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>();
        let render_device = world.resource::<RenderDevice>();
        ParticlePipeline {
            mesh_pipeline: mesh_pipeline.clone(),
            vertex_shader: match M::vertex_shader() {
                ShaderRef::Default => PARTICLE_VERTEX.clone(),
                ShaderRef::Handle(handle) => handle.clone(),
                ShaderRef::Path(path) => world.resource::<AssetServer>().load(path),
            },
            fragment_shader: match M::fragment_shader() {
                ShaderRef::Default => PARTICLE_FRAGMENT.clone(),
                ShaderRef::Handle(handle) => handle.clone(),
                ShaderRef::Path(path) => world.resource::<AssetServer>().load(path),
            },
            material_layout: M::bind_group_layout(render_device),
            p: PhantomData,
        }
    }
}

impl<M: ProjectileMaterial> SpecializedMeshPipeline for ParticlePipeline<M> {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;
        descriptor.vertex.shader = self.vertex_shader.clone();
        descriptor
            .vertex
            .buffers
            .push(<M::InstanceBuffer as ProjectileInstanceBuffer>::descriptor());
        descriptor.layout.insert(2, self.material_layout.clone());
        descriptor.fragment.as_mut().unwrap().shader = self.fragment_shader.clone();
        Ok(descriptor)
    }
}

type RenderParticles<M> = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetParticleBindGroup<M, 2>,
    DrawParticlesInstanced,
);

pub struct SetParticleBindGroup<M: ProjectileMaterial, const I: usize>(PhantomData<M>);

impl<P: PhaseItem, M: ProjectileMaterial, const I: usize> RenderCommand<P>
    for SetParticleBindGroup<M, I>
{
    type Param = (
        SRes<RenderAssets<PreparedProjectile<M>>>,
        SRes<ExtractedProjectileMeta<M>>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: Option<()>,
        (materials, material_instances): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let materials = materials.into_inner();
        let material_instances = material_instances.into_inner();

        let Some(material_asset_id) = material_instances.entity_material.get(&item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };
        let Some(material) = materials.get(*material_asset_id) else {
            return RenderCommandResult::Skip;
        };
        pass.set_bind_group(I, &material.bind_group, &[]);
        RenderCommandResult::Success
    }
}

struct DrawParticlesInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawParticlesInstanced {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
        SRes<PreparedInstanceBuffers>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _: Option<()>,
        (meshes, render_mesh_instances, mesh_allocator, instance_buffers): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // A borrow check workaround.
        let mesh_allocator = mesh_allocator.into_inner();

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };
        let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id) else {
            return RenderCommandResult::Skip;
        };
        let Some(instance_buffer) = instance_buffers
            .into_inner()
            .buffers
            .get(&item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };
        let Some(vertex_buffer_slice) =
            mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id)
        else {
            return RenderCommandResult::Skip;
        };

        // Not allowed in wgpu.
        if instance_buffer.length == 0 {
            return RenderCommandResult::Skip;
        }

        pass.set_vertex_buffer(0, vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.buffer.slice(..));

        match &gpu_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                index_format,
                count,
            } => {
                let Some(index_buffer_slice) =
                    mesh_allocator.mesh_index_slice(&mesh_instance.mesh_asset_id)
                else {
                    return RenderCommandResult::Skip;
                };

                pass.set_index_buffer(index_buffer_slice.buffer.slice(..), 0, *index_format);
                pass.draw_indexed(
                    index_buffer_slice.range.start..(index_buffer_slice.range.start + count),
                    vertex_buffer_slice.range.start as i32,
                    0..instance_buffer.length as u32,
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(vertex_buffer_slice.range, 0..instance_buffer.length as u32);
            }
        }
        RenderCommandResult::Success
    }
}
