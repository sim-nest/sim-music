/// DX7 operator velocity sensitivity, mapping note-on velocity to an output
/// level gain factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7VelocitySensitivity {
    /// Velocity sensitivity amount in the DX7 range 0..=7, where 0 ignores
    /// velocity entirely and 7 gives the strongest velocity response.
    pub sensitivity: u8,
}

impl Dx7VelocitySensitivity {
    /// Creates a velocity sensitivity, clamping `sensitivity` to the DX7
    /// range 0..=7.
    pub fn new(sensitivity: u8) -> Self {
        Self {
            sensitivity: sensitivity.min(7),
        }
    }

    /// Returns the output level gain for `velocity` (clamped to 0.0..=1.0).
    ///
    /// At sensitivity 0 the gain is always 1.0; higher sensitivity scales the
    /// gain down for low velocities.
    pub fn gain(self, velocity: f32) -> f32 {
        let velocity = velocity.clamp(0.0, 1.0);
        let depth = f32::from(self.sensitivity) / 7.0;
        (1.0 - depth) + depth * velocity
    }
}

impl Default for Dx7VelocitySensitivity {
    /// Returns a sensitivity of 0, so output level is independent of velocity.
    fn default() -> Self {
        Self::new(0)
    }
}
