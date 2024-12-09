//! Module for rendering trails.

use std::ops::Range;

use bevy::{
    asset::{Asset, Assets, Handle},
    math::{Vec2, Vec3},
    pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial},
    prelude::{Component, Entity, Mesh3d, Query, ResMut},
    reflect::TypePath,
    render::{
        mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues},
        render_asset::RenderAssetUsages,
        render_resource::{AsBindGroup, ShaderRef},
    },
};

use crate::{shader::TRAIL_VERTEX, ProjectileBuffer, ProjectileCluster};

/// Standard material of trails.
pub type TrailMaterial = ExtendedMaterial<StandardMaterial, TrailVertex>;

/// Standard material of trails.
#[derive(Debug, Clone, Default, AsBindGroup, TypePath, Asset)]
pub struct TrailVertex {}

impl MaterialExtension for TrailVertex {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Handle(TRAIL_VERTEX.clone())
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

    if let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_1) {
        uvs.clear()
    } else {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, Vec::<Vec2>::new())
    }
}

/// Place this next to a [`MaterialMeshBundle`](bevy::pbr::MaterialMeshBundle)
/// (or simply `Handle<Mesh>`) to render trails of a particle system.
#[derive(Debug, Component)]
#[require(Mesh3d)]
pub struct TrailMeshOf(pub Entity);

impl Default for TrailMeshOf {
    fn default() -> Self {
        TrailMeshOf(Entity::PLACEHOLDER)
    }
}

impl From<Entity> for TrailMeshOf {
    fn from(value: Entity) -> Self {
        TrailMeshOf(value)
    }
}

/// System for rendering trails.
pub fn trail_rendering(
    mut meshes: ResMut<Assets<Mesh>>,
    mut particles: Query<(&ProjectileCluster, &mut ProjectileBuffer)>,
    mut trails: Query<(&TrailMeshOf, &mut Mesh3d)>,
) {
    for (trail, mut handle) in trails.iter_mut() {
        let Ok((particle, buffer)) = particles.get_mut(trail.0) else {
            continue;
        };
        if buffer.is_uninit() {
            continue;
        }
        let modify = |mesh: &mut Mesh| {
            clean_mesh(mesh);
            particle.render_trail(&buffer, &mut TrailMeshBuilder::new(mesh));
        };

        if handle.id() == Handle::<Mesh>::default().id() {
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
            modify(&mut mesh);
            *handle = meshes.add(mesh).into();
        } else {
            match meshes.get_mut(handle.as_ref()) {
                Some(mesh) => modify(mesh),
                None => {
                    let mut mesh =
                        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
                    modify(&mut mesh);
                    *handle = meshes.add(mesh).into();
                }
            }
        }
    }
}

/// A Builder that generates a plane mesh representing a trail.
pub struct TrailMeshBuilder<'t> {
    mesh: &'t mut Mesh,
    buffer: Vec<(Vec3, f32)>,
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
        iter: impl IntoIterator<Item = (Vec3, f32)>,
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
            for (pos, _) in self.buffer.iter() {
                positions.push(pos.to_array());
                positions.push(pos.to_array());
            }
        }
        if let Some(VertexAttributeValues::Float32x3(normals)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
        {
            let v = (self.buffer[1].0 - self.buffer[0].0).normalize();
            normals.push((-v).to_array());
            normals.push(v.to_array());
            for i in 1..self.buffer.len() - 1 {
                let v = (self.buffer[i + 1].0 - self.buffer[i - 1].0).normalize();
                normals.push((-v).to_array());
                normals.push(v.to_array());
            }
            let i = self.buffer.len() - 1;
            let v = (self.buffer[i].0 - self.buffer[i - 1].0).normalize();
            normals.push((-v).to_array());
            normals.push(v.to_array());
        }

        if let Some(VertexAttributeValues::Float32x2(uvs)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        {
            for i in 0..len {
                uvs.push([uv_range.start + i as f32 * dx, 0.0]);
                uvs.push([uv_range.start + i as f32 * dx, 1.0]);
            }
        }

        if let Some(VertexAttributeValues::Float32x2(uvs)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_UV_1)
        {
            for (_, w) in self.buffer.iter() {
                uvs.push([*w, *w]);
                uvs.push([*w, *w]);
            }
        }
    }
}
