use bevy::{
    asset::{Asset, Handle},
    color::LinearRgba,
    pbr::Material,
    prelude::{Bundle, Deref, DerefMut, Entity},
    reflect::TypePath,
    render::{
        alpha::AlphaMode,
        mesh::Mesh,
        render_resource::{AsBindGroup, ShaderRef},
        texture::Image,
        view::VisibilityBundle,
    },
    transform::components::{GlobalTransform, Transform},
};

use crate::{
    shader::{PARTICLE_DBG_FRAGMENT, PARTICLE_FRAGMENT, PARTICLE_VERTEX},
    sub::{ParticleEventBuffer, ParticleParent},
    trail::TrailMeshOf,
    ParticleBuffer, ParticleInstance, ParticleRef,
};

/// [`Material`] that displays an unlit combination of `base_color` and `texture` on a mesh.
#[derive(Debug, Clone, Default, PartialEq, TypePath, Asset, AsBindGroup)]
pub struct StandardParticle<const BACKFACE_CULLING: bool = true> {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
    pub alpha_mode: AlphaMode,
}

impl<const CULL: bool> Material for StandardParticle<CULL> {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Handle(PARTICLE_VERTEX.clone())
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(PARTICLE_FRAGMENT.clone())
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}

/// [`Material`] that displays useful debug info of a particle.
#[derive(Debug, Clone, Default, PartialEq, TypePath, Asset, AsBindGroup)]
pub struct DebugParticle {}

impl Material for DebugParticle {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Handle(PARTICLE_VERTEX.clone())
    }
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(PARTICLE_DBG_FRAGMENT.clone())
    }
}

/// A Bundle of a particle system.
#[derive(Debug, Bundle)]
pub struct ParticleSystemBundle<M: Material> {
    /// A type erased [`ParticleSystem`](crate::ParticleSystem).
    pub particle_system: ParticleInstance,
    /// Does not need to be created by the user.
    pub particle_buffer: ParticleBuffer,
    /// Mesh shape of the particle.
    pub mesh: Handle<Mesh>,
    /// Material of the particle.
    pub material: Handle<M>,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: VisibilityBundle,
}

impl<M: Material> Default for ParticleSystemBundle<M> {
    fn default() -> Self {
        Self {
            particle_system: Default::default(),
            particle_buffer: Default::default(),
            mesh: Default::default(),
            material: Default::default(),
            transform: Default::default(),
            global_transform: Default::default(),
            visibility: Default::default(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Bundle, Deref, DerefMut)]
pub struct ParentedParticleSystemBundle<M: Material> {
    #[deref]
    bundle: ParticleSystemBundle<M>,
    pub parent: ParticleParent,
}

#[doc(hidden)]
#[derive(Debug, Bundle, Deref, DerefMut)]
pub struct EventParticleSystemBundle<M: Material> {
    #[deref]
    bundle: ParticleSystemBundle<M>,
    pub events: ParticleEventBuffer,
}

#[doc(hidden)]
#[derive(Debug, Bundle, Deref, DerefMut)]
pub struct ParentedEventParticleSystemBundle<M: Material> {
    #[deref]
    bundle: ParticleSystemBundle<M>,
    pub parent: ParticleParent,
    pub events: ParticleEventBuffer,
}

impl<M: Material> ParticleSystemBundle<M> {
    /// Add a parent to the particle system.
    pub fn parented(self, entity: Entity) -> ParentedParticleSystemBundle<M> {
        ParentedParticleSystemBundle {
            bundle: self,
            parent: ParticleParent(entity),
        }
    }

    /// Add an [`ParticleEventBuffer`] to the current particle system.
    pub fn with_events(self) -> EventParticleSystemBundle<M> {
        EventParticleSystemBundle {
            bundle: self,
            events: ParticleEventBuffer::default(),
        }
    }
}

impl<M: Material> ParentedParticleSystemBundle<M> {
    /// Add an [`ParticleEventBuffer`] to the current particle system.
    pub fn with_events(self) -> ParentedEventParticleSystemBundle<M> {
        ParentedEventParticleSystemBundle {
            bundle: self.bundle,
            parent: self.parent,
            events: ParticleEventBuffer::default(),
        }
    }
}

impl<M: Material> EventParticleSystemBundle<M> {
    /// Add a parent to the particle system.
    pub fn parented(self, entity: Entity) -> ParentedEventParticleSystemBundle<M> {
        ParentedEventParticleSystemBundle {
            bundle: self.bundle,
            events: self.events,
            parent: ParticleParent(entity),
        }
    }
}

/// A [`Material`] and [`Mesh`] that renders based on
/// another [`ParticleInstance`]'s output.
#[derive(Debug, Bundle)]
pub struct ParticleRefBundle<M: Material> {
    /// A reference to a [`ParticleInstance`].
    pub particles: ParticleRef,
    /// Mesh shape of the particle.
    pub mesh: Handle<Mesh>,
    /// Material of the particle.
    pub material: Handle<M>,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: VisibilityBundle,
}

impl<M: Material> Default for ParticleRefBundle<M> {
    fn default() -> Self {
        Self {
            particles: Default::default(),
            mesh: Default::default(),
            material: Default::default(),
            transform: Default::default(),
            global_transform: Default::default(),
            visibility: Default::default(),
        }
    }
}

/// A Bundle of a particle trails.
#[derive(Debug, Bundle, Default)]
pub struct ParticleTrailsBundle<M: Material> {
    /// A reference to a [`ParticleSystem`](crate::ParticleSystem).
    pub of: TrailMeshOf,
    /// Mesh shape of the trails, can be left as `default`.
    pub mesh: Handle<Mesh>,
    /// Material of the trails.
    pub material: Handle<M>,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: VisibilityBundle,
}
