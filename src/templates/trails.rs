use bevy::math::{StableInterpolate, Vec3};

#[derive(Debug, Clone, Copy)]
pub enum WidthCurve {
    Fac(fn(f32) -> f32),
    Distance { max: f32, curve: fn(f32) -> f32 },
}

/// A trail template that have points follow each other in a smooth manner.
#[derive(Debug, Clone, Copy)]
pub struct ExpDecayTrail<const N: usize> {
    /// Points and widths of the trail.
    pub buffer: [(Vec3, f32); N],
    /// Exponential decay factor, usually in `10..50`
    pub position_decay: f32,
    /// Width relative to position or length.
    pub width_curve: WidthCurve,
    /// The length of which the curve should be despawned.
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
        for i in 0..N - 1 {
            let last = self.buffer[i].0;
            self.buffer[i + 1]
                .0
                .smooth_nudge(&last, self.position_decay, dt);
        }
    }

    pub fn is_expired(&self) -> bool {
        if N <= 1 {
            true
        } else {
            self.buffer[0].0.distance(self.buffer[N - 1].0) < self.eps
        }
    }
}
