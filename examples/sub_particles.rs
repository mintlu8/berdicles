//! This example demonstrates the built-in 3d shapes in Bevy.
//! The scene includes a patterned texture and a rotation for visualizing the normals and UVs.

use berdicle::{
    util::{random_circle, transform_from_ddt},
    ErasedSubParticleSystem, EventParticleSystem, ExpirationState, Particle, ParticleInstance,
    ParticlePlugin, ParticleSystem, ParticleSystemBundle, StandardParticle, SubParticleSystem,
};
use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
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
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(ParticlePlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, fps)
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
        transform_from_ddt(f, self.life_time)
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
        Transform::from_translation(self.origin + Vec3::new(p.x, 1., p.y) * self.life_time * 12.)
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

    fn as_event_particle_system(&mut self) -> Option<&mut dyn berdicle::ErasedEventParticleSystem> {
        Some(self)
    }
}

impl EventParticleSystem for CollisionSpawner {
    fn spawn_event(&mut self, parent: &berdicle::ParticleEvent) -> usize {
        match parent.event {
            berdicle::ParticleEventType::Explode => 12,
            _ => 0,
        }
    }

    fn into_sub_particle(parent: &berdicle::ParticleEvent, seed: f32) -> Self::Particle {
        CollisionParticle {
            origin: parent.position,
            seed,
            life_time: 0.,
        }
    }
}

const SHAPES_X_EXTENT: f32 = 14.0;
const EXTRUSION_X_EXTENT: f32 = 16.0;
const Z_EXTENT: f32 = 5.0;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials2: ResMut<Assets<StandardParticle>>,
) {
    commands.spawn(TextBundle {
        text: Text::from_section("FPS: 60.00", Default::default()),
        ..Default::default()
    });
    let root = commands
        .spawn(
            ParticleSystemBundle {
                particle_system: ParticleInstance::new(MainSpawner(0.)),
                mesh: meshes.add(
                    Mesh::from(
                        Cone {
                            radius: 0.5,
                            height: 0.5,
                        }
                        .mesh(),
                    )
                    .rotated_by(Quat::from_rotation_x(-PI / 2.0)),
                ),
                material: materials2.add(StandardParticle {
                    base_color: LinearRgba::new(2., 2., 2., 1.),
                    texture: images.add(uv_debug_texture()),
                }),
                ..Default::default()
            }
            .with_events(),
        )
        .id();

    commands.spawn(
        ParticleSystemBundle {
            particle_system: ParticleInstance::new(ChildSpawner(0.)),
            mesh: meshes.add(Sphere::new(0.1).mesh()),
            material: materials2.add(StandardParticle {
                base_color: LinearRgba::new(0., 2., 2., 1.),
                texture: images.add(uv_debug_texture()),
            }),
            ..Default::default()
        }
        .parented(root),
    );

    commands.spawn(
        ParticleSystemBundle {
            particle_system: ParticleInstance::new(CollisionSpawner),
            mesh: meshes.add(Cuboid::new(0.2, 0.2, 0.2).mesh()),
            material: materials2.add(StandardParticle {
                base_color: LinearRgba::new(2., 0., 0., 1.),
                texture: images.add(uv_debug_texture()),
            }),
            ..Default::default()
        }
        .parented(root),
    );

    // ground plane
    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::default().mesh().size(50.0, 50.0).subdivisions(10)),
        material: materials.add(StandardMaterial::from_color(Srgba::GREEN)),
        transform: Transform::from_xyz(0., 0., 0.),
        ..default()
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
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 7., 50.0).looking_at(Vec3::new(0., 0., 0.), Vec3::Y),
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
