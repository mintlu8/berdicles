use std::ops::Range;

use bevy::{color::{Gray, Srgba}, math::Vec3};

use crate::Particle;


pub enum TrailSpan {
    None,
    LifeTime(Range<f32>),
    Fac(Range<f32>),
}

pub trait ParticleTrail {
    /// If set to false, don't generate normals.
    const NORMALS: bool = true;
    /// If set to false, `color()` has no effect.
    const VERTEX_COLOR: bool = true;
    type Particle: Particle;

    /// Return a span of the curve that is converted to mesh, in lifetime.
    /// 
    /// Arguments `fac` are relative to this span.
    fn span(&self, particle: &Self::Particle) -> Range<f32> {
        0.0..particle.get_lifetime()
    }

    /// Returns the amount of segments in the mesh.
    /// 
    /// Returning 0 will discard the particle.
    #[allow(unused)]
    fn segments(&self, particle: &Self::Particle) -> usize;

    /// Returns the range of uv in the x axis.
    /// Default is `0.0..1.0`, relative to `span()`'s fac.
    /// 
    /// (y axis is always `0.0..1.0`)
    #[allow(unused)]
    fn uv_x(&self, particle: &Self::Particle, fac: f32) -> f32 {
        fac
    }

    /// Modify a sampled position on the curve.
    #[allow(unused)]
    fn position(&self, particle: &Self::Particle, sampled: Vec3, fac: f32) -> Vec3 {
        sampled
    }

    /// Returns the width of the curve.
    fn width(&self, particle: &Self::Particle, fac: f32) -> f32;

    /// Set vertex color on the generated mesh.
    #[allow(unused)]
    fn color(&self, particle: &Self::Particle, fac: f32) -> Srgba {
        Srgba::WHITE
    }
}

