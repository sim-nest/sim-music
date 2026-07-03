use crate::Dx7PatchOperator;

/// DX7 keyboard scaling for one operator: level scaling around a breakpoint
/// plus envelope rate scaling across the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7KeyboardScaling {
    /// Breakpoint key position (0..=99, measured from MIDI key 21) that
    /// divides the left and right scaling regions.
    pub breakpoint: u8,
    /// Level scaling depth (0..=99) applied to keys below the breakpoint.
    pub left_depth: u8,
    /// Level scaling depth (0..=99) applied to keys above the breakpoint.
    pub right_depth: u8,
    /// Curve shape (0..=3) for the region below the breakpoint: linear or
    /// exponential, decreasing or increasing.
    pub left_curve: u8,
    /// Curve shape (0..=3) for the region above the breakpoint.
    pub right_curve: u8,
    /// Envelope rate scaling amount (0..=7) that speeds envelopes up toward
    /// higher keys.
    pub rate_scale: u8,
}

impl Dx7KeyboardScaling {
    /// Builds keyboard scaling from a patch operator, clamping each field to
    /// its valid DX7 range.
    pub fn from_patch_operator(operator: &Dx7PatchOperator) -> Self {
        Self {
            breakpoint: operator.breakpoint.min(99),
            left_depth: operator.left_depth.min(99),
            right_depth: operator.right_depth.min(99),
            left_curve: operator.left_curve.min(3),
            right_curve: operator.right_curve.min(3),
            rate_scale: operator.rate_scale.min(7),
        }
    }

    /// Returns the output level gain (clamped to 0.0..=2.0) for `key`,
    /// applying the appropriate depth and curve for its side of the
    /// breakpoint.
    pub fn level_gain(self, key: u8) -> f32 {
        let key_position = key.saturating_sub(21).min(99);
        let (depth, curve, distance) = if key_position < self.breakpoint {
            (
                self.left_depth,
                self.left_curve,
                f32::from(self.breakpoint - key_position),
            )
        } else {
            (
                self.right_depth,
                self.right_curve,
                f32::from(key_position - self.breakpoint),
            )
        };
        let amount = f32::from(depth) / 99.0 * (distance / 99.0);
        match curve {
            0 => 1.0 - amount,
            1 => 1.0 - amount * amount,
            2 => 1.0 + amount,
            _ => 1.0 + amount * amount,
        }
        .clamp(0.0, 2.0)
    }

    /// Returns the envelope rate boost for `key`, scaling [`rate_scale`] by
    /// the key position so higher keys advance envelopes faster.
    ///
    /// [`rate_scale`]: Dx7KeyboardScaling::rate_scale
    pub fn rate_boost(self, key: u8) -> u8 {
        ((u16::from(key.min(127)) * u16::from(self.rate_scale)) / 127) as u8
    }
}

impl Default for Dx7KeyboardScaling {
    /// Returns flat scaling: a mid-keyboard breakpoint with zero depth, curves,
    /// and rate scaling, so level and envelope rates are uniform across keys.
    fn default() -> Self {
        Self {
            breakpoint: 50,
            left_depth: 0,
            right_depth: 0,
            left_curve: 0,
            right_curve: 0,
            rate_scale: 0,
        }
    }
}
