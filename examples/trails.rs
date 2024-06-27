//! This example demonstrates the built-in 3d shapes in Bevy.
//! The scene includes a patterned texture and a rotation for visualizing the normals and UVs.

use berdicles::{
    trail::{TrailBuffer, TrailMeshBuilder, TrailMeshOf, TrailParticleSystem, TrailedParticle},
    util::transform_from_derivative,
    ExpirationState, Particle, ParticleBuffer, ParticleInstance, ParticlePlugin, ParticleSystem,
    ParticleSystemBundle, RingBuffer, StandardParticle,
};
use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    pbr::{NotShadowCaster, NotShadowReceiver},
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
            trail: Trail::default(),
            trail_meta: 0.,
        }
    }

    fn on_update(&mut self, dt: f32, buffer: &mut ParticleBuffer) {
        buffer.update_detached::<Self::Particle>(dt)
    }

    fn detach_slice(&mut self, detached: std::ops::Range<usize>, buffer: &mut ParticleBuffer) {
        buffer.detach_slice::<Self::Particle>(detached)
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
        .spawn(ParticleSystemBundle {
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
        })
        .id();

    commands.spawn((
        MaterialMeshBundle {
            mesh: meshes.add(Sphere::new(0.1).mesh()),
            material: materials.add(StandardMaterial {
                base_color: Color::srgba(0., 1., 1., 1.),
                base_color_texture: Some(images.add(uv_debug_texture())),
                cull_mode: None,
                double_sided: true,
                unlit: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        NotShadowCaster,
        NotShadowReceiver,
        TrailMeshOf(root),
    ));

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
        transform: Transform::from_xyz(0.0, 9., 30.0).looking_at(Vec3::new(0., 4.0, 0.), Vec3::Y),
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
