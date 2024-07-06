//! Utility for implementing particles.

use std::f32::consts::PI;

use bevy::{
    math::{Quat, Vec2, Vec3},
    transform::components::Transform,
};

/// Create a [`fastrand::Rng`] from a seed.
pub fn into_rng(seed: f32) -> fastrand::Rng {
    fastrand::Rng::with_seed((seed as f64 * u64::MAX as f64) as u64)
}

/// Create a random 2d unit vector.
pub fn random_circle(seed: f32) -> Vec2 {
    Vec2::from_angle(seed * (2. * PI))
}

/// Create a random 2d vector inside a `r=1` circle.
pub fn random_solid_circle(seed: f32) -> Vec2 {
    let mut rng = into_rng(seed);
    let r = rng.f32().sqrt();
    let (s, c) = (rng.f32() * 2. * PI).sin_cos();
    Vec2::new(r * c, r * s)
}

fn lerp(a: f32, b: f32, fac: f32) -> f32 {
    a * (1.0 - fac) + b * fac
}

/// Create a random 3d unit vector near a direction.
pub fn random_cone(points_to: Vec3, angle: f32, seed: f32) -> Vec3 {
    let mut rng = into_rng(seed);
    let theta = rng.f32() * 2. * PI;
    let angle = angle.cos();
    let phi = (lerp(1.0, angle, rng.f32())).acos();
    let (ps, pc) = phi.sin_cos();
    let (ts, tc) = theta.sin_cos();
    Quat::from_rotation_arc(Vec3::Z, points_to).mul_vec3(Vec3::new(ps * tc, ps * ts, pc))
}

/// Create a random 3d unit vector.
pub fn random_sphere(seed: f32) -> Vec3 {
    let mut rng = into_rng(seed);
    let theta = seed * 2. * PI;
    let phi = (rng.f32() * 2. - 1.).acos();
    let (ps, pc) = phi.sin_cos();
    let (ts, tc) = theta.sin_cos();
    Vec3::new(ps * tc, ps * ts, pc)
}

/// Place [`Transform`] on a curve while facing forward via derivatives.
pub fn transform_from_derivative(mut f: impl FnMut(f32) -> Vec3, lifetime: f32) -> Transform {
    const SMOL_NUM: f32 = 0.001;
    let translation = f(lifetime);
    let next = f(lifetime + SMOL_NUM);
    Transform::from_translation(translation).looking_to(next - translation, Vec3::Y)
}

/// Create a random [`Quat`].
pub fn random_quat(seed: f32) -> Quat {
    let mut rng = into_rng(seed);
    let u1 = rng.f32();
    let u2 = rng.f32();
    let u3 = rng.f32();
    Quat {
        x: (1. - u1).sqrt() * (2. * PI * u2).sin(),
        y: (1. - u1).sqrt() * (2. * PI * u2).cos(),
        z: (u1).sqrt() * (2. * PI * u3).sin(),
        w: (u1).sqrt() * (2. * PI * u3).cos(),
    }
}
