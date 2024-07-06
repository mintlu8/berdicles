//! This example demonstrates the built-in 3d shapes in Bevy.
//! The scene includes a patterned texture and a rotation for visualizing the normals and UVs.

use std::f32::consts::PI;

use berdicles::{
    shader::{PARTICLE_VERTEX_IN, PARTICLE_VERTEX_OUT},
    util::into_rng,
    ExpirationState, OneShotParticleInstance, Particle, ParticleMaterialPlugin, ParticlePlugin,
    ParticleSystem, StandardParticle,
};
use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
    window::PresentMode,
};
use noise::{NoiseFn, Perlin};

const GRASS_FN: &str = stringify!(
    @group(2) @binding(100) var<uniform> wind: vec2<f32>;
    @vertex
    fn vertex(vertex: Vertex) -> VertexOutput {
        var out: VertexOutput;
        var pos = vertex.position;
        var x = dot(vec4(pos, 1.0), vertex.transform_x);
        var y = dot(vec4(pos, 1.0), vertex.transform_y);
        var z = dot(vec4(pos, 1.0), vertex.transform_z);
        x += wind.x * y * y;
        z += wind.y * y * y;
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
);

pub static GRASS_VERTEX: &str = const_format::concatcp!(
    "#import bevy_pbr::mesh_functions::get_world_from_local\n",
    "#import bevy_pbr::view_transformations::position_world_to_clip,\n",
    PARTICLE_VERTEX_IN,
    PARTICLE_VERTEX_OUT,
    GRASS_FN,
);

pub static GRASS_SHADER: Handle<Shader> = Handle::weak_from_u128(12313213142414156);

#[derive(Debug, Clone, Copy, TypePath, Asset, AsBindGroup)]
struct GrassMat {
    #[uniform(100)]
    pub wind: Vec2,
}

impl MaterialExtension for GrassMat {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Handle(GRASS_SHADER.clone())
    }
}

#[derive(Debug, Resource)]
pub struct Noises {
    pub x: Perlin,
    pub y: Perlin,
}

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
        .add_plugins(ParticleMaterialPlugin::<
            ExtendedMaterial<StandardParticle, GrassMat>,
        >::new(None))
        .add_plugins(|a: &mut App| {
            a.world_mut()
                .resource_mut::<Assets<Shader>>()
                .insert(&GRASS_SHADER, Shader::from_wgsl(GRASS_VERTEX, "grass.wgsl"))
        })
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(ParticlePlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        .add_systems(Update, fps)
        .insert_resource(Noises {
            x: Perlin::new(131412412),
            y: Perlin::new(677543412),
        })
        .run();
}

#[derive(Debug, Clone, Copy)]
pub struct MyParticle {
    pub seed: f32,
}

impl Particle for MyParticle {
    fn get_seed(&self) -> f32 {
        self.seed
    }

    fn get_lifetime(&self) -> f32 {
        0.
    }

    fn get_transform(&self) -> Transform {
        let mut seed = into_rng(self.seed);
        Transform::from_translation(Vec3::new(
            seed.f32() * 50. - 25.,
            0.,
            seed.f32() * 50. - 25.,
        ))
        .with_rotation(Quat::from_rotation_y(seed.f32() * PI * 2.))
    }

    fn update(&mut self, _: f32) {}

    fn expiration_state(&self) -> ExpirationState {
        ExpirationState::None
    }
}

pub struct MySpawner;

impl ParticleSystem for MySpawner {
    type Particle = MyParticle;

    /// Doesn't matter.
    fn capacity(&self) -> usize {
        0
    }

    fn spawn_step(&mut self, _: f32) -> usize {
        50000
    }

    fn build_particle(&self, seed: f32) -> Self::Particle {
        MyParticle { seed }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ExtendedMaterial<StandardParticle, GrassMat>>>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(TextBundle {
        text: Text::from_section("FPS: 60.00", Default::default()),
        ..Default::default()
    });

    for _ in 0..10 {
        commands.spawn((
            MaterialMeshBundle {
                mesh: meshes.add(Mesh::from(
                    Plane3d::new(Vec3::Z, Vec2::splat(0.4))
                        .mesh()
                        .subdivisions(1),
                )),
                material: mats.add(ExtendedMaterial {
                    base: StandardParticle {
                        base_color: LinearRgba::WHITE,
                        texture: server.load("grass.png"),
                        alpha_mode: AlphaMode::Blend,
                    },
                    extension: GrassMat {
                        wind: Vec2::new(1., 1.),
                    },
                }),
                ..Default::default()
            },
            OneShotParticleInstance::new(MySpawner),
        ));
    }

    commands.spawn(PointLightBundle {
        point_light: PointLight {
            shadows_enabled: true,
            intensity: 10_000_000.,
            range: 100.0,
            shadow_depth_bias: 0.2,
            ..default()
        },
        transform: Transform::from_xyz(8.0, 16.0, 8.0),
        ..default()
    });

    // ground plane
    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::default().mesh().size(50.0, 50.0).subdivisions(10)),
        material: materials.add(StandardMaterial::from_color(Srgba::GREEN)),
        transform: Transform::from_xyz(0., 0., 0.),
        ..default()
    });

    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 7., 30.0).looking_at(Vec3::new(0., 0., 0.), Vec3::Y),
        ..default()
    });
}

fn update(
    noise: Res<Noises>,
    time: Res<Time>,
    mut mats: ResMut<Assets<ExtendedMaterial<StandardParticle, GrassMat>>>,
) {
    for mat in mats.iter_mut() {
        mat.1.extension.wind = Vec2::new(
            noise.x.get([time.elapsed_seconds_f64()]) as f32,
            noise.y.get([time.elapsed_seconds_f64()]) as f32,
        )
    }
}

fn fps(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut Text>) {
    if let Some(value) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed())
    {
        query.single_mut().sections[0].value = format!("FPS: {:.2}", value)
    }
}
