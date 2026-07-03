/// Named modulation source feeding a [`ModulationMatrix`] route.
///
/// Each variant selects one channel of a [`ModulationInput`] frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModSource {
    /// The first low-frequency oscillator.
    Lfo1,
    /// The first envelope generator.
    Envelope1,
    /// The note velocity of the triggering key.
    Velocity,
    /// A fixed constant value (a unipolar offset source).
    Constant,
}

impl ModSource {
    /// Returns the stable kebab-case identifier for this source.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lfo1 => "lfo1",
            Self::Envelope1 => "envelope1",
            Self::Velocity => "velocity",
            Self::Constant => "constant",
        }
    }

    /// Parses a source from its [`as_str`](Self::as_str) identifier, returning
    /// `None` for an unknown name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "lfo1" => Some(Self::Lfo1),
            "envelope1" => Some(Self::Envelope1),
            "velocity" => Some(Self::Velocity),
            "constant" => Some(Self::Constant),
            _ => None,
        }
    }
}

/// Named synth parameter that a [`ModulationMatrix`] route can drive.
///
/// Each variant accumulates into one field of a [`SynthModulation`] frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModTarget {
    /// Oscillator pitch offset, in semitones.
    OscPitchSemitones,
    /// Filter cutoff offset, in hertz.
    FilterCutoffHz,
    /// Amplifier gain offset (linear).
    AmpGain,
    /// Pulse-width offset for pulse/square oscillators.
    PulseWidth,
}

impl ModTarget {
    /// Returns the stable kebab-case identifier for this target.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OscPitchSemitones => "osc-pitch-semitones",
            Self::FilterCutoffHz => "filter-cutoff-hz",
            Self::AmpGain => "amp-gain",
            Self::PulseWidth => "pulse-width",
        }
    }

    /// Parses a target from its [`as_str`](Self::as_str) identifier, returning
    /// `None` for an unknown name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "osc-pitch-semitones" => Some(Self::OscPitchSemitones),
            "filter-cutoff-hz" => Some(Self::FilterCutoffHz),
            "amp-gain" => Some(Self::AmpGain),
            "pulse-width" => Some(Self::PulseWidth),
            _ => None,
        }
    }
}

/// A single wire in a [`ModulationMatrix`]: a scaled connection from a
/// [`ModSource`] to a [`ModTarget`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModulationRoute {
    /// The source channel read for this route.
    pub source: ModSource,
    /// The synth parameter driven by this route.
    pub target: ModTarget,
    /// The scaling applied to the source value before it accumulates at the
    /// target.
    pub amount: f32,
}

/// One frame of modulation source values, sampled together for a route apply.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModulationInput {
    /// Value of the first LFO this frame.
    pub lfo1: f32,
    /// Value of the first envelope this frame.
    pub envelope1: f32,
    /// Normalized note velocity for this frame.
    pub velocity: f32,
    /// The constant source value (typically a fixed `1.0`).
    pub constant: f32,
}

/// The accumulated modulation offsets produced by applying a matrix.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SynthModulation {
    /// Summed oscillator pitch offset, in semitones.
    pub osc_pitch_semitones: f32,
    /// Summed filter cutoff offset, in hertz.
    pub filter_cutoff_hz: f32,
    /// Summed amplifier gain offset (linear).
    pub amp_gain: f32,
    /// Summed pulse-width offset.
    pub pulse_width: f32,
}

/// An ordered set of [`ModulationRoute`]s that maps a [`ModulationInput`] frame
/// to a [`SynthModulation`] output by summing each route's contribution.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModulationMatrix {
    routes: Vec<ModulationRoute>,
}

impl ModulationMatrix {
    /// Builds a matrix from an explicit list of routes.
    pub fn new(routes: Vec<ModulationRoute>) -> Self {
        Self { routes }
    }

    /// Returns the routes in this matrix, in apply order.
    pub fn routes(&self) -> &[ModulationRoute] {
        &self.routes
    }

    /// Appends a route to the end of the matrix.
    pub fn push(&mut self, route: ModulationRoute) {
        self.routes.push(route);
    }

    /// Applies every route to `input`, summing scaled source values into the
    /// matching [`SynthModulation`] field.
    pub fn apply(&self, input: ModulationInput) -> SynthModulation {
        let mut output = SynthModulation::default();
        for route in &self.routes {
            let value = source_value(route.source, input) * route.amount;
            match route.target {
                ModTarget::OscPitchSemitones => output.osc_pitch_semitones += value,
                ModTarget::FilterCutoffHz => output.filter_cutoff_hz += value,
                ModTarget::AmpGain => output.amp_gain += value,
                ModTarget::PulseWidth => output.pulse_width += value,
            }
        }
        output
    }
}

fn source_value(source: ModSource, input: ModulationInput) -> f32 {
    match source {
        ModSource::Lfo1 => input.lfo1,
        ModSource::Envelope1 => input.envelope1,
        ModSource::Velocity => input.velocity,
        ModSource::Constant => input.constant,
    }
}
