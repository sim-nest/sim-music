use std::f32::consts::PI;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use super::{
    inspect_key, ps3_per_key_cell_component_id, ps3_per_key_cell_params, ps3_per_key_cell_ports,
    ps3_poly_array_component_id, ps3_poly_array_params, ps3_poly_array_ports, trace_key,
};
use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentPortDescriptor,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    ps3300::{PS3300_KEY_COUNT, ps3300_keyboard_assignment},
};

/// Per-key voltage-controlled filter settings for a voice cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300PerNoteVcfSettings {
    /// Base cutoff frequency, in Hz.
    pub cutoff_hz: f32,
    /// Resonance amount, in 0.0..=1.0.
    pub resonance: f32,
    /// Octaves of cutoff shift per volt of pitch CV (keyboard tracking).
    pub keyboard_tracking_octaves: f32,
    /// Octaves of cutoff shift driven by the full envelope.
    pub envelope_depth_octaves: f32,
}

impl Default for Ps3300PerNoteVcfSettings {
    fn default() -> Self {
        Self {
            cutoff_hz: 1_200.0,
            resonance: 0.35,
            keyboard_tracking_octaves: 0.5,
            envelope_depth_octaves: 1.25,
        }
    }
}

/// Per-key ADSR envelope settings for a voice cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300PerNoteEnvelopeSettings {
    /// Attack time, in seconds.
    pub attack_s: f32,
    /// Decay time, in seconds.
    pub decay_s: f32,
    /// Sustain level, in 0.0..=1.0.
    pub sustain: f32,
    /// Release time, in seconds.
    pub release_s: f32,
}

impl Default for Ps3300PerNoteEnvelopeSettings {
    fn default() -> Self {
        Self {
            attack_s: 0.004,
            decay_s: 0.12,
            sustain: 0.68,
            release_s: 0.18,
        }
    }
}

/// Per-key voltage-controlled amplifier settings for a voice cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300PerNoteVcaSettings {
    /// Output level scale, in 0.0..=1.0.
    pub level: f32,
    /// Exponent applied to the envelope to shape the gain response curve.
    pub response_curve: f32,
}

impl Default for Ps3300PerNoteVcaSettings {
    fn default() -> Self {
        Self {
            level: 0.85,
            response_curve: 1.2,
        }
    }
}

/// Full configuration for one [`Ps3300NoteCell`]: its key plus VCF, envelope,
/// and VCA stages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300NoteCellSettings {
    /// MIDI key this cell is responsible for.
    pub midi_key: u8,
    /// Filter settings.
    pub vcf: Ps3300PerNoteVcfSettings,
    /// Envelope settings.
    pub envelope: Ps3300PerNoteEnvelopeSettings,
    /// Amplifier settings.
    pub vca: Ps3300PerNoteVcaSettings,
}

impl Default for Ps3300NoteCellSettings {
    fn default() -> Self {
        Self {
            midi_key: ps3300_keyboard_assignment().first_midi_key,
            vcf: Ps3300PerNoteVcfSettings::default(),
            envelope: Ps3300PerNoteEnvelopeSettings::default(),
            vca: Ps3300PerNoteVcaSettings::default(),
        }
    }
}

/// One rendered sample of a voice cell across its filter/envelope/amp stages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300NoteCellFrame {
    /// Current envelope level, in 0.0..=1.0.
    pub envelope: f32,
    /// Filtered signal after the resonant one-pole VCF.
    pub filtered: f32,
    /// Final cell output after the VCA, clamped to -1.0..=1.0.
    pub output: f32,
    /// Gate state used for this sample.
    pub gate_high: bool,
}

/// One PS-3300 voice cell: a per-key VCF, ADSR envelope, and VCA in series.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300NoteCell {
    settings: Ps3300NoteCellSettings,
    sample_rate_hz: f32,
    envelope_level: f32,
    filter_state: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300NoteCell {
    /// Builds a voice cell from the given (sanitized) settings at a default
    /// 48 kHz sample rate.
    pub fn new(settings: Ps3300NoteCellSettings) -> Self {
        Self {
            settings: sanitize_cell(settings),
            sample_rate_hz: 48_000.0,
            envelope_level: 0.0,
            filter_state: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the current (sanitized) settings.
    pub fn settings(&self) -> Ps3300NoteCellSettings {
        self.settings
    }

    /// Sets the working sample rate in Hz (floored at 1.0).
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the current envelope level, used to detect active cells.
    pub fn envelope_level(&self) -> f32 {
        self.envelope_level
    }

    /// Advances the cell one sample: updates the envelope, sweeps the filter
    /// cutoff, filters `input`, and applies the VCA gain.
    pub fn next_sample(
        &mut self,
        input: f32,
        pitch_cv_v: f32,
        gate_high: bool,
    ) -> Ps3300NoteCellFrame {
        self.envelope_level = next_envelope(
            self.envelope_level,
            gate_high,
            self.settings.envelope,
            self.sample_rate_hz,
        );
        let cutoff_hz = self.cutoff_hz(pitch_cv_v, self.envelope_level);
        let coefficient = one_pole_coefficient(cutoff_hz, self.sample_rate_hz);
        self.filter_state += coefficient * (input - self.filter_state);
        let resonance_drive = 1.0 + self.settings.vcf.resonance * 0.35;
        let filtered = (self.filter_state * resonance_drive).clamp(-1.0, 1.0);
        let gain =
            self.envelope_level.powf(self.settings.vca.response_curve) * self.settings.vca.level;
        let output = (filtered * gain).clamp(-1.0, 1.0);
        let frame = Ps3300NoteCellFrame {
            envelope: self.envelope_level,
            filtered,
            output,
            gate_high,
        };
        self.last_trace = Some(self.trace_frame(cutoff_hz, frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn cutoff_hz(&self, pitch_cv_v: f32, envelope: f32) -> f32 {
        let octaves = pitch_cv_v * self.settings.vcf.keyboard_tracking_octaves
            + envelope * self.settings.vcf.envelope_depth_octaves;
        (self.settings.vcf.cutoff_hz * 2.0_f32.powf(octaves)).clamp(20.0, 18_000.0)
    }

    fn trace_frame(&self, cutoff_hz: f32, frame: Ps3300NoteCellFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_per_key_cell_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("midi-key"),
            ComponentTraceValue::Float(f64::from(self.settings.midi_key)),
        )
        .with_state(
            trace_key("cutoff-hz"),
            ComponentTraceValue::Float(f64::from(cutoff_hz)),
        )
        .with_state(
            trace_key("gate"),
            ComponentTraceValue::Bool(frame.gate_high),
        )
        .with_output(
            trace_key("envelope"),
            ComponentTraceValue::Float(f64::from(frame.envelope)),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(frame.output)),
        )
    }
}

impl Default for Ps3300NoteCell {
    fn default() -> Self {
        Self::new(Ps3300NoteCellSettings::default())
    }
}

impl DiscreteComponent for Ps3300NoteCell {
    fn component_id(&self) -> Symbol {
        ps3_per_key_cell_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_per_key_cell_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_per_key_cell_params()
    }

    fn reset(&mut self) {
        self.envelope_level = 0.0;
        self.filter_state = 0.0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let output = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
                input(block.in_audio, 2, frame) > 0.0,
            );
            write_output(block.out_audio, 0, frame, output.output);
            write_output(block.out_audio, 1, frame, output.envelope);
            write_output(block.out_audio, 2, frame, output.filtered);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_per_key_cell_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("midi-key"), self.settings.midi_key.to_string())
        .with_field(inspect_key("envelope"), self.envelope_level.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for a [`Ps3300PolyArray`] of per-key voice cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300PolyArraySettings {
    /// Output level of the summed array, in 0.0..=1.0.
    pub section_level: f32,
    /// MIDI key of the array's lowest cell.
    pub first_midi_key: u8,
    /// Number of contiguous keys (cells) in the array.
    pub key_count: usize,
}

impl Default for Ps3300PolyArraySettings {
    fn default() -> Self {
        let assignment = ps3300_keyboard_assignment();
        Self {
            section_level: 0.75,
            first_midi_key: assignment.first_midi_key,
            key_count: assignment.key_count,
        }
    }
}

/// One rendered chord of a poly array: the mix plus per-cell detail.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300PolyArrayFrame {
    /// Total number of cells in the array.
    pub cell_count: usize,
    /// Number of cells gated on for this chord.
    pub active_count: usize,
    /// Summed, leveled, and clamped array output.
    pub mixed: f32,
    /// Per-cell `(midi_key, output)` pairs for the chord.
    pub cell_outputs: Vec<(u8, f32)>,
}

/// Polyphonic array of one [`Ps3300NoteCell`] per key, rendering chords by
/// gating the cells whose keys are held.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300PolyArray {
    settings: Ps3300PolyArraySettings,
    cells: Vec<Ps3300NoteCell>,
    sample_rate_hz: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300PolyArray {
    /// Builds a poly array, allocating one voice cell per key from the
    /// (sanitized) settings.
    pub fn new(settings: Ps3300PolyArraySettings) -> Self {
        let settings = sanitize_poly(settings);
        let cells = (0..settings.key_count)
            .map(|offset| {
                Ps3300NoteCell::new(Ps3300NoteCellSettings {
                    midi_key: settings.first_midi_key + offset as u8,
                    ..Ps3300NoteCellSettings::default()
                })
            })
            .collect();
        Self {
            settings,
            cells,
            sample_rate_hz: 48_000.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the total number of cells in the array.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Returns the number of cells whose envelopes are still audibly open.
    pub fn active_cell_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.envelope_level() > 0.0001)
            .count()
    }

    /// Sets the working sample rate in Hz, propagating it to every cell.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
        for cell in &mut self.cells {
            cell.set_sample_rate(self.sample_rate_hz);
        }
    }

    /// Renders one chord: feeds `source_sample` to every cell, gating those
    /// whose keys appear in `active_keys`, and sums the result.
    pub fn next_chord(&mut self, source_sample: f32, active_keys: &[u8]) -> Ps3300PolyArrayFrame {
        let mut mixed = 0.0;
        let mut active_count = 0;
        let mut outputs = Vec::with_capacity(self.cells.len());
        for cell in &mut self.cells {
            let midi_key = cell.settings().midi_key;
            let gate_high = active_keys.contains(&midi_key);
            if gate_high {
                active_count += 1;
            }
            let pitch_cv = (f32::from(midi_key) - f32::from(self.settings.first_midi_key)) / 12.0;
            let frame = cell.next_sample(source_sample, pitch_cv, gate_high);
            mixed += frame.output;
            outputs.push((midi_key, frame.output));
        }
        let mixed =
            (mixed / self.cells.len().max(1) as f32 * self.settings.section_level).clamp(-1.0, 1.0);
        let frame = Ps3300PolyArrayFrame {
            cell_count: self.cells.len(),
            active_count,
            mixed,
            cell_outputs: outputs,
        };
        self.last_trace = Some(self.trace_frame(&frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, frame: &Ps3300PolyArrayFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_poly_array_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("cell-count"),
            ComponentTraceValue::Float(frame.cell_count as f64),
        )
        .with_state(
            trace_key("active-count"),
            ComponentTraceValue::Float(frame.active_count as f64),
        )
        .with_output(
            trace_key("mixed"),
            ComponentTraceValue::Float(f64::from(frame.mixed)),
        )
    }
}

impl Default for Ps3300PolyArray {
    fn default() -> Self {
        Self::new(Ps3300PolyArraySettings::default())
    }
}

impl DiscreteComponent for Ps3300PolyArray {
    fn component_id(&self) -> Symbol {
        ps3_poly_array_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_poly_array_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_poly_array_params()
    }

    fn reset(&mut self) {
        for cell in &mut self.cells {
            cell.reset();
        }
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let gate = input(block.in_audio, 1, frame) > 0.0;
            let key = self.settings.first_midi_key;
            let source = input(block.in_audio, 0, frame);
            let output = if gate {
                self.next_chord(source, &[key])
            } else {
                self.next_chord(source, &[])
            };
            write_output(block.out_audio, 0, frame, output.mixed);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_poly_array_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("cell-count"), self.cell_count().to_string())
        .with_field(
            inspect_key("active-count"),
            self.active_cell_count().to_string(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

fn sanitize_cell(settings: Ps3300NoteCellSettings) -> Ps3300NoteCellSettings {
    Ps3300NoteCellSettings {
        midi_key: settings.midi_key.min(127),
        vcf: Ps3300PerNoteVcfSettings {
            cutoff_hz: settings.vcf.cutoff_hz.clamp(20.0, 18_000.0),
            resonance: settings.vcf.resonance.clamp(0.0, 1.0),
            keyboard_tracking_octaves: settings.vcf.keyboard_tracking_octaves.clamp(0.0, 2.0),
            envelope_depth_octaves: settings.vcf.envelope_depth_octaves.clamp(0.0, 4.0),
        },
        envelope: Ps3300PerNoteEnvelopeSettings {
            attack_s: settings.envelope.attack_s.clamp(0.001, 2.0),
            decay_s: settings.envelope.decay_s.clamp(0.001, 4.0),
            sustain: settings.envelope.sustain.clamp(0.0, 1.0),
            release_s: settings.envelope.release_s.clamp(0.001, 8.0),
        },
        vca: Ps3300PerNoteVcaSettings {
            level: settings.vca.level.clamp(0.0, 1.0),
            response_curve: settings.vca.response_curve.clamp(0.25, 4.0),
        },
    }
}

fn sanitize_poly(settings: Ps3300PolyArraySettings) -> Ps3300PolyArraySettings {
    let first_midi_key = settings.first_midi_key.min(127);
    let available_keys = 128usize - usize::from(first_midi_key);
    let max_key_count = available_keys.clamp(1, PS3300_KEY_COUNT);
    Ps3300PolyArraySettings {
        section_level: settings.section_level.clamp(0.0, 1.0),
        first_midi_key,
        key_count: settings.key_count.clamp(1, max_key_count),
    }
}

fn next_envelope(
    current: f32,
    gate_high: bool,
    settings: Ps3300PerNoteEnvelopeSettings,
    sample_rate_hz: f32,
) -> f32 {
    if gate_high {
        let attack_step = 1.0 / (settings.attack_s * sample_rate_hz).max(1.0);
        if current < 1.0 {
            (current + attack_step).min(1.0)
        } else {
            let decay_step =
                (1.0 - settings.sustain) / (settings.decay_s * sample_rate_hz).max(1.0);
            (current - decay_step).max(settings.sustain)
        }
    } else {
        let release_step = current / (settings.release_s * sample_rate_hz).max(1.0);
        (current - release_step.max(0.000_001)).max(0.0)
    }
}

fn one_pole_coefficient(cutoff_hz: f32, sample_rate_hz: f32) -> f32 {
    let normalized = (2.0 * PI * cutoff_hz / sample_rate_hz.max(1.0)).clamp(0.0, PI);
    (1.0 - (-normalized).exp()).clamp(0.0, 1.0)
}

fn input(channels: &[&[f32]], channel: usize, frame: usize) -> f32 {
    channels
        .get(channel)
        .and_then(|samples| samples.get(frame))
        .copied()
        .unwrap_or(0.0)
}

fn write_output(channels: &mut [&mut [f32]], channel: usize, frame: usize, value: f32) {
    if let Some(samples) = channels.get_mut(channel)
        && let Some(sample) = samples.get_mut(frame)
    {
        *sample = value;
    }
}
