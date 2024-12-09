use std::ops::{Add, Mul, Sub};

use bevy::math::Vec3;

#[derive(Debug, Clone, Copy)]
pub enum WidthCurve {
    Fac(fn(f32) -> f32),
    Distance { max: f32, curve: fn(f32) -> f32 },
}

/// A trail template that have points follow each other in a smooth manner.
#[derive(Debug, Clone, Copy)]
pub struct ExpDecayTrail<const N: usize> {
    pub buffer: [(Vec3, f32); N],
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
        self.buffer[0].0 = position;
    }
}

impl<const N: usize> Default for ExpDecayTrail<N> {
    fn default() -> Self {
        Self {
            buffer: [Default::default(); N],
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

impl<const N: usize> ExpDecayTrail<N> {
    pub fn update(&mut self, dt: f32) {
        if N <= 1 {
            return;
        }

        match self.width_curve {
            WidthCurve::Fac(curve) => {
                for (idx, item) in self.buffer.iter_mut().enumerate() {
                    item.1 = curve(idx as f32 / (N - 1) as f32);
                }
            }
            WidthCurve::Distance { max, curve } => {
                let mut distance = 0.;
                let last = None;
                for item in self.buffer.iter_mut() {
                    if let Some(prev) = last {
                        distance += item.0.distance(prev);
                    }
                    item.1 = curve(distance / max);
                }
            }
        }
        //item.width.exp_decay(self.min_width, 1., dt);
        for i in 0..N - 1 {
            let last = self.buffer[i].0;
            self.buffer[i + 1]
                .0
                .exp_decay(last, self.position_decay, dt);
        }
    }

    pub fn is_expired(&self) -> bool {
        if N == 0 {
            true
        } else {
            self.buffer[0].0.distance(self.buffer[N - 1].0) < self.eps
        }
    }
}
