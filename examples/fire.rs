//! Recreation of tutorial https://www.youtube.com/watch?v=OnxiEY3Khow by Gabriel Aguiar Prod.
mod util;

use std::f32::consts::PI;

use berdicles::{
    util::{into_rng, map_range, random_cone, random_sphere, spawn_rate},
    DefaultInstanceBuffer, ExpirationState, ExtendedInstancedMaterial, InstancedMaterial3d,
    InstancedMaterialExtension, InstancedMaterialPlugin, Projectile, ProjectileCluster,
    ProjectilePlugin, ProjectileSystem, StandardParticle,
};
use bevy::{
    core_pipeline::bloom::Bloom,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
    window::PresentMode,
};
use util::FPSPlugin;

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
        .add_plugins(FPSPlugin)
        .add_plugins(ProjectilePlugin)
        .add_systems(Startup, setup)
        .add_plugins(InstancedMaterialPlugin::<
            ExtendedInstancedMaterial<StandardParticle, ErosionExt>,
        >::default())
        .add_plugins(|app: &mut App| {
            app.world_mut().resource_mut::<Assets<Shader>>().insert(
                SHADER_HANDLE.id(),
                Shader::from_wgsl(SHADER, "erosion.wgsl"),
            );
        })
        //.add_systems(Update, spin)
        .run();
}

#[derive(Debug, Clone, TypePath, Asset, AsBindGroup)]
pub struct ErosionExt {
    #[texture(50)]
    #[sampler(51)]
    pub voronoi: Handle<Image>,
}

static SHADER: &str = r#"
    #import berdicle::{VertexOutput, color, texture, texture_sampler};

    @group(2) @binding(50) var erosion: texture_2d<f32>;
    @group(2) @binding(51) var erosion_sampler: sampler;

    @fragment
    fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
        let sampled = textureSample(texture, texture_sampler, input.uv);
        let erosion = textureSample(erosion, erosion_sampler, input.uv).r;
        let t = input.fac * 6.0 + 1.0;
        let rgb = input.color * color * sampled;
        let a = pow(pow(erosion, 0.5) * rgb.a, t);
        return vec4(rgb.xyz, a);
    }
"#;

static SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(2132121512512);

impl InstancedMaterialExtension for ErosionExt {
    type InstanceBuffer = DefaultInstanceBuffer;

    fn fragment_shader() -> bevy::render::render_resource::ShaderRef {
        ShaderRef::Handle(SHADER_HANDLE.clone())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MyParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: f32,
    pub angular_velocity: f32,
    pub life_time: f32,
}

impl Projectile for MyParticle {
    fn get_transform(&self) -> Transform {
        let v = map_range(self.life_time, 0.0..4.0, 1.0..0.6);
        Transform {
            translation: self.position,
            rotation: Quat::from_rotation_z(self.rotation),
            scale: Vec3::splat(v),
        }
    }

    fn get_fac(&self) -> f32 {
        (self.life_time / 1.7).min(1.)
    }

    fn get_color(&self) -> Srgba {
        map_range(
            self.life_time,
            0.0..1.7,
            Srgba::WHITE..Srgba::new(1., 1., 1., 0.),
        )
    }

    fn update(&mut self, dt: f32) {
        self.life_time += dt;
        self.rotation += self.angular_velocity * dt;
        self.position += self.velocity * dt;
    }

    fn expiration_state(&self) -> ExpirationState {
        if self.life_time > 1.7 {
            ExpirationState::Fizzle
        } else {
            ExpirationState::None
        }
    }
}

pub struct MySpawner {
    pub spawn_rate: f32,
    pub speed_range: (f32, f32),
    pub spawn_meta: f32,
    pub position: Vec3,
}

impl ProjectileSystem for MySpawner {
    type Projectile = MyParticle;

    const WORLD_SPACE: bool = true;

    fn capacity(&self) -> usize {
        100
    }

    fn spawn_step(&mut self, time: f32) -> usize {
        spawn_rate(&mut self.spawn_meta, self.spawn_rate, time)
    }

    fn build_particle(&self, seed: f32) -> Self::Projectile {
        let mut rng = into_rng(seed);
        MyParticle {
            position: self.position + random_sphere(rng.f32()) * 0.1,
            //velocity: random_sphere(rng.f32()) * 0.4,
            velocity: random_cone(Vec3::Y, 40.0f32.to_radians(), rng.f32())
                * (rng.f32() * (self.speed_range.1 - self.speed_range.0) + self.speed_range.0),
            rotation: rng.f32() * 2. * PI,
            angular_velocity: (rng.f32() / 4. + 0.25) * if rng.bool() { 1.0 } else { -1.0 },
            life_time: 0.,
        }
    }

    fn update_position(&mut self, transform: &GlobalTransform) {
        self.position = transform.translation()
    }
}

#[derive(Debug, Component)]
pub struct Root;

fn setup(mut commands: Commands, server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>) {
    let plane = meshes.add(Mesh::from(Plane3d {
        normal: Dir3::Z,
        half_size: Vec2::splat(0.5),
    }));
    let plane2 = meshes.add(Mesh::from(Plane3d {
        normal: Dir3::Z,
        half_size: Vec2::splat(0.6),
    }));

    commands
        .spawn((Transform::default(), Visibility::Inherited, Root))
        .with_children(|commands| {
            commands.spawn((
                ProjectileCluster::new(MySpawner {
                    spawn_rate: 20.,
                    speed_range: (1.0, 1.4),
                    spawn_meta: 0.,
                    position: Vec3::ZERO,
                }),
                Mesh3d(plane.clone()),
                InstancedMaterial3d(server.add(ExtendedInstancedMaterial {
                    base: StandardParticle {
                        base_color: LinearRgba::new(50., 3.5, 1.0, 1.),
                        texture: server.load("flames.png"),
                        alpha_mode: AlphaMode::Blend,
                        billboard: true,
                        ..Default::default()
                    },
                    extension: ErosionExt {
                        voronoi: server.load("voronoi.png"),
                    },
                })),
            ));

            commands.spawn((
                ProjectileCluster::new(MySpawner {
                    spawn_rate: 60.,
                    spawn_meta: 0.,
                    speed_range: (1.0, 2.0),
                    position: Vec3::ZERO,
                }),
                Mesh3d(plane2.clone()),
                InstancedMaterial3d(server.add(ExtendedInstancedMaterial {
                    base: StandardParticle {
                        base_color: LinearRgba::new(0., 0., 0., 1.),
                        texture: server.load("flames.png"),
                        alpha_mode: AlphaMode::Blend,
                        billboard: true,
                        ..Default::default()
                    },
                    extension: ErosionExt {
                        voronoi: server.load("voronoi.png"),
                    },
                })),
            ));
        });

    // ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0).subdivisions(10))),
        MeshMaterial3d(server.add(StandardMaterial::from_color(Srgba::GREEN))),
        Transform::from_xyz(0., -0.5, 0.),
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

    commands.spawn((
        Transform::from_xyz(0.0, 4., 15.0).looking_at(Vec3::new(0., 0., 0.), Vec3::Y),
        Camera3d::default(),
        Camera {
            hdr: true,
            ..Default::default()
        },
        Bloom::NATURAL,
    ));
}

fn spin(time: Res<Time<Virtual>>, mut root: Query<&mut Transform, With<Root>>) {
    for mut transform in &mut root {
        transform.translation.x = (time.elapsed_secs() / 20.).sin() * 4.;
        transform.translation.z = (time.elapsed_secs() / 20.).cos() * 4.;
    }
}
