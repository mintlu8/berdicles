use bevy::{
    asset::{Asset, Handle},
    color::LinearRgba,
    ecs::system::SystemParamItem,
    image::Image,
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
use bytemuck::Pod;

use crate::{
    shader::{PARTICLE_FRAGMENT, PARTICLE_VERTEX},
    ExtractedProjectile,
};

pub trait ProjectileInstanceBuffer: Pod {
    fn descriptor() -> VertexBufferLayout;
}

pub trait ProjectileMaterial: Asset + AsBindGroup + Clone {
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
}

pub trait ProjectileMaterialExtension: Asset + AsBindGroup + Clone {
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
}

#[derive(Debug, Component)]
pub struct ProjectileMat<T: ProjectileMaterial>(pub Handle<T>);

/// [`ProjectileMaterial`] that displays an unlit combination of `base_color` and `texture` on a mesh.
#[derive(Debug, Clone, Default, PartialEq, TypePath, Asset, AsBindGroup)]
pub struct StandardProjectile {
    #[uniform(0)]
    pub billboard: i32,
    #[uniform(1)]
    pub base_color: LinearRgba,
    #[texture(2)]
    #[sampler(3)]
    pub texture: Handle<Image>,
    pub alpha_mode: AlphaMode,
    pub cull_mode: Option<Face>,
}

impl ProjectileMaterial for StandardProjectile {
    type InstanceBuffer = ExtractedProjectile;

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
}

#[derive(Debug, Clone, Default, TypePath, Asset)]
pub struct ExtendedProjectileMat<
    B: ProjectileMaterial,
    E: ProjectileMaterialExtension<InstanceBuffer = B::InstanceBuffer>,
> {
    pub base: B,
    pub extension: E,
}

impl<B: ProjectileMaterial, E: ProjectileMaterialExtension<InstanceBuffer = B::InstanceBuffer>>
    ProjectileMaterial for ExtendedProjectileMat<B, E>
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
}

impl<B: ProjectileMaterial, E: ProjectileMaterialExtension<InstanceBuffer = B::InstanceBuffer>>
    AsBindGroup for ExtendedProjectileMat<B, E>
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
