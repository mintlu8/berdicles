//! Utility for implementing particles.

use std::f32::consts::PI;

use bevy::{
    math::{Vec2, Vec3},
    transform::components::Transform,
};

/// Create a pseudo-random `f32` from another `f32`.
pub fn next_seed(seed: f32) -> f32 {
    fastrand::Rng::with_seed(seed.to_bits() as u64).f32()
}
/// Create a random 3d unit vector.
pub fn random_circle(seed: f32) -> Vec2 {
    Vec2::from_angle(seed * (2. * PI))
}

/// Create a random 3d unit vector.
pub fn random_unit(seed: f32) -> Vec3 {
    let theta = seed * 2. * PI;
    let phi = (next_seed(seed) * 2. - 1.).acos();
    let (ps, pc) = phi.sin_cos();
    let (ts, tc) = theta.sin_cos();
    Vec3::new(ps * tc, ps * ts, pc)
}

/// Place [`Transform`] on a curve while facing forward via derivatives.
pub fn transform_from_ddt(mut f: impl FnMut(f32) -> Vec3, lifetime: f32) -> Transform {
    const SMOL_NUM: f32 = 0.001;
    let translation = f(lifetime);
    let next = f(lifetime + SMOL_NUM);
    Transform::from_translation(translation).looking_to(next - translation, Vec3::Y)
}
