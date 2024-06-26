//! Module for rendering trails.

use std::ops::Range;

use bevy::{
    asset::{Assets, Handle},
    math::{Vec2, Vec3},
    prelude::{Component, Entity, Query, ResMut, Without},
    render::{
        camera::Camera,
        mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues},
        render_asset::RenderAssetUsages,
    },
};

use crate::{Particle, ParticleBuffer, ParticleInstance, ParticleSystem};

/// A buffer of vertices on a curve.
///
/// Typically implemented as a [`RingBuffer`](crate::RingBuffer) of a particle like type.
pub trait TrailBuffer: Copy + Send + Sync + 'static {
    /// Advance by time.
    fn update(&mut self, dt: f32);
    /// Returns true if expired, must be ordered by lifetime with items within the same trail.
    fn expired(&self) -> bool;
    /// Build mesh, you may find [`TrailMeshBuilder`] useful in implementing this.
    #[allow(unused_variables)]
    fn build_trail(&self, mesh: &mut Mesh);

    /// By default we only generate position, uv, normal and indices.
    fn default_mesh() -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<Vec3>::new())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<Vec3>::new())
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<Vec2>::new())
            .with_inserted_indices(Indices::U32(Vec::new()))
    }
}

/// [`Particle`] that contains a [`TrailBuffer`] and can be rendered as a mesh.
///
/// You might find [`RingBuffer`](crate::RingBuffer) useful in implementing [`TrailBuffer`].
pub trait TrailedParticle: Particle {
    /// Usually a fixed sized ring buffer of points that constructs a mesh.
    type TrailBuffer: TrailBuffer;

    /// Convert to [`TrailBuffer`].
    fn as_trail_buffer(&self) -> Self::TrailBuffer;
    /// Convert to a mutable [`TrailBuffer`].
    fn as_trail_buffer_mut(&mut self) -> &mut Self::TrailBuffer;
}

/// ParticleSystem with [`Particle`] as a [`TrailedParticle`].
pub trait TrailParticleSystem {
    /// Generate a default [`Mesh`].
    fn default_mesh(&self) -> Mesh;
    /// Build a trail [`Mesh`].
    fn build_trail(&self, buffer: &ParticleBuffer, mesh: &mut Mesh);
}

impl<T> TrailParticleSystem for T
where
    T: ParticleSystem<Particle: TrailedParticle>,
{
    fn default_mesh(&self) -> Mesh {
        <T::Particle as TrailedParticle>::TrailBuffer::default_mesh()
    }

    fn build_trail(&self, buffer: &ParticleBuffer, mesh: &mut Mesh) {
        for particle in buffer.get::<T::Particle>() {
            if particle.is_expired() {
                continue;
            }
            particle.as_trail_buffer().build_trail(mesh);
        }
        if let Some(detached) = buffer.detached::<<T::Particle as TrailedParticle>::TrailBuffer>() {
            for trail in detached {
                trail.build_trail(mesh);
            }
        }
    }
}

// Removed items but preserve allocation.
fn clean_mesh(mesh: &mut Mesh) {
    match mesh.indices_mut() {
        Some(Indices::U16(indices)) => indices.clear(),
        Some(Indices::U32(indices)) => indices.clear(),
        None => mesh.insert_indices(Indices::U32(Vec::new())),
    }
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        positions.clear()
    } else {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<Vec3>::new())
    }
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
    {
        positions.clear()
    } else {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<Vec3>::new())
    }
    if let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        uvs.clear()
    } else {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<Vec2>::new())
    }
}

/// Place this next to a [`MaterialMeshBundle`](bevy::pbr::MaterialMeshBundle)
/// (or simply `Handle<Mesh>`) to render trails of a particle system.
///
/// # Tips
/// * You can leave [`Handle<Mesh>`] at default as a new mesh will automatically be created.
/// * Remember to set cull mode to `None`.
#[derive(Debug, Component)]
pub struct TrailMeshOf(pub Entity);

/// System for rendering trails.
pub fn trail_rendering(
    mut meshes: ResMut<Assets<Mesh>>,
    mut particles: Query<(&mut ParticleInstance, &mut ParticleBuffer), Without<Camera>>,
    mut trails: Query<(&TrailMeshOf, &mut Handle<Mesh>)>,
) {
    for (trail, mut handle) in trails.iter_mut() {
        let Ok((mut particle, buffer)) = particles.get_mut(trail.0) else {
            continue;
        };
        let Some(trail) = particle.as_trail_particle_system() else {
            continue;
        };
        let modify = |mesh: &mut Mesh| {
            clean_mesh(mesh);
            trail.build_trail(&buffer, mesh);
        };

        if handle.id() == Handle::<Mesh>::default().id() {
            let mut mesh = trail.default_mesh();
            modify(&mut mesh);
            *handle = meshes.add(mesh);
        } else {
            match meshes.get_mut(handle.as_ref()) {
                Some(mesh) => modify(mesh),
                None => {
                    let mut mesh = trail.default_mesh();
                    modify(&mut mesh);
                    *handle = meshes.add(mesh);
                }
            }
        }
    }
}

/// A Builder that generates a plane mesh representing a trail.
pub struct TrailMeshBuilder<'t> {
    mesh: &'t mut Mesh,
    buffer: Vec<(Vec3, Vec3, f32)>,
}

impl TrailMeshBuilder<'_> {
    pub fn new(mesh: &mut Mesh) -> TrailMeshBuilder {
        TrailMeshBuilder {
            mesh,
            buffer: Vec::new(),
        }
    }

    /// Build a row of faces from a stream of points.
    ///
    /// The inputs are, in order, `(position, tangent, width)`.
    pub fn build_plane(
        &mut self,
        iter: impl IntoIterator<Item = (Vec3, Vec3, f32)>,
        uv_range: Range<f32>,
    ) {
        self.buffer.clear();
        self.buffer.extend(iter);
        let len = self.buffer.len();
        if len < 2 {
            return;
        }
        let dx = (uv_range.end - uv_range.start) / len as f32;

        let origin = if let Some(VertexAttributeValues::Float32x3(positions)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            positions.len()
        } else {
            return;
        };
        match self.mesh.indices_mut() {
            Some(Indices::U16(indices)) => {
                for i in 0..len - 1 {
                    let i = (i * 2 + origin) as u16;
                    indices.extend([i, i + 1, i + 2, i + 1, i + 3, i + 2])
                }
            }
            Some(Indices::U32(indices)) => {
                for i in 0..len - 1 {
                    let i = (i * 2 + origin) as u32;
                    indices.extend([i, i + 1, i + 2, i + 1, i + 3, i + 2])
                }
            }
            None => return,
        }

        if let Some(VertexAttributeValues::Float32x3(positions)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            for (pos, dir, w) in self.buffer.iter().copied() {
                positions.push((pos - dir * w).to_array());
                positions.push((pos + dir * w).to_array());
            }
        }
        if let Some(VertexAttributeValues::Float32x3(normals)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
        {
            for _ in 0..len {
                // TODO: implement normals.
                normals.push([1.0, 0.0, 0.0]);
                normals.push([1.0, 0.0, 0.0]);
            }
        }

        if let Some(VertexAttributeValues::Float32x2(uvs)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        {
            for i in 0..len {
                uvs.push([uv_range.start + i as f32 * dx, 0.0]);
                uvs.push([uv_range.start + i as f32 * dx, 1.0]);
            }
        }
    }
}
