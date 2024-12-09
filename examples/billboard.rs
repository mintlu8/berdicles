//! This example demonstrates how to render mesh that always faces the camera.

use berdicles::{
    util::{random_cone, random_quat},
    ExpirationState, InstancedMaterial3d, Projectile, ProjectileCluster, ProjectilePlugin,
    ProjectileSystem, StandardParticle,
};
use bevy::{prelude::*, window::PresentMode};
use util::{uv_debug_texture, FPSPlugin};
mod util;

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
    fn get_seed(&self) -> f32 {
        self.seed
    }

    fn get_lifetime(&self) -> f32 {
        self.life_time
    }

    fn get_transform(&self) -> Transform {
        let translation =
            random_cone(Vec3::Y, f32::to_radians(30.), self.seed) * self.life_time * 2.;
        let rotation = random_quat(self.seed);
        Transform {
            translation,
            rotation,
            scale: Vec3::ONE,
        }
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

const CAM: Vec3 = Vec3::new(0.0, 7., 30.0);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials2: ResMut<Assets<StandardParticle>>,
) {
    let mesh_handle = meshes.add(Mesh::from(
        Plane3d {
            normal: Dir3::Z,
            half_size: Vec2::ONE,
        }
        .mesh(),
    ));
    commands.spawn((
        ProjectileCluster::new(MySpawner(0.)),
        Mesh3d(mesh_handle.clone()),
        InstancedMaterial3d(materials2.add(StandardParticle {
            base_color: LinearRgba::new(2., 0., 0., 1.),
            texture: images.add(uv_debug_texture()),
            alpha_mode: AlphaMode::Opaque,
            billboard: true,
            ..Default::default()
        })),
        Transform::from_xyz(-4., 0., 0.),
    ));

    commands.spawn((
        ProjectileCluster::new(MySpawner(0.)),
        Mesh3d(mesh_handle.clone()),
        InstancedMaterial3d(materials2.add(StandardParticle {
            base_color: LinearRgba::new(0., 0., 2., 1.),
            texture: images.add(uv_debug_texture()),
            alpha_mode: AlphaMode::Opaque,
            ..Default::default()
        })),
        Transform::from_xyz(4., 0., 0.),
    ));

    // commands.spawn(MaterialMeshBundle {
    //     mesh: mesh_handle.clone(),
    //     material: materials.add(StandardMaterial {
    //         base_color: LinearRgba::new(2., 2., 0., 1.).into(),
    //         base_color_texture: Some(images.add(uv_debug_texture())),
    //         ..Default::default()
    //     }),
    //     transform: Transform::from_xyz(0., 0.5, 0.).looking_at(CAM, Vec3::Y),
    //     ..default()
    // });

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
