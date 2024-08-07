//! This example demonstrates
//! how to reuse a simulation result with different material and mesh.

use berdicles::{
    shader::{PARTICLE_VERTEX_IN, PARTICLE_VERTEX_OUT},
    util::{random_cone, transform_from_derivative},
    ExpirationState, Particle, ParticleInstance, ParticleMaterialPlugin, ParticlePlugin,
    ParticleRef, ParticleRefBundle, ParticleSystem, ParticleSystemBundle, StandardParticle,
};
use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{AsBindGroup, Extent3d, ShaderRef, TextureDimension, TextureFormat},
    },
    window::PresentMode,
};
use std::f32::consts::PI;

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
            ExtendedMaterial<StandardParticle, SpinMat>,
        >::default())
        .add_plugins(|a: &mut App| {
            a.world_mut()
                .resource_mut::<Assets<Shader>>()
                .insert(&SPIN_SHADER, Shader::from_wgsl(SPIN_VERTEX, "spin.wgsl"))
        })
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(ParticlePlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, fps)
        .run();
}

#[derive(Debug, Clone, Copy)]
pub struct MyParticle {
    pub seed: f32,
    pub life_time: f32,
}

impl Particle for MyParticle {
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

impl ParticleSystem for MySpawner {
    type Particle = MyParticle;

    fn capacity(&self) -> usize {
        10000
    }

    fn spawn_step(&mut self, time: f32) -> usize {
        self.0 += time * 10.;
        let result = self.0.floor() as usize;
        self.0 = self.0.fract();
        result
    }

    fn build_particle(&self, seed: f32) -> Self::Particle {
        MyParticle {
            seed,
            life_time: 0.,
        }
    }
}

const SPIN_FN: &str = stringify!(
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
);

pub static SPIN_VERTEX: &str = const_format::concatcp!(
    "#import bevy_pbr::mesh_functions::get_world_from_local\n",
    "#import bevy_pbr::view_transformations::position_world_to_clip,\n",
    PARTICLE_VERTEX_IN,
    PARTICLE_VERTEX_OUT,
    SPIN_FN,
);

pub static SPIN_SHADER: Handle<Shader> = Handle::weak_from_u128(123123412412412412);

#[derive(Debug, Clone, Copy, TypePath, Asset, AsBindGroup)]
struct SpinMat {}

impl MaterialExtension for SpinMat {
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
    mut materials3: ResMut<Assets<ExtendedMaterial<StandardParticle, SpinMat>>>,
) {
    commands.spawn(TextBundle {
        text: Text::from_section("FPS: 60.00", Default::default()),
        ..Default::default()
    });
    let e = commands
        .spawn(ParticleSystemBundle {
            particle_system: ParticleInstance::new(MySpawner(0.)),
            mesh: meshes.add(
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
            material: materials2.add(StandardParticle {
                base_color: LinearRgba::new(2., 2., 2., 1.),
                texture: images.add(uv_debug_texture()),
                alpha_mode: AlphaMode::Opaque,
            }),
            ..Default::default()
        })
        .id();

    commands.spawn(ParticleRefBundle {
        particles: ParticleRef(e),
        mesh: meshes.add(
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
        material: materials2.add(StandardParticle {
            base_color: LinearRgba::new(2., 2., 2., 1.),
            texture: images.add(uv_debug_texture()),
            alpha_mode: AlphaMode::Opaque,
        }),
        ..Default::default()
    });

    commands.spawn(ParticleRefBundle {
        particles: ParticleRef(e),
        mesh: meshes.add({
            let mut mesh =
                Mesh::from(Sphere::new(0.1).mesh()).translated_by(Vec3::new(0.8, 0., 0.));
            mesh.merge(&Mesh::from(Sphere::new(0.1).mesh()).translated_by(Vec3::new(-0.8, 0., 0.)));
            mesh
        }),
        material: materials3.add(ExtendedMaterial {
            base: StandardParticle {
                base_color: LinearRgba::new(2., 2., 0., 1.),
                texture: images.add(uv_debug_texture()),
                alpha_mode: AlphaMode::Opaque,
            },
            extension: SpinMat {},
        }),
        ..Default::default()
    });

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

/// Creates a colorful test pattern
fn uv_debug_texture() -> Image {
    const TEXTURE_SIZE: usize = 8;

    let mut palette: [u8; 32] = [
        255, 102, 159, 255, 255, 159, 102, 255, 236, 255, 102, 255, 121, 255, 102, 255, 102, 255,
        198, 255, 102, 198, 255, 255, 121, 102, 255, 255, 236, 102, 255, 255,
    ];

    let mut texture_data = [0; TEXTURE_SIZE * TEXTURE_SIZE * 4];
    for y in 0..TEXTURE_SIZE {
        let offset = TEXTURE_SIZE * y * 4;
        texture_data[offset..(offset + TEXTURE_SIZE * 4)].copy_from_slice(&palette);
        palette.rotate_right(4);
    }

    Image::new_fill(
        Extent3d {
            width: TEXTURE_SIZE as u32,
            height: TEXTURE_SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn fps(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut Text>) {
    if let Some(value) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed())
    {
        query.single_mut().sections[0].value = format!("FPS: {:.2}", value)
    }
}
