use std::f32::consts::TAU;

use crate::QPhase;

/// Function family a [`GeneratedLut`] tabulates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedLutKind {
    /// One-cycle sine table.
    Sine,
    /// Exponential table.
    Exp,
    /// Natural-log table.
    Log,
}

/// A precomputed lookup table sampled with linear interpolation, either wrapping
/// (periodic) or clamped to its input range.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedLut {
    kind: GeneratedLutKind,
    input_min: f32,
    input_max: f32,
    wraps: bool,
    samples: Vec<f32>,
}

impl GeneratedLut {
    /// Builds a one-cycle sine table. `sample` wraps and interpolates between
    /// the last and first samples, so phase input is continuous at the turn.
    pub fn sine(len: usize) -> Self {
        let len = len.max(2);
        let samples = (0..len)
            .map(|index| (TAU * index as f32 / len as f32).sin())
            .collect();
        Self {
            kind: GeneratedLutKind::Sine,
            input_min: 0.0,
            input_max: 1.0,
            wraps: true,
            samples,
        }
    }

    /// Builds an exponential table over a clamped input range. `sample` uses
    /// linear interpolation between adjacent generated values.
    pub fn exp(len: usize, input_min: f32, input_max: f32) -> Self {
        let (input_min, input_max) = ordered_range(input_min, input_max, 1.0);
        Self::generate(
            GeneratedLutKind::Exp,
            len,
            input_min,
            input_max,
            false,
            f32::exp,
        )
    }

    /// Builds a natural-log table over a positive clamped input range. `sample`
    /// uses linear interpolation between adjacent generated values.
    pub fn log(len: usize, input_min: f32, input_max: f32) -> Self {
        let input_min = input_min.max(f32::MIN_POSITIVE);
        let input_max = input_max.max(input_min + f32::EPSILON);
        Self::generate(
            GeneratedLutKind::Log,
            len,
            input_min,
            input_max,
            false,
            f32::ln,
        )
    }

    /// Returns the function family this table tabulates.
    pub fn kind(&self) -> GeneratedLutKind {
        self.kind
    }

    /// Returns the lower bound of the tabulated input range.
    pub fn input_min(&self) -> f32 {
        self.input_min
    }

    /// Returns the upper bound of the tabulated input range.
    pub fn input_max(&self) -> f32 {
        self.input_max
    }

    /// Returns whether the table wraps periodically at its range ends.
    pub fn wraps(&self) -> bool {
        self.wraps
    }

    /// Returns the generated sample values.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Samples the table at `input` with linear interpolation, wrapping or
    /// clamping per the table's mode.
    pub fn sample(&self, input: f32) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        if self.samples.len() == 1 {
            return self.samples[0];
        }
        if self.wraps {
            return self.sample_wrapped(input);
        }
        self.sample_clamped(input)
    }

    /// Samples the table from a [`QPhase`], using its turns for wrapping tables
    /// and the range minimum otherwise.
    pub fn sample_phase(&self, phase: QPhase) -> f32 {
        if self.wraps {
            self.sample(phase.turns() as f32)
        } else {
            self.sample(self.input_min)
        }
    }

    fn generate(
        kind: GeneratedLutKind,
        len: usize,
        input_min: f32,
        input_max: f32,
        wraps: bool,
        f: impl Fn(f32) -> f32,
    ) -> Self {
        let len = len.max(2);
        let span = input_max - input_min;
        let samples = (0..len)
            .map(|index| {
                let t = if len == 1 {
                    0.0
                } else {
                    index as f32 / (len - 1) as f32
                };
                f(input_min + span * t)
            })
            .collect();
        Self {
            kind,
            input_min,
            input_max,
            wraps,
            samples,
        }
    }

    fn sample_wrapped(&self, input: f32) -> f32 {
        let span = (self.input_max - self.input_min).max(f32::EPSILON);
        let normalized = (input - self.input_min).rem_euclid(span) / span;
        let position = normalized * self.samples.len() as f32;
        let left = position.floor() as usize % self.samples.len();
        let right = (left + 1) % self.samples.len();
        let frac = position - position.floor();
        lerp(self.samples[left], self.samples[right], frac)
    }

    fn sample_clamped(&self, input: f32) -> f32 {
        let span = self.input_max - self.input_min;
        if span <= f32::EPSILON {
            return self.samples[0];
        }
        let normalized =
            ((input.clamp(self.input_min, self.input_max) - self.input_min) / span).clamp(0.0, 1.0);
        let position = normalized * (self.samples.len() - 1) as f32;
        let left = position.floor() as usize;
        let right = (left + 1).min(self.samples.len() - 1);
        let frac = position - position.floor();
        lerp(self.samples[left], self.samples[right], frac)
    }
}

fn ordered_range(input_min: f32, input_max: f32, fallback_span: f32) -> (f32, f32) {
    if input_min < input_max {
        (input_min, input_max)
    } else if input_min > input_max {
        (input_max, input_min)
    } else {
        (input_min, input_min + fallback_span)
    }
}

fn lerp(left: f32, right: f32, frac: f32) -> f32 {
    left * (1.0 - frac) + right * frac
}
