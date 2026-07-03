use crate::QPhase;

/// Bit width of the OPS phase accumulator.
pub const DX7_OPS_PHASE_BITS: u8 = 32;
/// Bit width of the log-domain (log-sine and attenuation) words.
pub const DX7_OPS_LOG_BITS: u8 = 15;
/// Bit width of the signed OPS output word.
pub const DX7_OPS_OUTPUT_BITS: u8 = 14;
/// Number of address bits into the sine lookup table.
pub const DX7_OPS_TABLE_BITS: u8 = 10;
/// Length of the OPS sine lookup table (`1 << DX7_OPS_TABLE_BITS`).
pub const DX7_OPS_TABLE_LEN: usize = 1 << DX7_OPS_TABLE_BITS;

const OUTPUT_MAX: i32 = (1 << (DX7_OPS_OUTPUT_BITS - 1)) - 1;
const LOG_MAX: u16 = (1 << DX7_OPS_LOG_BITS) - 1;
const LEVEL_MAX: u16 = (1 << DX7_OPS_OUTPUT_BITS) - 1;

/// Declared fixed-point word widths of the OPS sine/exp datapath.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7OpsWordWidths {
    /// Phase-accumulator bit width.
    pub phase_bits: u8,
    /// Log-domain word bit width.
    pub log_bits: u8,
    /// Signed output word bit width.
    pub output_bits: u8,
    /// Sine table address bit width.
    pub table_bits: u8,
}

impl Dx7OpsWordWidths {
    /// Returns the word widths the OPS datapath is compiled against.
    pub const fn declared() -> Self {
        Self {
            phase_bits: DX7_OPS_PHASE_BITS,
            log_bits: DX7_OPS_LOG_BITS,
            output_bits: DX7_OPS_OUTPUT_BITS,
            table_bits: DX7_OPS_TABLE_BITS,
        }
    }
}

/// One sample of input to the OPS datapath.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7OpsInput {
    /// Current oscillator phase.
    pub phase: QPhase,
    /// Phase-modulation input from other operators.
    pub modulation: i32,
    /// Envelope-generator level word.
    pub envelope: u16,
    /// Self-feedback amount (clamped to 0..=7).
    pub feedback: u8,
}

/// One sample of output from the OPS datapath, including intermediate
/// datapath words for tracing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7OpsOutput {
    /// Signed operator output sample.
    pub raw: i32,
    /// Output accumulated with the modulation input.
    pub cascade: i32,
    /// Table index derived from the modulated phase.
    pub phase_index: u16,
    /// Log-sine table value at `phase_index`.
    pub log_sine: u16,
    /// Exponentiated output level after envelope attenuation.
    pub exp_level: u16,
    /// Mean of the two prior feedback samples.
    pub feedback_average: i32,
}

/// Stateful OPS sine/exp operator datapath with a two-sample feedback delay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7OpsDatapath {
    feedback: [i32; 2],
}

impl Dx7OpsDatapath {
    /// Creates a datapath with cleared feedback history.
    pub fn new() -> Self {
        Self { feedback: [0; 2] }
    }

    /// Clears the feedback delay line.
    pub fn reset(&mut self) {
        self.feedback = [0; 2];
    }

    /// Returns the two-sample feedback delay line.
    pub fn feedback(&self) -> [i32; 2] {
        self.feedback
    }

    /// Advances the datapath by one sample and returns its output.
    pub fn next(&mut self, input: Dx7OpsInput) -> Dx7OpsOutput {
        let feedback_average = (self.feedback[0] + self.feedback[1]) / 2;
        let feedback_offset = feedback_average.saturating_mul(i32::from(input.feedback.min(7))) / 7;
        let phase = input.phase.raw().wrapping_add(phase_offset(
            input.modulation.saturating_add(feedback_offset),
        ));
        let phase_index = (phase >> (DX7_OPS_PHASE_BITS - DX7_OPS_TABLE_BITS)) as u16;
        let log_sine = log_sine_lookup(phase_index);
        let exp_level = exp_output_level(log_sine, input.envelope);
        let raw = signed_output(phase_index, exp_level);
        let cascade = cascade_accumulate(input.modulation, raw);
        self.feedback = [raw, self.feedback[0]];
        Dx7OpsOutput {
            raw,
            cascade,
            phase_index,
            log_sine,
            exp_level,
            feedback_average,
        }
    }
}

impl Default for Dx7OpsDatapath {
    fn default() -> Self {
        Self::new()
    }
}

/// Saturating sum of two operator samples for cascaded modulation.
pub fn cascade_accumulate(left: i32, right: i32) -> i32 {
    left.saturating_add(right)
}

/// Returns the quarter-folded parabolic log-sine value for a phase index.
pub fn log_sine_lookup(index: u16) -> u16 {
    let position = usize::from(index) & (DX7_OPS_TABLE_LEN - 1);
    let half_position = position & ((DX7_OPS_TABLE_LEN / 2) - 1);
    let quarter = DX7_OPS_TABLE_LEN / 4;
    let folded = if half_position <= quarter {
        half_position
    } else {
        (DX7_OPS_TABLE_LEN / 2) - half_position
    };
    let distance = quarter.saturating_sub(folded);
    ((distance * distance * usize::from(LOG_MAX)) / (quarter * quarter)) as u16
}

/// Exponentiates a log-sine value attenuated by the envelope into an output
/// level word, returning zero once total attenuation saturates.
pub fn exp_output_level(log_sine: u16, envelope: u16) -> u16 {
    let envelope_loss = LEVEL_MAX.saturating_sub(envelope.min(LEVEL_MAX));
    let attenuation = u32::from(log_sine) + u32::from(envelope_loss);
    if attenuation >= u32::from(LOG_MAX) {
        return 0;
    }
    let remaining = u32::from(LOG_MAX) - attenuation;
    ((remaining * LEVEL_MAX as u32) / u32::from(LOG_MAX)) as u16
}

fn signed_output(index: u16, level: u16) -> i32 {
    let magnitude = (i32::from(level) * OUTPUT_MAX) / i32::from(LEVEL_MAX);
    if (usize::from(index) & (DX7_OPS_TABLE_LEN / 2)) == 0 {
        magnitude
    } else {
        -magnitude
    }
}

fn phase_offset(raw: i32) -> u32 {
    ((i64::from(raw) * (1_i64 << 17)) / i64::from(OUTPUT_MAX)) as u32
}
