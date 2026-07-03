use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_midi_sysex::{Dx7Operator, Dx7Voice};

use crate::{InstrumentPatch, PatchJack, PatchModule, PatchRawView};

const LIB_NS: &str = "audio-synth";

/// A parsed DX7 voice patch: named operators, global modulation settings, and
/// the raw SysEx bytes it was decoded from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7Patch {
    /// Voice name as stored in the patch.
    pub name: String,
    /// The six FM operators.
    pub operators: Vec<Dx7PatchOperator>,
    /// Global pitch envelope generator.
    pub pitch_envelope: Dx7Envelope,
    /// Algorithm number (1..=32) selecting operator routing.
    pub algorithm: u8,
    /// Operator-1 self-feedback amount.
    pub feedback: u8,
    /// Whether oscillator phases reset (sync) on key-on.
    pub oscillator_sync: bool,
    /// Low-frequency oscillator settings.
    pub lfo: Dx7Lfo,
    /// Master transpose in semitones (centered offset).
    pub transpose: u8,
    /// Raw patch bytes preserved for round-tripping.
    pub raw: Dx7RawPatch,
}

impl Dx7Patch {
    /// Builds a patch from a decoded [`Dx7Voice`], preserving its raw bytes.
    pub fn from_voice(voice: &Dx7Voice) -> Self {
        let common = voice.common();
        Self {
            name: common.name,
            operators: voice
                .operators()
                .iter()
                .map(Dx7PatchOperator::from_operator)
                .collect(),
            pitch_envelope: Dx7Envelope {
                rates: common.pitch_rates,
                levels: common.pitch_levels,
            },
            algorithm: common.algorithm,
            feedback: common.feedback,
            oscillator_sync: common.oscillator_sync,
            lfo: Dx7Lfo {
                speed: common.lfo_speed,
                delay: common.lfo_delay,
                pitch_mod_depth: common.lfo_pitch_mod_depth,
                amp_mod_depth: common.lfo_amp_mod_depth,
                sync: common.lfo_sync,
                waveform: common.lfo_waveform,
                pitch_mod_sens: common.pitch_mod_sens,
            },
            transpose: common.transpose,
            raw: Dx7RawPatch {
                edit_buffer: voice.edit_buffer().to_vec(),
                packed_voice: voice.packed_voice(),
            },
        }
    }

    /// Lowers the patch into a generic [`InstrumentPatch`] with per-operator
    /// modules and a raw-byte view.
    pub fn to_instrument_patch(&self) -> InstrumentPatch {
        let mut patch = InstrumentPatch::new(Symbol::qualified(LIB_NS, "dx7"))
            .with_setting(Symbol::new("name"), Expr::String(self.name.clone()))
            .with_setting(Symbol::new("algorithm"), number_u8(self.algorithm))
            .with_setting(Symbol::new("feedback"), number_u8(self.feedback))
            .with_setting(Symbol::new("transpose"), number_u8(self.transpose))
            .with_raw_view(self.raw.to_raw_view());

        for (index, operator) in self.operators.iter().enumerate() {
            patch = patch.with_module(operator.to_patch_module(index));
        }
        patch
    }
}

impl Default for Dx7Patch {
    fn default() -> Self {
        Self {
            name: "SIM DX7 INIT".to_owned(),
            operators: vec![Dx7PatchOperator::default(); 6],
            pitch_envelope: Dx7Envelope::default(),
            algorithm: 1,
            feedback: 0,
            oscillator_sync: false,
            lfo: Dx7Lfo::default(),
            transpose: 24,
            raw: Dx7RawPatch::default(),
        }
    }
}

/// One DX7 FM operator: its envelope, keyboard scaling, and oscillator tuning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7PatchOperator {
    /// Four envelope-generator rates (R1..R4).
    pub rates: [u8; 4],
    /// Four envelope-generator levels (L1..L4).
    pub levels: [u8; 4],
    /// Keyboard-level-scaling breakpoint key.
    pub breakpoint: u8,
    /// Level-scaling depth below the breakpoint.
    pub left_depth: u8,
    /// Level-scaling depth above the breakpoint.
    pub right_depth: u8,
    /// Level-scaling curve below the breakpoint.
    pub left_curve: u8,
    /// Level-scaling curve above the breakpoint.
    pub right_curve: u8,
    /// Envelope rate keyboard scaling.
    pub rate_scale: u8,
    /// Amplitude-modulation sensitivity.
    pub amp_mod_sens: u8,
    /// Key-velocity sensitivity.
    pub key_velocity_sens: u8,
    /// Operator output level.
    pub output_level: u8,
    /// Oscillator mode (0 = frequency ratio, 1 = fixed frequency).
    pub oscillator_mode: u8,
    /// Coarse frequency selector.
    pub frequency_coarse: u8,
    /// Fine frequency selector.
    pub frequency_fine: u8,
    /// Oscillator detune amount.
    pub detune: u8,
}

impl Dx7PatchOperator {
    fn from_operator(operator: &Dx7Operator) -> Self {
        Self {
            rates: operator.rates,
            levels: operator.levels,
            breakpoint: operator.breakpoint,
            left_depth: operator.left_depth,
            right_depth: operator.right_depth,
            left_curve: operator.left_curve,
            right_curve: operator.right_curve,
            rate_scale: operator.rate_scale,
            amp_mod_sens: operator.amp_mod_sens,
            key_velocity_sens: operator.key_velocity_sens,
            output_level: operator.output_level,
            oscillator_mode: operator.oscillator_mode,
            frequency_coarse: operator.frequency_coarse,
            frequency_fine: operator.frequency_fine,
            detune: operator.detune,
        }
    }

    fn to_patch_module(&self, index: usize) -> PatchModule {
        PatchModule::new(
            Symbol::new(format!("operator-{}", index + 1)),
            dx7_patch_component_kind(),
        )
        .with_input(PatchJack::control("pitch", true))
        .with_input(PatchJack::control("modulation", false))
        .with_output(PatchJack::audio("audio", true))
        .with_setting(Symbol::new("rates"), byte_vector(&self.rates))
        .with_setting(Symbol::new("levels"), byte_vector(&self.levels))
        .with_setting(Symbol::new("output-level"), number_u8(self.output_level))
        .with_setting(
            Symbol::new("oscillator-mode"),
            number_u8(self.oscillator_mode),
        )
        .with_setting(
            Symbol::new("frequency-coarse"),
            number_u8(self.frequency_coarse),
        )
        .with_setting(
            Symbol::new("frequency-fine"),
            number_u8(self.frequency_fine),
        )
        .with_setting(Symbol::new("detune"), number_u8(self.detune))
    }
}

impl Default for Dx7PatchOperator {
    fn default() -> Self {
        Self {
            rates: [99, 80, 70, 60],
            levels: [99, 80, 60, 0],
            breakpoint: 39,
            left_depth: 0,
            right_depth: 0,
            left_curve: 0,
            right_curve: 0,
            rate_scale: 0,
            amp_mod_sens: 0,
            key_velocity_sens: 0,
            output_level: 99,
            oscillator_mode: 0,
            frequency_coarse: 1,
            frequency_fine: 0,
            detune: 7,
        }
    }
}

/// A four-stage DX7 envelope generator (rates and levels), used for the pitch
/// envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7Envelope {
    /// Four envelope rates (R1..R4).
    pub rates: [u8; 4],
    /// Four envelope levels (L1..L4).
    pub levels: [u8; 4],
}

impl Default for Dx7Envelope {
    fn default() -> Self {
        Self {
            rates: [99, 80, 70, 60],
            levels: [99, 80, 60, 50],
        }
    }
}

/// DX7 low-frequency oscillator settings shared by all operators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7Lfo {
    /// LFO speed (rate).
    pub speed: u8,
    /// LFO delay before onset.
    pub delay: u8,
    /// Pitch-modulation depth.
    pub pitch_mod_depth: u8,
    /// Amplitude-modulation depth.
    pub amp_mod_depth: u8,
    /// Whether the LFO resets phase on key-on.
    pub sync: bool,
    /// LFO waveform selector.
    pub waveform: u8,
    /// Pitch-modulation sensitivity.
    pub pitch_mod_sens: u8,
}

impl Default for Dx7Lfo {
    fn default() -> Self {
        Self {
            speed: 35,
            delay: 0,
            pitch_mod_depth: 0,
            amp_mod_depth: 0,
            sync: false,
            waveform: 0,
            pitch_mod_sens: 0,
        }
    }
}

/// Raw DX7 SysEx bytes preserved for exact round-tripping.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dx7RawPatch {
    /// Unpacked 155-byte edit-buffer representation.
    pub edit_buffer: Vec<u8>,
    /// Packed 128-byte voice representation.
    pub packed_voice: Vec<u8>,
}

impl Dx7RawPatch {
    fn to_raw_view(&self) -> PatchRawView {
        PatchRawView::new(Symbol::qualified("audio-synth/raw", "dx7-voice"))
            .with_field(
                Symbol::new("edit-buffer"),
                Expr::Bytes(self.edit_buffer.clone()),
            )
            .with_field(
                Symbol::new("packed-voice"),
                Expr::Bytes(self.packed_voice.clone()),
            )
    }
}

/// Returns the qualified component-kind symbol for DX7 patch operators.
pub fn dx7_patch_component_kind() -> Symbol {
    Symbol::qualified(LIB_NS, "dx7-operator")
}

fn byte_vector(bytes: &[u8]) -> Expr {
    Expr::Vector(bytes.iter().map(|byte| number_u8(*byte)).collect())
}

fn number_u8(value: u8) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}
