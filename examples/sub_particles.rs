//! This example demonstrates how to use parent particle systems's particles as spawners.
mod util;
use berdicles::{
    util::{random_circle, transform_from_derivative},
    ErasedEventParticleSystem, ErasedSubParticleSystem, EventParticleSystem, ExpirationState,
    Particle, ParticleEvent, ParticleEventBuffer, ParticleEventType, ParticleSystem,
    ProjectileCluster, ProjectileMat, ProjectileParent, ProjectilePlugin, StandardProjectile,
    SubParticleSystem,
};
use bevy::{prelude::*, window::PresentMode};
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

    fn get_color(&self) -> Srgba {
        Srgba::WHITE
    }

    fn update(&mut self, dt: f32) {
        self.life_time += dt;
    }

    fn expiration_state(&self) -> ExpirationState {
        if self.life_time > 8.0 {
            ExpirationState::Explode
        } else {
            ExpirationState::None
        }
    }
}

pub struct MainSpawner(f32);

impl ParticleSystem for MainSpawner {
    type Particle = MainParticle;

    fn capacity(&self) -> usize {
        60
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
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrailParticle {
    pub origin: Transform,
    pub seed: f32,
    pub life_time: f32,
}

impl Particle for TrailParticle {
    fn get_seed(&self) -> f32 {
        self.seed
    }

    fn get_lifetime(&self) -> f32 {
        self.life_time
    }

    fn get_transform(&self) -> Transform {
        let point = self
            .origin
            .transform_point(random_circle(self.seed).extend(2.) * self.life_time);
        self.origin.with_translation(point)
    }

    fn get_color(&self) -> Srgba {
        Srgba::WHITE
    }

    fn update(&mut self, dt: f32) {
        self.life_time += dt;
    }

    fn expiration_state(&self) -> ExpirationState {
        if self.life_time > 1.0 {
            ExpirationState::Fizzle
        } else {
            ExpirationState::None
        }
    }
}

pub struct ChildSpawner(f32);

impl ParticleSystem for ChildSpawner {
    type Particle = TrailParticle;

    fn capacity(&self) -> usize {
        100000
    }

    fn spawn_step(&mut self, _: f32) -> usize {
        0
    }

    fn build_particle(&self, _: f32) -> Self::Particle {
        unreachable!()
    }

    fn as_sub_particle_system(&mut self) -> Option<&mut dyn ErasedSubParticleSystem> {
        Some(self)
    }
}

impl SubParticleSystem for ChildSpawner {
    type Parent = MainParticle;

    fn spawn_step_sub(&mut self, parent: &mut Self::Parent, dt: f32) -> usize {
        parent.meta += dt * 100.;
        let result = parent.meta.floor() as usize;
        parent.meta = parent.meta.fract();
        result
    }

    fn into_sub_particle(parent: &Self::Parent, seed: f32) -> Self::Particle {
        TrailParticle {
            origin: parent
                .get_transform()
                .looking_to(parent.get_tangent(), Vec3::Y),
            seed,
            life_time: 0.,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CollisionParticle {
    pub origin: Vec3,
    pub seed: f32,
    pub life_time: f32,
}

impl Particle for CollisionParticle {
    fn get_seed(&self) -> f32 {
        self.seed
    }

    fn get_lifetime(&self) -> f32 {
        self.life_time
    }

    fn get_transform(&self) -> Transform {
        let p = random_circle(self.seed);
        let z = (self.life_time - self.life_time * self.life_time) * 4.;
        Transform::from_translation(
            self.origin + Vec3::new(p.x, 0., p.y) * self.life_time * 4. + Vec3::new(0., z, 0.),
        )
    }

    fn update(&mut self, dt: f32) {
        self.life_time += dt;
    }

    fn expiration_state(&self) -> ExpirationState {
        if self.life_time > 1.0 {
            ExpirationState::Explode
        } else {
            ExpirationState::None
        }
    }
}

pub struct CollisionSpawner;

impl ParticleSystem for CollisionSpawner {
    type Particle = CollisionParticle;

    fn capacity(&self) -> usize {
        100000
    }

    fn spawn_step(&mut self, _: f32) -> usize {
        0
    }

    fn build_particle(&self, _: f32) -> Self::Particle {
        unreachable!()
    }

    fn as_event_particle_system(&mut self) -> Option<&mut dyn ErasedEventParticleSystem> {
        Some(self)
    }
}

impl EventParticleSystem for CollisionSpawner {
    fn spawn_on_event(&mut self, parent: &ParticleEvent) -> usize {
        match parent.event {
            ParticleEventType::Explode => 12,
            _ => 0,
        }
    }

    fn into_sub_particle(parent: &ParticleEvent, seed: f32) -> Self::Particle {
        CollisionParticle {
            origin: parent.position,
            seed,
            life_time: 0.,
        }
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
            ParticleEventBuffer::default(),
        ))
        .id();

    commands.spawn((
        ProjectileCluster::new(ChildSpawner(0.)),
        Mesh3d(meshes.add(Sphere::new(0.1).mesh())),
        ProjectileMat(materials2.add(StandardProjectile {
            base_color: LinearRgba::new(0., 2., 2., 1.),
            texture: images.add(uv_debug_texture()),
            alpha_mode: AlphaMode::Opaque,
            ..Default::default()
        })),
        ProjectileParent(root),
    ));

    commands.spawn((
        ProjectileCluster::new(CollisionSpawner),
        Mesh3d(meshes.add(Cuboid::new(0.2, 0.2, 0.2).mesh())),
        ProjectileMat(materials2.add(StandardProjectile {
            base_color: LinearRgba::new(2., 0., 0., 1.),
            texture: images.add(uv_debug_texture()),
            alpha_mode: AlphaMode::Opaque,
            ..Default::default()
        })),
        ProjectileParent(root),
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
