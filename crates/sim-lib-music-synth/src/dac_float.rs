use crate::DX7_OPS_OUTPUT_BITS;

/// Bit width of the signed DAC input word (the OPS output width).
pub const DX7_DAC_INPUT_BITS: u8 = DX7_OPS_OUTPUT_BITS;
/// Bit width of the held floating sample emitted by the DAC.
pub const DX7_DAC_HELD_SAMPLE_BITS: u8 = 24;

const DAC_FULL_SCALE: f32 = ((1_u32 << (DX7_DAC_INPUT_BITS - 1)) - 1) as f32;

/// Declared word widths of the floating DAC stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7DacWordWidths {
    /// Signed input word bit width.
    pub input_bits: u8,
    /// Held output sample bit width.
    pub held_sample_bits: u8,
}

impl Dx7DacWordWidths {
    /// Returns the word widths the DAC stage is compiled against.
    pub const fn declared() -> Self {
        Self {
            input_bits: DX7_DAC_INPUT_BITS,
            held_sample_bits: DX7_DAC_HELD_SAMPLE_BITS,
        }
    }
}

/// Models the DX7 floating DAC: a sample-and-hold that converts signed integer
/// words to normalized floats and holds each for a fixed number of frames.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7FloatingDac {
    hold_frames: u32,
    hold_clock: u32,
    held_sample: f32,
}

impl Dx7FloatingDac {
    /// Creates a DAC that holds each converted sample for `hold_frames`
    /// (clamped to >= 1) frames.
    pub fn new(hold_frames: u32) -> Self {
        Self {
            hold_frames: hold_frames.max(1),
            hold_clock: 0,
            held_sample: 0.0,
        }
    }

    /// Returns the configured hold length in frames.
    pub fn hold_frames(&self) -> u32 {
        self.hold_frames
    }

    /// Returns the currently held output sample.
    pub fn held_sample(&self) -> f32 {
        self.held_sample
    }

    /// Resets the hold clock and clears the held sample.
    pub fn reset(&mut self) {
        self.hold_clock = 0;
        self.held_sample = 0.0;
    }

    /// Advances the DAC one frame, latching a freshly converted sample when the
    /// hold clock wraps and returning the held value.
    pub fn next_sample(&mut self, raw: i32) -> f32 {
        if self.hold_clock == 0 {
            self.held_sample = Self::convert(raw);
        }
        self.hold_clock = (self.hold_clock + 1) % self.hold_frames;
        self.held_sample
    }

    /// Converts a signed input word to a normalized float clamped to -1.0..=1.0.
    pub fn convert(raw: i32) -> f32 {
        (raw as f32 / DAC_FULL_SCALE).clamp(-1.0, 1.0)
    }
}

impl Default for Dx7FloatingDac {
    fn default() -> Self {
        Self::new(1)
    }
}
