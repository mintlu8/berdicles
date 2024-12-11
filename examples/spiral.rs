//! This example demonstrates how to render trails behind particles.

use berdicles::{
    templates::{ExpDecayTrail, WidthCurve},
    trail::{TrailMaterial, TrailMeshOf},
    util::transform_from_derivative,
    ExpirationState, InstancedMaterial3d, Projectile, ProjectileCluster, ProjectilePlugin,
    ProjectileSystem, StandardParticle,
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
    pub trail: ExpDecayTrail<16>,
}

impl Projectile for MainParticle {
    fn get_transform(&self) -> Transform {
        let f = |t: f32| {
            let front = Vec2::from_angle(self.seed * PI * 4.);
            let front = Vec3::new(front.x, 0., front.y);
            let tangent = front.cross(Vec3::Y);
            let angle = t * 12.;
            front * t * 10. + tangent * angle.cos() + Vec3::Y * angle.sin()
        };
        transform_from_derivative(f, self.life_time)
    }

    fn get_color(&self) -> Srgba {
        Srgba::WHITE
    }

    fn update(&mut self, dt: f32) {
        if !self.is_expired() {
            self.life_time += dt;
        }
        self.trail.update(dt);
        self.trail.set_first(self.get_position())
    }

    fn trail(&self) -> &[(Vec3, f32)] {
        &self.trail.buffer
    }

    fn expiration_state(&self) -> ExpirationState {
        if self.life_time > 8.0 {
            ExpirationState::FadeOut
        } else {
            ExpirationState::None
        }
    }

    fn should_despawn(&self) -> bool {
        self.is_expired() && self.trail.is_expired()
    }
}

pub struct MainSpawner(f32);

impl ProjectileSystem for MainSpawner {
    type Projectile = MainParticle;
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

    fn build_particle(&self, seed: f32) -> Self::Projectile {
        MainParticle {
            seed,
            life_time: 0.,
            trail: ExpDecayTrail {
                width_curve: WidthCurve::Fac(|x| (1. - x * x / 2.) * 0.25),
                eps: 0.5,
                ..Default::default()
            },
        }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials2: ResMut<Assets<StandardParticle>>,
    mut materials3: ResMut<Assets<TrailMaterial>>,
) {
    let mut mesh = Mesh::from(Sphere { radius: 0.2 }.mesh());
    let _ = mesh.generate_tangents();
    let root = commands
        .spawn((
            ProjectileCluster::new(MainSpawner(0.)),
            Mesh3d(meshes.add(mesh)),
            InstancedMaterial3d(materials2.add(StandardParticle {
                base_color: LinearRgba::new(2., 2., 2., 1.),
                texture: images.add(uv_debug_texture()),
                alpha_mode: AlphaMode::Opaque,
                ..Default::default()
            })),
        ))
        .id();

    commands.spawn((
        MeshMaterial3d(materials3.add(TrailMaterial {
            base: StandardMaterial {
                base_color: Color::srgba(0., 1., 1., 1.),
                base_color_texture: Some(server.load("lightning.png")),
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                double_sided: true,
                unlit: true,
                ..Default::default()
            },
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
        Mesh3d(meshes.add(Plane3d::default().mesh().size(100.0, 100.0).subdivisions(5))),
        MeshMaterial3d(materials.add(StandardMaterial::from_color(Srgba::GREEN))),
        Transform::from_xyz(0., -10., 0.),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(7.0, 30., 7.0).looking_at(Vec3::new(0., 0., 0.), Vec3::Y),
    ));
}
