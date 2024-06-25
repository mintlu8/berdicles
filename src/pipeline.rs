//! A shader that renders a mesh multiple times in one draw call.

use std::{marker::PhantomData, mem::size_of};

use bevy::{
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    pbr::{
        MeshPipeline, MeshPipelineKey, RenderMaterialInstances, RenderMeshInstances,
        SetMeshBindGroup, SetMeshViewBindGroup,
    },
    prelude::*,
    render::{
        extract_instances::ExtractInstancesPlugin,
        mesh::{GpuBufferInfo, GpuMesh, MeshVertexBufferLayoutRef},
        render_asset::{PrepareAssetError, RenderAsset, RenderAssetPlugin, RenderAssets},
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
            RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
        },
        render_resource::*,
        renderer::RenderDevice,
        texture::{FallbackImage, GpuImage},
        view::ExtractedView,
        Render, RenderApp, RenderSet,
    },
};

use crate::{
    shader::{PARTICLE_FRAGMENT, PARTICLE_VERTEX},
    ExtractedParticle, ExtractedParticleBuffer,
};

#[derive(Clone)]
pub struct ParticleMaterialPlugin<M: Material>(PhantomData<M>);

impl<M: Material> Default for ParticleMaterialPlugin<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M: Material> Plugin for ParticleMaterialPlugin<M> {
    fn build(&self, app: &mut App) {
        app.init_asset::<M>().add_plugins((
            ExtractInstancesPlugin::<AssetId<M>>::extract_visible(),
            RenderAssetPlugin::<PreparedParticle<M>>::default(),
        ));
        app.sub_app_mut(RenderApp)
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
pub struct PreparedParticle<T: Material> {
    pub bind_group: BindGroup,
    pub p: PhantomData<T>,
}

impl<M: Material> RenderAsset for PreparedParticle<M> {
    type SourceAsset = M;

    type Param = (
        SRes<RenderDevice>,
        SRes<RenderAssets<GpuImage>>,
        SRes<FallbackImage>,
        SRes<ParticlePipeline<M>>,
    );

    fn prepare_asset(
        material: Self::SourceAsset,
        (render_device, images, fallback_image, pipeline): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        match material.as_bind_group(
            &pipeline.material_layout,
            render_device,
            images,
            fallback_image,
        ) {
            Ok(prepared) => Ok(PreparedParticle::<M> {
                bind_group: prepared.bind_group,
                p: PhantomData,
            }),
            Err(AsBindGroupError::RetryNextUpdate) => {
                Err(PrepareAssetError::RetryNextUpdate(material))
            }
        }
    }
}

fn queue_particles<M: Material>(
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    custom_pipeline: Res<ParticlePipeline<M>>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<ParticlePipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<GpuMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    material_meshes: Query<Entity, With<ExtractedParticleBuffer>>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut views: Query<(Entity, &ExtractedView)>,
) {
    let draw_custom = transparent_3d_draw_functions
        .read()
        .id::<RenderParticles<M>>();

    let msaa_key = MeshPipelineKey::from_msaa_samples(msaa.samples());

    for (view_entity, view) in &mut views {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view_entity) else {
            continue;
        };

        let view_key = msaa_key | MeshPipelineKey::from_hdr(view.hdr);
        let rangefinder = view.rangefinder3d();
        for entity in &material_meshes {
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(entity) else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let key =
                view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology());
            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();
            transparent_phase.add(Transparent3d {
                entity,
                pipeline,
                draw_function: draw_custom,
                distance: rangefinder.distance_translation(&mesh_instance.translation),
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::NONE,
            });
        }
    }
}

#[derive(Component)]
struct InstanceBuffer {
    buffer: Buffer,
    length: usize,
}

fn prepare_instance_buffers(
    mut commands: Commands,
    query: Query<(Entity, &ExtractedParticleBuffer)>,
    render_device: Res<RenderDevice>,
) {
    for (entity, instance_data) in &query {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("instance data buffer"),
            contents: instance_data.as_bytes(),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });
        commands.entity(entity).insert(InstanceBuffer {
            buffer,
            length: instance_data.len(),
        });
    }
}

#[derive(Resource)]
pub struct ParticlePipeline<M: Material> {
    mesh_pipeline: MeshPipeline,
    vertex_shader: Handle<Shader>,
    fragment_shader: Handle<Shader>,
    material_layout: BindGroupLayout,
    p: PhantomData<M>,
}

impl<M: Material> FromWorld for ParticlePipeline<M> {
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

impl<M: Material> SpecializedMeshPipeline for ParticlePipeline<M> {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        descriptor.vertex.shader = self.vertex_shader.clone();
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: size_of::<ExtractedParticle>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: 0,
                    shader_location: 3,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 4,
                    shader_location: 4,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 8,
                    shader_location: 5,
                },
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: 12,
                    shader_location: 6,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 7,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 8,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 48,
                    shader_location: 9,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 64,
                    shader_location: 10,
                },
            ],
        });
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

pub struct SetParticleBindGroup<M: Material, const I: usize>(PhantomData<M>);

impl<P: PhaseItem, M: Material, const I: usize> RenderCommand<P> for SetParticleBindGroup<M, I> {
    type Param = (
        SRes<RenderAssets<PreparedParticle<M>>>,
        SRes<RenderMaterialInstances<M>>,
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

        let Some(material_asset_id) = material_instances.get(&item.entity()) else {
            return RenderCommandResult::Failure;
        };
        let Some(material) = materials.get(*material_asset_id) else {
            return RenderCommandResult::Failure;
        };
        pass.set_bind_group(I, &material.bind_group, &[]);
        RenderCommandResult::Success
    }
}

struct DrawParticlesInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawParticlesInstanced {
    type Param = (
        Res<'static, RenderAssets<GpuMesh>>,
        Res<'static, RenderMeshInstances>,
    );
    type ViewQuery = ();
    type ItemQuery = &'static InstanceBuffer;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        instance_buffer: Option<&'w InstanceBuffer>,
        (meshes, render_mesh_instances): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.entity())
        else {
            return RenderCommandResult::Failure;
        };
        let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id) else {
            return RenderCommandResult::Failure;
        };
        let Some(instance_buffer) = instance_buffer else {
            return RenderCommandResult::Failure;
        };

        pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.buffer.slice(..));

        match &gpu_mesh.buffer_info {
            GpuBufferInfo::Indexed {
                buffer,
                index_format,
                count,
            } => {
                pass.set_index_buffer(buffer.slice(..), 0, *index_format);
                pass.draw_indexed(0..*count, 0, 0..instance_buffer.length as u32);
            }
            GpuBufferInfo::NonIndexed => {
                pass.draw(0..gpu_mesh.vertex_count, 0..instance_buffer.length as u32);
            }
        }
        RenderCommandResult::Success
    }
}
