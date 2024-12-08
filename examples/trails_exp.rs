//! This example demonstrates how to render trails behind particles.

use berdicles::{
    templates::{CameraDirection, ExpDecayTrail, WidthCurve},
    trail::{TrailBuffer, TrailMeshOf, TrailParticleSystem, TrailedParticle},
    util::transform_from_derivative,
    ExpirationState, Particle, ParticleBuffer, ParticleSystem, ProjectileCluster, ProjectileMat,
    ProjectilePlugin, StandardProjectile,
};
use bevy::{
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    window::PresentMode,
};
use std::{any::Any, f32::consts::PI};
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
        .add_systems(Update, particles)
        .add_systems(Update, spin.before(particles))
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
    pub trail: ExpDecayTrail<16>,
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
        self.trail.update(dt);
        self.trail.set_first(self.get_position())
    }

    fn expiration_state(&self) -> ExpirationState {
        if self.life_time > 8.0 {
            ExpirationState::Fizzle
        } else {
            ExpirationState::None
        }
    }
}

pub struct MainSpawner(f32);

impl ParticleSystem for MainSpawner {
    type Particle = MainParticle;
    //const STRATEGY: ParticleBufferStrategy = ParticleBufferStrategy::RingBuffer;

    fn capacity(&self) -> usize {
        100
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
            trail: ExpDecayTrail {
                width_curve: WidthCurve::Fac(|x| (1. - x * x / 2.) * 0.25),
                ..Default::default()
            },
        }
    }

    fn as_trail_particle_system(&mut self) -> Option<&mut dyn TrailParticleSystem> {
        Some(self)
    }

    fn apply_meta(&mut self, command: &dyn Any, buffer: &mut ParticleBuffer) {
        if !buffer.is_uninit() {
            if let Some(cam) = command.downcast_ref::<CameraDirection>() {
                for particle in buffer.get_mut::<MainParticle>() {
                    particle.trail.camera = *cam
                }
                if let Some(detached) = buffer.detached_mut::<ExpDecayTrail<16>>() {
                    for item in detached {
                        item.camera = *cam
                    }
                }
            }
        }
    }
}

impl TrailedParticle for MainParticle {
    type TrailBuffer = ExpDecayTrail<16>;

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
fn particles(
    mut query: Query<(&mut ProjectileCluster, &mut ParticleBuffer)>,
    camera: Query<(&Projection, &GlobalTransform)>,
) {
    let Ok((camera, transform)) = camera.get_single() else {
        return;
    };
    let cam = match camera {
        Projection::Perspective(_) => CameraDirection::Perspective {
            position: transform.translation(),
        },
        Projection::Orthographic(_) => CameraDirection::Orthographic {
            direction: transform.forward().into(),
        },
    };
    for (mut particle, mut buffer) in query.iter_mut() {
        particle.apply_meta(&cam, &mut buffer)
    }
}

fn spin(time: Res<Time<Virtual>>, mut query: Query<&mut Transform, With<Camera>>) {
    if let Ok(mut transform) = query.get_single_mut() {
        transform.rotate_around(Vec3::ZERO, Quat::from_rotation_y(time.delta_secs() / 4.))
    }
}
