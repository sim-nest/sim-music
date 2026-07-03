use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::{
    ControlVoltage, CvConvention, GateConvention, GateConverter, GateFrame, InstrumentPatch,
    PatchJack, PatchModule, VoltsPerOctave,
};

const LIB_NS: &str = "audio-synth";

/// A bus of per-key gate inputs feeding a [`PolyphonicArray`], one entry per
/// currently sounding key.
#[derive(Clone, Debug, PartialEq)]
pub struct PerKeyGateBus {
    keys: Vec<PerKeyGateInput>,
}

impl PerKeyGateBus {
    /// Builds a bus from an explicit list of per-key inputs.
    pub fn new(keys: Vec<PerKeyGateInput>) -> Self {
        Self { keys }
    }

    /// Appends a per-key input and returns the bus (builder style).
    pub fn with_key(mut self, key: PerKeyGateInput) -> Self {
        self.keys.push(key);
        self
    }

    /// Returns the per-key inputs on this bus, in order.
    pub fn keys(&self) -> &[PerKeyGateInput] {
        &self.keys
    }
}

impl Default for PerKeyGateBus {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

/// One key's gate state on a [`PerKeyGateBus`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerKeyGateInput {
    /// MIDI key number for this entry.
    pub key: u8,
    /// Raw gate voltage for this key.
    pub gate_volts: f32,
    /// Normalized note velocity in `0.0..=1.0`.
    pub velocity: f32,
}

impl PerKeyGateInput {
    /// Builds a per-key input, clamping `velocity` into `0.0..=1.0`.
    pub fn new(key: u8, gate_volts: f32, velocity: f32) -> Self {
        Self {
            key,
            gate_volts,
            velocity: velocity.clamp(0.0, 1.0),
        }
    }
}

/// A per-voice module setting, scoped to a named section, that is replicated
/// across every voice of a [`PolyphonicArray`]'s patch.
#[derive(Clone, Debug, PartialEq)]
pub struct PolyphonicSectionSetting {
    /// The section name the setting belongs to.
    pub section: Symbol,
    /// The setting key within the section.
    pub key: Symbol,
    /// The setting value.
    pub value: Expr,
}

impl PolyphonicSectionSetting {
    /// Builds a section setting from its section, key, and value.
    pub fn new(section: Symbol, key: Symbol, value: Expr) -> Self {
        Self {
            section,
            key,
            value,
        }
    }

    fn patch_key(&self) -> Symbol {
        Symbol::qualified(
            format!("{LIB_NS}/section/{}", self.section.name),
            self.key.name.to_string(),
        )
    }
}

/// The resolved signal assigned to one voice after fanning out a gate bus:
/// its pitch CV, converted gate, and velocity.
#[derive(Clone, Debug, PartialEq)]
pub struct PolyKeySignal {
    /// Index of the voice this signal drives.
    pub voice_index: usize,
    /// MIDI key number routed to the voice.
    pub key: u8,
    /// Pitch control voltage for the voice.
    pub pitch: ControlVoltage,
    /// Converted gate frame for the voice.
    pub gate: GateFrame,
    /// Normalized note velocity for the voice.
    pub velocity: f32,
}

/// A fixed bank of synth voices that fans a [`PerKeyGateBus`] out to per-voice
/// pitch CV and gate signals, and emits a per-voice [`InstrumentPatch`].
#[derive(Clone, Debug, PartialEq)]
pub struct PolyphonicArray {
    id: Symbol,
    voice_count: usize,
    pitch: VoltsPerOctave,
    pitch_cv: CvConvention,
    gate: GateConvention,
    section_settings: Vec<PolyphonicSectionSetting>,
    converters: Vec<GateConverter>,
}

impl PolyphonicArray {
    /// Builds an array of `voice_count` voices (floored to at least one) with
    /// the given id, pitch, and gate conventions and a default bipolar 5 V
    /// pitch CV range.
    pub fn new(
        id: Symbol,
        voice_count: usize,
        pitch: VoltsPerOctave,
        gate: GateConvention,
    ) -> Self {
        let voice_count = voice_count.max(1);
        Self {
            id,
            voice_count,
            pitch,
            pitch_cv: CvConvention::bipolar(5.0),
            gate,
            section_settings: Vec::new(),
            converters: vec![GateConverter::new(gate); voice_count],
        }
    }

    /// Overrides the pitch CV convention and returns the array (builder style).
    pub fn with_pitch_cv(mut self, pitch_cv: CvConvention) -> Self {
        self.pitch_cv = pitch_cv;
        self
    }

    /// Adds a per-voice section setting and returns the array (builder style).
    pub fn with_section_setting(mut self, setting: PolyphonicSectionSetting) -> Self {
        self.section_settings.push(setting);
        self
    }

    /// Returns the array's identifying symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the number of voices in the array.
    pub fn voice_count(&self) -> usize {
        self.voice_count
    }

    /// Returns the pitch (volts-per-octave) convention.
    pub fn pitch(&self) -> VoltsPerOctave {
        self.pitch
    }

    /// Returns the gate convention.
    pub fn gate(&self) -> GateConvention {
        self.gate
    }

    /// Returns the per-voice section settings.
    pub fn section_settings(&self) -> &[PolyphonicSectionSetting] {
        &self.section_settings
    }

    /// Resets every voice's gate converter, clearing edge-detection state.
    pub fn reset(&mut self) {
        for converter in &mut self.converters {
            converter.reset();
        }
    }

    /// Distributes bus entries across voices, producing one [`PolyKeySignal`]
    /// per assigned voice (entries beyond [`voice_count`](Self::voice_count)
    /// are dropped) with pitch CV and per-voice gate conversion applied.
    pub fn fan_out(&mut self, bus: &PerKeyGateBus) -> Vec<PolyKeySignal> {
        bus.keys()
            .iter()
            .take(self.voice_count)
            .enumerate()
            .map(|(voice_index, input)| {
                let pitch_volts = self.pitch.midi_key_to_volts(input.key);
                PolyKeySignal {
                    voice_index,
                    key: input.key,
                    pitch: ControlVoltage::new(pitch_volts, self.pitch_cv),
                    gate: self.converters[voice_index].convert(input.gate_volts),
                    velocity: input.velocity,
                }
            })
            .collect()
    }

    /// Builds an [`InstrumentPatch`] with one `poly-voice` module per voice,
    /// wired with pitch CV input, gate input, and audio output jacks and
    /// carrying the pitch, gate, and section settings for each voice.
    pub fn per_note_patch(&self) -> InstrumentPatch {
        let mut patch = InstrumentPatch::new(self.id.clone());
        for voice_index in 0..self.voice_count {
            let mut module = PatchModule::new(
                Symbol::new(format!("voice-{voice_index}")),
                Symbol::qualified(LIB_NS, "poly-voice"),
            )
            .with_input(PatchJack::cv("pitch", true))
            .with_input(PatchJack::gate("gate", true))
            .with_output(PatchJack::audio("audio", true))
            .with_setting(Symbol::new("voice-index"), number_usize(voice_index))
            .with_setting(
                Symbol::new("pitch-zero-key"),
                number_usize(usize::from(self.pitch.zero_volt_key())),
            )
            .with_setting(
                Symbol::new("volts-per-octave"),
                number_f32(self.pitch.volts_per_octave()),
            )
            .with_setting(
                Symbol::new("gate-mode"),
                Expr::Symbol(self.gate.mode().symbol()),
            );
            for setting in &self.section_settings {
                module = module.with_setting(setting.patch_key(), setting.value.clone());
            }
            patch = patch.with_module(module);
        }
        patch
    }
}

fn number_f32(value: f32) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "f64"),
        canonical: value.to_string(),
    })
}

fn number_usize(value: usize) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}
