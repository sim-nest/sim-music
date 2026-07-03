use sim_kernel::Symbol;

/// Polarity convention for a control-voltage range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CvPolarity {
    /// A signed range centered on zero (for example `-5 V..=+5 V`).
    Bipolar,
    /// An unsigned range starting at zero (for example `0 V..=5 V`).
    Unipolar,
}

impl CvPolarity {
    /// Returns the stable kebab-case identifier for this polarity.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bipolar => "bipolar",
            Self::Unipolar => "unipolar",
        }
    }

    /// Returns the qualified symbol naming this polarity.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/cv-polarity", self.as_str())
    }
}

/// A control-voltage range: a [`CvPolarity`] plus its minimum and maximum
/// voltage bounds, used to clamp, normalize, and scale CV values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CvConvention {
    polarity: CvPolarity,
    min_volts: f32,
    max_volts: f32,
}

impl CvConvention {
    /// Builds a bipolar convention spanning `-max_abs_volts..=+max_abs_volts`.
    ///
    /// The magnitude is taken as an absolute value and floored to a tiny
    /// positive epsilon so the range never collapses.
    pub fn bipolar(max_abs_volts: f32) -> Self {
        let max_abs_volts = max_abs_volts.abs().max(f32::EPSILON);
        Self {
            polarity: CvPolarity::Bipolar,
            min_volts: -max_abs_volts,
            max_volts: max_abs_volts,
        }
    }

    /// Builds a unipolar convention spanning `0..=max_volts`.
    ///
    /// The maximum is floored to a tiny positive epsilon so the range never
    /// collapses.
    pub fn unipolar(max_volts: f32) -> Self {
        Self {
            polarity: CvPolarity::Unipolar,
            min_volts: 0.0,
            max_volts: max_volts.max(f32::EPSILON),
        }
    }

    /// Returns the polarity of this convention.
    pub fn polarity(self) -> CvPolarity {
        self.polarity
    }

    /// Returns the minimum voltage of the range.
    pub fn min_volts(self) -> f32 {
        self.min_volts
    }

    /// Returns the maximum voltage of the range.
    pub fn max_volts(self) -> f32 {
        self.max_volts
    }

    /// Clamps `volts` to the convention's `[min, max]` range.
    pub fn clamp(self, volts: f32) -> f32 {
        volts.clamp(self.min_volts, self.max_volts)
    }

    /// Maps `volts` to a normalized `0.0..=1.0` position within the range,
    /// clamping out-of-range input first.
    pub fn normalize(self, volts: f32) -> f32 {
        (self.clamp(volts) - self.min_volts) / (self.max_volts - self.min_volts)
    }

    /// Maps a normalized `0.0..=1.0` position back to volts within the range,
    /// clamping the input first. Inverse of [`normalize`](Self::normalize).
    pub fn scale(self, normalized: f32) -> f32 {
        self.min_volts + normalized.clamp(0.0, 1.0) * (self.max_volts - self.min_volts)
    }
}

impl Default for CvConvention {
    fn default() -> Self {
        Self::bipolar(5.0)
    }
}

/// A control-voltage reading: a raw voltage paired with the [`CvConvention`]
/// that gives it meaning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlVoltage {
    volts: f32,
    convention: CvConvention,
}

impl ControlVoltage {
    /// Builds a control voltage of `volts` interpreted under `convention`.
    pub fn new(volts: f32, convention: CvConvention) -> Self {
        Self { volts, convention }
    }

    /// Returns the raw, unclamped voltage.
    pub fn volts(self) -> f32 {
        self.volts
    }

    /// Returns the convention this voltage is interpreted under.
    pub fn convention(self) -> CvConvention {
        self.convention
    }

    /// Returns the voltage clamped to the convention's range.
    pub fn clamped_volts(self) -> f32 {
        self.convention.clamp(self.volts)
    }

    /// Returns the voltage as a normalized `0.0..=1.0` position in the range.
    pub fn normalized(self) -> f32 {
        self.convention.normalize(self.volts)
    }
}

/// A volts-per-octave pitch convention: the MIDI key that maps to zero volts
/// and the voltage spanned by one octave (twelve semitones).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoltsPerOctave {
    zero_volt_key: u8,
    volts_per_octave: f32,
}

impl VoltsPerOctave {
    /// Builds a convention anchored at `zero_volt_key` with the given octave
    /// span. The span is taken as an absolute value floored to a tiny epsilon.
    pub fn new(zero_volt_key: u8, volts_per_octave: f32) -> Self {
        Self {
            zero_volt_key,
            volts_per_octave: volts_per_octave.abs().max(f32::EPSILON),
        }
    }

    /// Returns the MIDI key that maps to zero volts.
    pub fn zero_volt_key(self) -> u8 {
        self.zero_volt_key
    }

    /// Returns the voltage spanned by one octave.
    pub fn volts_per_octave(self) -> f32 {
        self.volts_per_octave
    }

    /// Converts a MIDI key number to its pitch CV, in volts.
    pub fn midi_key_to_volts(self, key: u8) -> f32 {
        (f32::from(key) - f32::from(self.zero_volt_key)) * self.volts_per_octave / 12.0
    }

    /// Converts a pitch CV back to a (fractional) MIDI key number.
    pub fn volts_to_midi_key(self, volts: f32) -> f32 {
        f32::from(self.zero_volt_key) + volts * 12.0 / self.volts_per_octave
    }

    /// Converts a frequency in hertz to its pitch CV, in volts, via the
    /// A440 / MIDI-key 69 reference.
    pub fn frequency_hz_to_volts(self, frequency_hz: f32) -> f32 {
        let midi_key = 69.0 + 12.0 * (frequency_hz.max(f32::EPSILON) / 440.0).log2();
        (midi_key - f32::from(self.zero_volt_key)) * self.volts_per_octave / 12.0
    }
}

impl Default for VoltsPerOctave {
    fn default() -> Self {
        Self::new(60, 1.0)
    }
}

/// Electrical gate signalling convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateMode {
    /// A positive voltage gate: high voltage means active (note held).
    VoltageGate,
    /// A positive voltage trigger: a brief high pulse marks an event.
    VoltageTrigger,
    /// A Moog-style S-trigger: low voltage (a closure to ground) means active.
    STrigger,
}

impl GateMode {
    /// Returns the stable kebab-case identifier for this mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VoltageGate => "voltage-gate",
            Self::VoltageTrigger => "voltage-trigger",
            Self::STrigger => "s-trigger",
        }
    }

    /// Returns the qualified symbol naming this gate mode.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/gate", self.as_str())
    }
}

/// A gate signalling convention: a [`GateMode`] plus its Schmitt-trigger
/// thresholds and the active/inactive output voltages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateConvention {
    mode: GateMode,
    low_threshold_v: f32,
    high_threshold_v: f32,
    inactive_voltage_v: f32,
    active_voltage_v: f32,
}

impl GateConvention {
    /// Builds the standard positive voltage-gate convention
    /// (0.8 V / 2.0 V thresholds, 0 V inactive, 5 V active).
    pub fn voltage_gate() -> Self {
        Self {
            mode: GateMode::VoltageGate,
            low_threshold_v: 0.8,
            high_threshold_v: 2.0,
            inactive_voltage_v: 0.0,
            active_voltage_v: 5.0,
        }
    }

    /// Builds a positive voltage-trigger convention, sharing the voltage-gate
    /// thresholds and voltages but tagged as [`GateMode::VoltageTrigger`].
    pub fn voltage_trigger() -> Self {
        Self {
            mode: GateMode::VoltageTrigger,
            ..Self::voltage_gate()
        }
    }

    /// Builds an S-trigger convention with inverted polarity
    /// (5 V inactive, 0 V active) and active-when-low detection.
    pub fn s_trigger() -> Self {
        Self {
            mode: GateMode::STrigger,
            low_threshold_v: 0.8,
            high_threshold_v: 2.0,
            inactive_voltage_v: 5.0,
            active_voltage_v: 0.0,
        }
    }

    /// Returns the gate mode of this convention.
    pub fn mode(self) -> GateMode {
        self.mode
    }

    /// Returns the low (release) threshold voltage.
    pub fn low_threshold_v(self) -> f32 {
        self.low_threshold_v
    }

    /// Returns the high (assert) threshold voltage.
    pub fn high_threshold_v(self) -> f32 {
        self.high_threshold_v
    }

    /// Reports whether `volts` reads as an active gate under this convention.
    ///
    /// Voltage modes are active at or above the high threshold; S-trigger is
    /// active at or below the low threshold.
    pub fn is_active(self, volts: f32) -> bool {
        match self.mode {
            GateMode::VoltageGate | GateMode::VoltageTrigger => volts >= self.high_threshold_v,
            GateMode::STrigger => volts <= self.low_threshold_v,
        }
    }

    /// Returns this convention's own output voltage for the given active state.
    pub fn native_voltage(self, active: bool) -> f32 {
        if active {
            self.active_voltage_v
        } else {
            self.inactive_voltage_v
        }
    }

    /// Translates `volts` (read under this convention) into the equivalent
    /// standard positive voltage-gate output voltage.
    pub fn voltage_gate_voltage(self, volts: f32) -> f32 {
        let voltage_gate = Self::voltage_gate();
        voltage_gate.native_voltage(self.is_active(volts))
    }
}

impl Default for GateConvention {
    fn default() -> Self {
        Self::voltage_gate()
    }
}

/// One converted gate observation produced by a [`GateConverter`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateFrame {
    /// Whether the gate reads as active this frame.
    pub active: bool,
    /// Whether this frame is a rising edge (active now, inactive last frame).
    pub triggered: bool,
    /// The equivalent standard positive voltage-gate output voltage.
    pub voltage_gate_volts: f32,
}

/// Stateful converter that normalizes incoming gate voltages to [`GateFrame`]s,
/// tracking the previous active state to detect rising edges.
#[derive(Clone, Debug, PartialEq)]
pub struct GateConverter {
    convention: GateConvention,
    was_active: bool,
}

impl GateConverter {
    /// Builds a converter for `convention`, starting in the inactive state.
    pub fn new(convention: GateConvention) -> Self {
        Self {
            convention,
            was_active: false,
        }
    }

    /// Returns the convention this converter reads under.
    pub fn convention(&self) -> GateConvention {
        self.convention
    }

    /// Clears the remembered active state so the next active frame is an edge.
    pub fn reset(&mut self) {
        self.was_active = false;
    }

    /// Converts `volts` to a [`GateFrame`], updating the edge-detection state.
    pub fn convert(&mut self, volts: f32) -> GateFrame {
        let active = self.convention.is_active(volts);
        let triggered = active && !self.was_active;
        self.was_active = active;
        GateFrame {
            active,
            triggered,
            voltage_gate_volts: self.convention.voltage_gate_voltage(volts),
        }
    }
}
