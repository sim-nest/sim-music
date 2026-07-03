//! Fixed-point DSP primitives for deterministic synthesis math.
//!
//! Defines the Q-format types used by the discrete-component DSP path: a
//! descriptor of a fixed-point layout ([`FixedFormat`]), the rounding modes for
//! converting from floating point ([`FixedRounding`]), an unsigned Q0.32 phase
//! accumulator ([`QPhase`]), and a signed Q1.30 level value ([`QLevel`]). These
//! give bit-exact, platform-independent arithmetic for phase tracking and
//! sample/level scaling.

/// Description of a fixed-point Q layout: sign, integer bits, and fractional
/// bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedFormat {
    signed: bool,
    integer_bits: u8,
    fractional_bits: u8,
}

impl FixedFormat {
    /// Builds a format from its sign flag, integer-bit count, and
    /// fractional-bit count.
    pub const fn new(signed: bool, integer_bits: u8, fractional_bits: u8) -> Self {
        Self {
            signed,
            integer_bits,
            fractional_bits,
        }
    }

    /// Returns whether the format reserves a sign bit.
    pub const fn signed(self) -> bool {
        self.signed
    }

    /// Returns the number of integer bits in the layout.
    pub const fn integer_bits(self) -> u8 {
        self.integer_bits
    }

    /// Returns the number of fractional bits in the layout.
    pub const fn fractional_bits(self) -> u8 {
        self.fractional_bits
    }
}

/// Rounding policy applied when quantizing a float into a fixed-point value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedRounding {
    /// Drops the fractional part, rounding toward zero.
    Truncate,
    /// Rounds to the nearest representable value.
    RoundNearest,
    /// Adds half before truncating, biasing ties toward positive infinity.
    BiasTowardPositive,
    /// Rounds half away from zero, preserving symmetry around zero.
    BiasAwayFromZero,
}

/// Unsigned Q0.32 phase accumulator that wraps naturally over one full turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct QPhase {
    raw: u32,
}

impl QPhase {
    /// Fixed-point layout of the phase value: unsigned Q0.32.
    pub const FORMAT: FixedFormat = FixedFormat::new(false, 0, 32);
    /// Number of fractional bits (the whole 32-bit word is fractional).
    pub const FRACTIONAL_BITS: u8 = 32;
    /// Phase of zero turns.
    pub const ZERO: Self = Self { raw: 0 };

    /// Wraps a raw 32-bit accumulator value as a phase.
    pub const fn from_raw(raw: u32) -> Self {
        Self { raw }
    }

    /// Builds a phase from a fraction of a turn, wrapping into `[0, 1)`.
    pub fn from_turns(turns: f64) -> Self {
        let wrapped = turns.rem_euclid(1.0);
        let raw = (wrapped * phase_scale()).floor() as u32;
        Self { raw }
    }

    /// Builds the phase that maps the given index of a `len`-entry table to its
    /// position on the unit circle; returns [`QPhase::ZERO`] for an empty table.
    pub fn from_table_index(index: usize, len: usize) -> Self {
        if len == 0 {
            return Self::ZERO;
        }
        Self::from_turns(index as f64 / len as f64)
    }

    /// Returns the underlying raw 32-bit accumulator value.
    pub const fn raw(self) -> u32 {
        self.raw
    }

    /// Returns the phase as a fraction of a turn in `[0, 1)`.
    pub fn turns(self) -> f64 {
        f64::from(self.raw) / phase_scale()
    }

    /// Advances by `rhs`, wrapping modulo one turn on overflow.
    pub fn wrapping_add(self, rhs: Self) -> Self {
        Self {
            raw: self.raw.wrapping_add(rhs.raw),
        }
    }

    /// Retreats by `rhs`, wrapping modulo one turn on underflow.
    pub fn wrapping_sub(self, rhs: Self) -> Self {
        Self {
            raw: self.raw.wrapping_sub(rhs.raw),
        }
    }

    /// Advances by `rhs`, clamping at the maximum representable phase instead of
    /// wrapping.
    pub fn saturating_add(self, rhs: Self) -> Self {
        Self {
            raw: self.raw.saturating_add(rhs.raw),
        }
    }

    /// Advances this accumulator by `delta` in place, wrapping over one turn.
    pub fn advance_wrapping(&mut self, delta: Self) {
        *self = self.wrapping_add(delta);
    }

    /// Resolves the phase to a `len`-entry table lookup, returning the left and
    /// right sample indices plus the interpolation fraction, or `None` for an
    /// empty table.
    pub fn table_position(self, len: usize) -> Option<(usize, usize, f32)> {
        if len == 0 {
            return None;
        }
        let position = self.turns() * len as f64;
        let left = position.floor() as usize % len;
        let right = (left + 1) % len;
        Some((left, right, (position - position.floor()) as f32))
    }
}

/// Signed Q1.30 level value used for sample amplitudes and modulation depths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct QLevel {
    raw: i32,
}

impl QLevel {
    /// Fixed-point layout of the level value: signed Q1.30.
    pub const FORMAT: FixedFormat = FixedFormat::new(true, 1, 30);
    /// Number of fractional bits below the unit position.
    pub const FRACTIONAL_BITS: u8 = 30;
    /// Level of zero.
    pub const ZERO: Self = Self { raw: 0 };
    /// Level of exactly one (full scale at the unit position).
    pub const ONE: Self = Self { raw: 1 << 30 };
    /// Most negative representable level.
    pub const MIN: Self = Self { raw: i32::MIN };
    /// Most positive representable level.
    pub const MAX: Self = Self { raw: i32::MAX };

    /// Wraps a raw 32-bit Q1.30 value as a level.
    pub const fn from_raw(raw: i32) -> Self {
        Self { raw }
    }

    /// Converts an `f32` to a level using round-to-nearest, saturating on
    /// overflow.
    pub fn from_f32(value: f32) -> Self {
        Self::from_f64(value as f64, FixedRounding::RoundNearest)
    }

    /// Converts an `f64` to a level under the given rounding mode, saturating on
    /// overflow.
    pub fn from_f64(value: f64, rounding: FixedRounding) -> Self {
        Self::from_scaled(value * level_scale(), rounding)
    }

    /// Converts an `f64` to a level, adding `raw_bias` in raw units before
    /// rounding and saturation.
    pub fn from_f64_with_raw_bias(value: f64, raw_bias: i32, rounding: FixedRounding) -> Self {
        Self::from_scaled(value * level_scale() + f64::from(raw_bias), rounding)
    }

    /// Returns the underlying raw Q1.30 value.
    pub const fn raw(self) -> i32 {
        self.raw
    }

    /// Converts the level back to an `f32` in unit-scaled space.
    pub fn to_f32(self) -> f32 {
        (f64::from(self.raw) / level_scale()) as f32
    }

    /// Adds `rhs`, clamping at the representable range on overflow.
    pub fn saturating_add(self, rhs: Self) -> Self {
        Self {
            raw: self.raw.saturating_add(rhs.raw),
        }
    }

    /// Adds `rhs`, wrapping on overflow.
    pub fn wrapping_add(self, rhs: Self) -> Self {
        Self {
            raw: self.raw.wrapping_add(rhs.raw),
        }
    }

    /// Subtracts `rhs`, clamping at the representable range on underflow.
    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self {
            raw: self.raw.saturating_sub(rhs.raw),
        }
    }

    /// Subtracts `rhs`, wrapping on underflow.
    pub fn wrapping_sub(self, rhs: Self) -> Self {
        Self {
            raw: self.raw.wrapping_sub(rhs.raw),
        }
    }

    /// Multiplies two Q1.30 levels (rescaling by [`QLevel::FRACTIONAL_BITS`]),
    /// clamping the product to the representable range.
    pub fn saturating_mul(self, rhs: Self) -> Self {
        let product = (i64::from(self.raw) * i64::from(rhs.raw)) >> Self::FRACTIONAL_BITS;
        Self {
            raw: clamp_i64_to_i32(product),
        }
    }

    /// Multiplies two Q1.30 levels (rescaling by [`QLevel::FRACTIONAL_BITS`]),
    /// wrapping the product on overflow.
    pub fn wrapping_mul(self, rhs: Self) -> Self {
        let product = (i64::from(self.raw) * i64::from(rhs.raw)) >> Self::FRACTIONAL_BITS;
        Self {
            raw: product as i32,
        }
    }

    /// Adds a raw-unit bias to the level, clamping on overflow.
    pub fn saturating_add_raw_bias(self, raw_bias: i32) -> Self {
        Self {
            raw: self.raw.saturating_add(raw_bias),
        }
    }

    /// Shifts the raw value right by `bits` with round-to-nearest (half away
    /// from zero); returns 0 once the shift consumes the whole word.
    pub fn rounded_shift_right(self, bits: u8) -> i32 {
        if bits == 0 {
            return self.raw;
        }
        if bits >= 31 {
            return 0;
        }
        let add = 1_i64 << (bits - 1);
        let raw = i64::from(self.raw);
        let biased = if raw >= 0 { raw + add } else { raw - add };
        (biased >> bits) as i32
    }

    /// Shifts the raw value right by `bits`, truncating the magnitude toward
    /// zero; returns 0 once the shift consumes the whole word.
    pub fn truncated_shift_right(self, bits: u8) -> i32 {
        if bits >= 31 {
            return 0;
        }
        let raw = i64::from(self.raw);
        let magnitude = raw.abs() >> bits;
        if raw < 0 {
            -(magnitude as i32)
        } else {
            magnitude as i32
        }
    }

    fn from_scaled(scaled: f64, rounding: FixedRounding) -> Self {
        let rounded = match rounding {
            FixedRounding::Truncate => scaled.trunc(),
            FixedRounding::RoundNearest => scaled.round(),
            FixedRounding::BiasTowardPositive => (scaled + 0.5).trunc(),
            FixedRounding::BiasAwayFromZero if scaled >= 0.0 => (scaled + 0.5).trunc(),
            FixedRounding::BiasAwayFromZero => (scaled - 0.5).trunc(),
        };
        Self {
            raw: clamp_i64_to_i32(rounded as i64),
        }
    }
}

fn phase_scale() -> f64 {
    u32::MAX as f64 + 1.0
}

fn level_scale() -> f64 {
    (1_i64 << QLevel::FRACTIONAL_BITS) as f64
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
