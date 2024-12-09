//! This example demonstrates
//! the basics while serving as a stress test.
mod util;

use berdicles::{
    util::{random_sphere, transform_from_derivative},
    DefaultInstanceBuffer, ExpirationState, InstancedMaterial3d, ParticleSystem, Projectile,
    ProjectileCluster, ProjectilePlugin, StandardParticle,
};
use bevy::{prelude::*, window::PresentMode};
use std::f32::consts::PI;
use util::{uv_debug_texture, FPSPlugin, InspectEntity};

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
        .run();
}

#[derive(Debug, Clone, Copy)]
pub struct MyParticle {
    pub seed: f32,
    pub life_time: f32,
}

impl Projectile for MyParticle {
    type Extracted = DefaultInstanceBuffer;

    fn get_seed(&self) -> f32 {
        self.seed
    }

    fn get_lifetime(&self) -> f32 {
        self.life_time
    }

    fn get_transform(&self) -> Transform {
        let f = |t| random_sphere(self.seed) * t * 2.;
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
    type Projectile = MyParticle;

    fn capacity(&self) -> usize {
        100000
    }

    fn spawn_step(&mut self, time: f32) -> usize {
        self.0 += time * 4000.;
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

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials2: ResMut<Assets<StandardParticle>>,
) {
    commands
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
        .inspect();

    // ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0).subdivisions(10))),
        MeshMaterial3d(Handle::<StandardMaterial>::default()),
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
        Transform::from_xyz(0.0, 7., 30.0).looking_at(Vec3::new(0., 0., 0.), Vec3::Y),
        Camera3d::default(),
    ));
}
