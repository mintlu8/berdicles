use bevy::{
    math::{Mat3, Quat, Vec3},
    prelude::{Component, Query, With, Without},
    render::camera::Camera,
    transform::components::GlobalTransform,
};

/// Add to a `ParticleSystemBundle` to make it always face the camera.
///
/// You may want to mark your camera as [`BillboardCamera`] if you have multiple.
#[derive(Debug, Component, Default)]
pub struct BillboardParticle(pub(crate) Quat);

impl BillboardParticle {
    pub const fn new() -> Self {
        BillboardParticle(Quat::IDENTITY)
    }
}

/// Marker component that selects [`Camera`] for billboard rendering. 
/// 
/// Optional if only one camera existså.
#[derive(Debug, Component)]
pub struct BillboardCamera;

/// System for calculating billboard orientation.
pub fn billboard_system(
    untagged: Query<&GlobalTransform, (With<Camera>, Without<BillboardCamera>)>,
    tagged: Query<&GlobalTransform, (With<Camera>, With<BillboardCamera>)>,
    mut billboard: Query<&mut BillboardParticle>,
) {
    let Ok(cam) = tagged
        .get_single()
        .map(|x| x.to_scale_rotation_translation().1)
        .or_else(|_| {
            untagged
                .get_single()
                .map(|x| x.to_scale_rotation_translation().1)
        })
    else {
        return;
    };

    let quat = billboard_quaternion(cam);
    for mut item in billboard.iter_mut() {
        item.0 = quat;
    }
}

fn billboard_quaternion(camera_quaternion: Quat) -> Quat {
    let camera_inverse = Quat::conjugate(camera_quaternion);

    let forward_vector = Vec3::new(0.0, 0.0, -1.0);
    let rotated_forward = camera_inverse * forward_vector;

    let up = Vec3::new(0.0, 1.0, 0.0); // World up vector.
    let right = up.cross(rotated_forward).normalize(); // Right vector.
    let up_new = rotated_forward.cross(right); // Correct up vector.

    let rotation_matrix = Mat3::from_cols(right, up_new, rotated_forward);
    Quat::from_mat3(&rotation_matrix)
}
