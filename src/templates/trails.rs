use std::ops::{Add, Mul, Sub};

use bevy::{math::Vec3, prelude::Mesh};

use crate::trail::{TrailBuffer, TrailMeshBuilder};

#[derive(Debug, Clone, Copy)]
pub enum CameraDirection {
    Orthographic { direction: Vec3 },
    Perspective { position: Vec3 },
}

#[derive(Debug, Clone, Copy)]
pub enum WidthCurve {
    Fac(fn(f32) -> f32),
    Distance { max: f32, curve: fn(f32) -> f32 },
}

/// A trail template that faces the camera and have points follow each other in a smooth manner.
#[derive(Debug, Clone, Copy)]
pub struct ExpDecayTrail<const N: usize> {
    pub buffer: [ExpDecayTrailItem; N],
    pub camera: CameraDirection,
    pub position_decay: f32,
    pub width_curve: WidthCurve,
    pub eps: f32,
}

impl<const N: usize> ExpDecayTrail<N> {
    /// Set the position of the first item,
    /// the rest of the items should follow.
    pub fn set_first(&mut self, position: Vec3) {
        if N == 0 {
            return;
        }
        self.buffer[0].position = position;
    }
}

impl<const N: usize> Default for ExpDecayTrail<N> {
    fn default() -> Self {
        Self {
            buffer: [Default::default(); N],
            camera: CameraDirection::Orthographic { direction: Vec3::Y },
            position_decay: 16.,
            width_curve: WidthCurve::Fac(|_| 1.),
            eps: 0.001,
        }
    }
}

pub(crate) trait ExpDecay:
    Mul<f32, Output = Self> + Add<Self, Output = Self> + Sub<Self, Output = Self> + Sized + Copy
{
    fn exp_decay(&mut self, b: Self, decay: f32, dt: f32) {
        *self = b + (*self - b) * f32::exp(-decay * dt)
    }
}

impl<T> ExpDecay for T where
    T: Mul<f32, Output = Self> + Add<Self, Output = Self> + Sub<Self, Output = Self> + Sized + Copy
{
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExpDecayTrailItem {
    pub position: Vec3,
    pub width: f32,
}

impl<const N: usize> TrailBuffer for ExpDecayTrail<N> {
    fn update(&mut self, dt: f32) {
        if N <= 1 {
            return;
        }

        match self.width_curve {
            WidthCurve::Fac(curve) => {
                for (idx, item) in self.buffer.iter_mut().enumerate() {
                    item.width = curve(idx as f32 / (N - 1) as f32);
                }
            }
            WidthCurve::Distance { max, curve } => {
                let mut distance = 0.;
                let last = None;
                for item in self.buffer.iter_mut() {
                    if let Some(prev) = last {
                        distance += item.position.distance(prev);
                    }
                    item.width = curve(distance / max);
                }
            }
        }
        //item.width.exp_decay(self.min_width, 1., dt);
        for i in 0..N - 1 {
            let last = self.buffer[i].position;
            self.buffer[i + 1]
                .position
                .exp_decay(last, self.position_decay, dt);
        }
    }

    fn expired(&self) -> bool {
        if N == 0 {
            true
        } else {
            self.buffer[0]
                .position
                .distance(self.buffer[N - 1].position)
                < self.eps
        }
    }

    fn build_trail(&self, mesh: &mut Mesh) {
        if N <= 1 {
            return;
        }
        TrailMeshBuilder::new(mesh).build_plane(
            self.buffer.iter().enumerate().map(|(i, x)| {
                let normal = match self.camera {
                    CameraDirection::Orthographic { direction } => -direction,
                    CameraDirection::Perspective { position } => position - x.position,
                };
                let tangent = if i == 0 {
                    self.buffer[1].position - self.buffer[0].position
                } else if i == N - 1 {
                    self.buffer[N - 1].position - self.buffer[N - 2].position
                } else {
                    self.buffer[i + 1].position - self.buffer[i - 1].position
                };
                (x.position, normal.cross(tangent).normalize(), x.width)
            }),
            0.0..1.0,
        )
    }
}
