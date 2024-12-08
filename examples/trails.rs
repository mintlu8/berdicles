//! This example demonstrates how to render trails behind particles.

use berdicles::{
    trail::{TrailBuffer, TrailMeshBuilder, TrailMeshOf, TrailParticleSystem, TrailedParticle},
    util::transform_from_derivative,
    ExpirationState, Particle, ParticleSystem, ProjectileCluster, ProjectileMat, ProjectilePlugin,
    RingBuffer, StandardProjectile,
};
use bevy::{
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    window::PresentMode,
};
use std::f32::consts::PI;
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

/// A marker component for our shapes so we can query them separately from the ground plane
#[derive(Component)]
struct Shape;

#[derive(Debug, Clone, Copy)]
pub struct MainParticle {
    pub seed: f32,
    pub life_time: f32,
    pub meta: f32,
    pub trail_meta: f32,
    pub trail: Trail,
}

impl Particle for MainParticle {
    fn get_seed(&self) -> f32 {
        self.seed
    }

    fn get_lifetime(&self) -> f32 {
        self.life_time
    }

    fn get_transform(&self) -> Transform {
        let f = |t| {
            let z = t * 8. - t * t;
            let xy: Vec2 = Vec2::from_angle(self.seed * PI * 4.) * t;
            Vec3::new(xy.x, z, xy.y)
        };
        transform_from_derivative(f, self.life_time)
    }

    fn get_position(&self) -> Vec3 {
        let t = self.life_time;
        let z = t * 8. - t * t;
        let xy: Vec2 = Vec2::from_angle(self.seed * PI * 4.) * t;
        Vec3::new(xy.x, z, xy.y)
    }

    fn get_color(&self) -> Srgba {
        Srgba::WHITE
    }

    fn update(&mut self, dt: f32) {
        self.life_time += dt;
        self.trail_meta += dt * 16.;
        self.trail.update(dt);
        if self.trail_meta >= 1. {
            self.trail_meta = self.trail_meta.fract();
            self.trail.0.push(TrailVertex {
                position: self.get_position(),
                tangent: Vec3::X,
                width: 0.2,
            })
        }
    }

    fn expiration_state(&self) -> ExpirationState {
        if self.life_time > 8.0 {
            ExpirationState::Fizzle
        } else {
            ExpirationState::None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrailVertex {
    pub position: Vec3,
    pub tangent: Vec3,
    pub width: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Trail(RingBuffer<TrailVertex, 16>);

impl TrailBuffer for Trail {
    fn update(&mut self, dt: f32) {
        self.0.retain_mut_ordered(|item| {
            item.width -= dt * 0.3;
            item.width > 0.
        });
    }

    fn expired(&self) -> bool {
        self.0.is_empty()
    }

    fn build_trail(&self, mesh: &mut Mesh) {
        TrailMeshBuilder::new(mesh).build_plane(
            self.0.iter().map(|x| (x.position, x.tangent, x.width)),
            0.0..1.0,
        )
    }
}

pub struct MainSpawner(f32);

impl ParticleSystem for MainSpawner {
    type Particle = MainParticle;

    fn capacity(&self) -> usize {
        200
    }

    fn spawn_step(&mut self, time: f32) -> usize {
        self.0 += time * 4.;
        let result = self.0.floor() as usize;
        self.0 = self.0.fract();
        result
    }

    fn build_particle(&self, seed: f32) -> Self::Particle {
        MainParticle {
            seed,
            life_time: 0.,
            meta: 0.,
            trail: Trail::default(),
            trail_meta: 0.,
        }
    }

    fn as_trail_particle_system(&mut self) -> Option<&mut dyn TrailParticleSystem> {
        Some(self)
    }
}

impl TrailedParticle for MainParticle {
    type TrailBuffer = Trail;

    fn as_trail_buffer(&self) -> Self::TrailBuffer {
        self.trail
    }

    fn as_trail_buffer_mut(&mut self) -> &mut Self::TrailBuffer {
        &mut self.trail
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials2: ResMut<Assets<StandardProjectile>>,
) {
    let root = commands
        .spawn((
            ProjectileCluster::new(MainSpawner(0.)),
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
            ProjectileMat(materials2.add(StandardProjectile {
                base_color: LinearRgba::new(2., 2., 2., 1.),
                texture: images.add(uv_debug_texture()),
                alpha_mode: AlphaMode::Opaque,
                ..Default::default()
            })),
        ))
        .id();

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.1).mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0., 1., 1., 1.),
            base_color_texture: Some(images.add(uv_debug_texture())),
            cull_mode: None,
            double_sided: true,
            unlit: true,
            ..Default::default()
        })),
        NotShadowCaster,
        NotShadowReceiver,
        TrailMeshOf(root),
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
