//! This example demonstrates
//! how to reuse a simulation result with different material and mesh.
mod util;

use berdicles::{
    util::{random_cone, transform_from_derivative},
    DefaultInstanceBuffer, ExpirationState, ExtendedInstancedMaterial, InstancedMaterial3d,
    InstancedMaterialExtension, InstancedMaterialPlugin, Projectile, ProjectileCluster,
    ProjectilePlugin, ProjectileRef, ProjectileSystem, StandardParticle,
};
use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
    window::PresentMode,
};
use std::f32::consts::PI;
use util::{uv_debug_texture, FPSPlugin};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: PresentMode::AutoNoVsync,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
        )
        .add_plugins(InstancedMaterialPlugin::<
            ExtendedInstancedMaterial<StandardParticle, SpinMat>,
        >::default())
        .add_plugins(|a: &mut App| {
            a.world_mut()
                .resource_mut::<Assets<Shader>>()
                .insert(&SPIN_SHADER, Shader::from_wgsl(SPIN_VERTEX, "spin.wgsl"))
        })
        .add_plugins(FPSPlugin)
        .add_plugins(ProjectilePlugin)
        .add_systems(Startup, setup)
        .run();
}

#[derive(Debug, Clone, Copy)]
pub struct MyParticle {
    pub seed: f32,
    pub life_time: f32,
}

impl Projectile for MyParticle {
    fn get_seed(&self) -> f32 {
        self.seed
    }

    fn get_lifetime(&self) -> f32 {
        self.life_time
    }

    fn get_transform(&self) -> Transform {
        let f = |t| random_cone(Vec3::Y, f32::to_radians(30.), self.seed) * t * 2.;
        transform_from_derivative(f, self.life_time + 1.)
    }

    fn get_color(&self) -> Srgba {
        Srgba::WHITE
    }

    fn update(&mut self, dt: f32) {
        self.life_time += dt;
    }

    fn expiration_state(&self) -> ExpirationState {
        if self.life_time > 20.0 {
            ExpirationState::Fizzle
        } else {
            ExpirationState::None
        }
    }
}

pub struct MySpawner(f32);

impl ProjectileSystem for MySpawner {
    type Projectile = MyParticle;

    fn capacity(&self) -> usize {
        10000
    }

    fn spawn_step(&mut self, time: f32) -> usize {
        self.0 += time * 10.;
        let result = self.0.floor() as usize;
        self.0 = self.0.fract();
        result
    }

    fn build_particle(&self, seed: f32) -> Self::Projectile {
        MyParticle {
            seed,
            life_time: 0.,
        }
    }
}

const SPIN_VERTEX: &str = r#"
    #import berdicle::{Vertex, VertexOutput};
    #import bevy_pbr::mesh_functions::get_world_from_local;
    #import bevy_pbr::view_transformations::position_world_to_clip;

    @vertex
    fn vertex(vertex: Vertex) -> VertexOutput {
        var out: VertexOutput;
        var pos = vertex.position;
        var t = vertex.lifetime * 4.0 + vertex.seed * 6.28;
        var px = pos.x * cos(t) - pos.y * sin(t);
        var py = pos.x * sin(t) + pos.y * cos(t);
        pos.x = px;
        pos.y = py;
        var x = dot(vec4(pos, 1.0), vertex.transform_x);
        var y = dot(vec4(pos, 1.0), vertex.transform_y);
        var z = dot(vec4(pos, 1.0), vertex.transform_z);
        out.clip_position = position_world_to_clip(vec3(x, y, z));
        out.id = vertex.id;
        out.lifetime = vertex.lifetime;
        out.fac = vertex.fac;
        out.seed = vertex.seed;
        out.color = vertex.color;
        out.normal = vertex.normal;
        out.uv = vertex.uv;
        return out;
    }
"#;

pub static SPIN_SHADER: Handle<Shader> = Handle::weak_from_u128(123123412412412412);

#[derive(Debug, Clone, Copy, TypePath, Asset, AsBindGroup)]
struct SpinMat {}

impl InstancedMaterialExtension for SpinMat {
    type InstanceBuffer = DefaultInstanceBuffer;

    fn vertex_shader() -> ShaderRef {
        ShaderRef::Handle(SPIN_SHADER.clone())
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials2: ResMut<Assets<StandardParticle>>,
    mut materials3: ResMut<Assets<ExtendedInstancedMaterial<StandardParticle, SpinMat>>>,
) {
    let e = commands
        .spawn((
            ProjectileCluster::new(MySpawner(0.)),
            Mesh3d(
                meshes.add(
                    Mesh::from(
                        Cone {
                            radius: 0.5,
                            height: 0.5,
                        }
                        .mesh(),
                    )
                    .translated_by(Vec3::new(0., 0.25, 0.))
                    .rotated_by(Quat::from_rotation_x(-PI / 2.0)),
                ),
            ),
            InstancedMaterial3d(materials2.add(StandardParticle {
                base_color: LinearRgba::new(2., 2., 2., 1.),
                texture: images.add(uv_debug_texture()),
                alpha_mode: AlphaMode::Opaque,
                ..Default::default()
            })),
        ))
        .id();

    commands.spawn((
        ProjectileRef(e),
        Mesh3d(
            meshes.add(
                Mesh::from(
                    Cone {
                        radius: 0.5,
                        height: 0.5,
                    }
                    .mesh(),
                )
                .translated_by(Vec3::new(0., 0.25, 0.))
                .rotated_by(Quat::from_rotation_x(PI / 2.0)),
            ),
        ),
        InstancedMaterial3d(materials2.add(StandardParticle {
            base_color: LinearRgba::new(2., 2., 2., 1.),
            texture: images.add(uv_debug_texture()),
            alpha_mode: AlphaMode::Opaque,
            ..Default::default()
        })),
    ));

    commands.spawn((
        ProjectileRef(e),
        Mesh3d(meshes.add({
            let mut mesh =
                Mesh::from(Sphere::new(0.1).mesh()).translated_by(Vec3::new(0.8, 0., 0.));
            mesh.merge(&Mesh::from(Sphere::new(0.1).mesh()).translated_by(Vec3::new(-0.8, 0., 0.)));
            mesh
        })),
        InstancedMaterial3d(materials3.add(ExtendedInstancedMaterial {
            base: StandardParticle {
                base_color: LinearRgba::new(2., 2., 0., 1.),
                texture: images.add(uv_debug_texture()),
                alpha_mode: AlphaMode::Opaque,
                ..Default::default()
            },
            extension: SpinMat {},
        })),
    ));

    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 10_000_000.,
            range: 100.0,
            shadow_depth_bias: 0.2,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0),
    ));

    // ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0).subdivisions(10))),
        MeshMaterial3d(materials.add(StandardMaterial::from_color(Srgba::GREEN))),
        Transform::from_xyz(0., 0., 0.),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 7., 30.0).looking_at(Vec3::new(0., 0., 0.), Vec3::Y),
    ));
}
