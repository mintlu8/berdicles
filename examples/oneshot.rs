//! This example demonstrates the built-in 3d shapes in Bevy.
//! The scene includes a patterned texture and a rotation for visualizing the normals and UVs.

use berdicles::{
    util::random_cone, ExpirationState, OneShotParticleInstance, Particle, ParticlePlugin,
    ParticleSystem, StandardParticle,
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

#[derive(Debug, Clone, Copy)]
pub struct MyParticle {
    pub seed: f32,
}

impl Particle for MyParticle {
    fn get_seed(&self) -> f32 {
        self.seed
    }

    fn get_lifetime(&self) -> f32 {
        0.
    }

    fn get_transform(&self) -> Transform {
        Transform::from_translation(random_cone(Vec3::Y, f32::to_radians(30.), self.seed) * 2.)
    }

    fn update(&mut self, _: f32) {}

    fn expiration_state(&self) -> ExpirationState {
        ExpirationState::None
    }
}

pub struct MySpawner;

impl ParticleSystem for MySpawner {
    type Particle = MyParticle;

    fn capacity(&self) -> usize {
        40
    }

    fn spawn_step(&mut self, _: f32) -> usize {
        40
    }

    fn build_particle(&self, seed: f32) -> Self::Particle {
        MyParticle { seed }
    }
}

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
    commands.spawn((
        MaterialMeshBundle {
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
        },
        OneShotParticleInstance::new(MySpawner),
    ));

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
