use bevy::{
    asset::RenderAssetUsages,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

pub struct FPSPlugin;

impl Plugin for FPSPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin)
            .add_systems(Startup, setup)
            .add_systems(Update, fps);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Text::new("FPS: 60.00"));
}

fn fps(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut Text>) {
    if let Some(value) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed())
    {
        query.single_mut().0 = format!("FPS: {:.2}", value)
    }
}

/// Creates a colorful test pattern
pub fn uv_debug_texture() -> Image {
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

pub trait InspectEntity {
    fn inspect(&mut self) -> &mut Self;
}

impl InspectEntity for EntityCommands<'_> {
    fn inspect(&mut self) -> &mut Self {
        let entity = self.id();
        self.commands().queue(move |w: &mut World| {
            w.inspect_entity(entity)
                .for_each(|x| println!("{}", x.name()));
        });
        self
    }
}
