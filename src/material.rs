use bevy::{
    asset::{Asset, Handle},
    color::LinearRgba,
    ecs::system::SystemParamItem,
    prelude::Component,
    reflect::TypePath,
    render::{
        alpha::AlphaMode,
        mesh::VertexBufferLayout,
        render_resource::{
            AsBindGroup, AsBindGroupError, BindGroupLayout, BindGroupLayoutEntry, Face, ShaderRef,
            UnpreparedBindGroup,
        },
        renderer::RenderDevice,
    },
};
use bevy_image::Image;
use bytemuck::Pod;

use crate::{
    pipeline::InstancedPipelineKey,
    shader::{PARTICLE_FRAGMENT, PARTICLE_VERTEX},
    DefaultInstanceBuffer,
};

pub trait ProjectileInstanceBuffer: Pod {
    fn descriptor() -> VertexBufferLayout;
}

pub trait InstancedMaterial: Asset + AsBindGroup + Clone {
    type InstanceBuffer: ProjectileInstanceBuffer;

    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn cull_mode(&self) -> Option<Face> {
        None
    }

    fn billboard(&self) -> bool {
        false
    }

    fn pipeline_key(&self) -> InstancedPipelineKey {
        let cull_key = match self.cull_mode() {
            Some(Face::Front) => InstancedPipelineKey::CULL_FRONT,
            Some(Face::Back) => InstancedPipelineKey::CULL_BACK,
            None => InstancedPipelineKey::empty(),
        };
        let billboard_key = if self.billboard() {
            InstancedPipelineKey::BILLBOARD
        } else {
            InstancedPipelineKey::empty()
        };
        cull_key | billboard_key
    }
}

pub trait InstancedMaterialExtension: Asset + AsBindGroup + Clone {
    type InstanceBuffer: ProjectileInstanceBuffer;

    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn alpha_mode(&self) -> Option<AlphaMode> {
        None
    }

    fn cull_mode(&self) -> Option<Option<Face>> {
        None
    }

    fn billboard(&self) -> bool {
        false
    }

    fn pipeline_key(&self) -> InstancedPipelineKey {
        let cull_key = match self.cull_mode() {
            Some(Some(Face::Front)) => InstancedPipelineKey::CULL_FRONT,
            Some(Some(Face::Back)) => InstancedPipelineKey::CULL_BACK,
            _ => InstancedPipelineKey::empty(),
        };
        let billboard_key = if self.billboard() {
            InstancedPipelineKey::BILLBOARD
        } else {
            InstancedPipelineKey::empty()
        };
        cull_key | billboard_key
    }
}

/// Component form of [`InstancedMaterial`], provides a material for [`ProjectileCluster`](crate::ProjectileCluster).
#[derive(Debug, Component)]
pub struct InstancedMaterial3d<T: InstancedMaterial>(pub Handle<T>);

/// [`InstancedMaterial`] that displays an unlit combination of `base_color` and `texture` on a mesh.
#[derive(Debug, Clone, Default, PartialEq, TypePath, Asset, AsBindGroup)]
pub struct StandardParticle {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
    pub alpha_mode: AlphaMode,
    pub cull_mode: Option<Face>,
    // todo: screen/pixel space billboard?
    /// If true, render the object at the center of the projectile facing the camera **orthographically** and **to scale**.
    ///
    /// Since we allow rotation,
    /// in order for the projectile to actually face the camera,
    /// its local rotation must be either 0 or around the Z axis.
    pub billboard: bool,
}

impl InstancedMaterial for StandardParticle {
    type InstanceBuffer = DefaultInstanceBuffer;

    fn vertex_shader() -> ShaderRef {
        ShaderRef::Handle(PARTICLE_VERTEX.clone())
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(PARTICLE_FRAGMENT.clone())
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    fn cull_mode(&self) -> Option<Face> {
        self.cull_mode
    }

    fn billboard(&self) -> bool {
        self.billboard
    }
}

/// Extended version of a base [`InstancedMaterial`] using [`InstancedMaterialExtension`].
#[derive(Debug, Clone, Default, TypePath, Asset)]
pub struct ExtendedInstancedMaterial<
    B: InstancedMaterial,
    E: InstancedMaterialExtension<InstanceBuffer = B::InstanceBuffer>,
> {
    pub base: B,
    pub extension: E,
}

impl<B: InstancedMaterial, E: InstancedMaterialExtension<InstanceBuffer = B::InstanceBuffer>>
    InstancedMaterial for ExtendedInstancedMaterial<B, E>
{
    type InstanceBuffer = B::InstanceBuffer;

    fn vertex_shader() -> ShaderRef {
        match E::vertex_shader() {
            ShaderRef::Default => B::vertex_shader(),
            shader => shader,
        }
    }

    fn fragment_shader() -> ShaderRef {
        match E::fragment_shader() {
            ShaderRef::Default => B::fragment_shader(),
            shader => shader,
        }
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.extension
            .alpha_mode()
            .unwrap_or(self.base.alpha_mode())
    }

    fn cull_mode(&self) -> Option<Face> {
        self.extension.cull_mode().unwrap_or(self.base.cull_mode())
    }

    fn billboard(&self) -> bool {
        self.extension.billboard() | self.base.billboard()
    }

    fn pipeline_key(&self) -> InstancedPipelineKey {
        self.extension.pipeline_key() | self.base.pipeline_key()
    }
}

impl<B: InstancedMaterial, E: InstancedMaterialExtension<InstanceBuffer = B::InstanceBuffer>>
    AsBindGroup for ExtendedInstancedMaterial<B, E>
{
    type Data = (<B as AsBindGroup>::Data, <E as AsBindGroup>::Data);
    type Param = (<B as AsBindGroup>::Param, <E as AsBindGroup>::Param);

    fn unprepared_bind_group(
        &self,
        layout: &BindGroupLayout,
        render_device: &RenderDevice,
        (base_param, extended_param): &mut SystemParamItem<'_, '_, Self::Param>,
    ) -> Result<UnpreparedBindGroup<Self::Data>, AsBindGroupError> {
        // add together the bindings of the base material and the user material
        let UnpreparedBindGroup {
            mut bindings,
            data: base_data,
        } = B::unprepared_bind_group(&self.base, layout, render_device, base_param)?;
        let extended_bindgroup =
            E::unprepared_bind_group(&self.extension, layout, render_device, extended_param)?;

        bindings.extend(extended_bindgroup.bindings);

        Ok(UnpreparedBindGroup {
            bindings,
            data: (base_data, extended_bindgroup.data),
        })
    }

    fn bind_group_layout_entries(render_device: &RenderDevice) -> Vec<BindGroupLayoutEntry>
    where
        Self: Sized,
    {
        // add together the bindings of the standard material and the user material
        let mut entries = B::bind_group_layout_entries(render_device);
        entries.extend(E::bind_group_layout_entries(render_device));
        entries
    }
}
