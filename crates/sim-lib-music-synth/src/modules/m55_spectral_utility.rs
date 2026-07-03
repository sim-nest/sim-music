use std::f32::consts::TAU;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Number of bands in the System 55 fixed filter bank.
pub const SYSTEM55_FIXED_FILTER_BAND_COUNT: usize = 10;
/// Center frequencies in hertz for each fixed filter bank band.
pub const SYSTEM55_FIXED_FILTER_BANK_CENTERS_HZ: [f32; SYSTEM55_FIXED_FILTER_BAND_COUNT] = [
    125.0, 175.0, 250.0, 350.0, 500.0, 700.0, 1_000.0, 1_400.0, 2_000.0, 2_800.0,
];

/// Configuration for the System 55 fixed filter bank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55FixedFilterBankSettings {
    /// Per-band output gains applied to each filtered band.
    pub gains: [f32; SYSTEM55_FIXED_FILTER_BAND_COUNT],
    /// Overall gain applied to the summed band output.
    pub output_gain: f32,
}

impl Default for System55FixedFilterBankSettings {
    fn default() -> Self {
        Self {
            gains: [1.0; SYSTEM55_FIXED_FILTER_BAND_COUNT],
            output_gain: 1.0,
        }
    }
}

/// Per-sample output of the System 55 fixed filter bank.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System55FixedFilterBankFrame {
    /// Gained output of each individual band.
    pub bands: [f32; SYSTEM55_FIXED_FILTER_BAND_COUNT],
    /// Summed and clamped mix of all bands.
    pub output: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct BandState {
    low: f32,
    band: f32,
}

/// System 55 fixed filter bank: a fixed bank of band-pass filters whose
/// outputs are individually gained and summed.
#[derive(Clone, Debug, PartialEq)]
pub struct System55FixedFilterBank {
    settings: System55FixedFilterBankSettings,
    sample_rate_hz: f32,
    bands: [BandState; SYSTEM55_FIXED_FILTER_BAND_COUNT],
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55FixedFilterBank {
    /// Creates a filter bank from sanitized settings at the default sample
    /// rate with cleared band state.
    pub fn new(settings: System55FixedFilterBankSettings) -> Self {
        Self {
            settings: sanitize_fixed_filter_bank(settings),
            sample_rate_hz: 48_000.0,
            bands: [BandState::default(); SYSTEM55_FIXED_FILTER_BAND_COUNT],
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the fixed center frequencies of the bank's bands.
    pub fn band_centers_hz(&self) -> [f32; SYSTEM55_FIXED_FILTER_BAND_COUNT] {
        SYSTEM55_FIXED_FILTER_BANK_CENTERS_HZ
    }

    /// Sets the processing sample rate in hertz, clamped to at least 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Advances the bank by one sample and returns the per-band and mixed
    /// outputs.
    pub fn next_frame(&mut self, input: f32) -> System55FixedFilterBankFrame {
        let mut bands = [0.0; SYSTEM55_FIXED_FILTER_BAND_COUNT];
        for (index, center_hz) in SYSTEM55_FIXED_FILTER_BANK_CENTERS_HZ.iter().enumerate() {
            let center_hz = center_hz.min(self.sample_rate_hz * 0.45);
            let coeff = (TAU * center_hz / self.sample_rate_hz)
                .sin()
                .abs()
                .clamp(0.001, 0.95);
            let state = &mut self.bands[index];
            state.low += coeff * (input - state.low);
            let high = input - state.low;
            state.band += coeff * (high - state.band);
            bands[index] = state.band * self.settings.gains[index];
        }
        let output = bands.iter().sum::<f32>() * self.settings.output_gain;
        let frame = System55FixedFilterBankFrame {
            bands,
            output: output.clamp(-4.0, 4.0),
        };
        self.last_trace = Some(trace_output(
            m55_fixed_filter_bank_component_id(),
            self.clock,
            frame.output,
        ));
        self.clock = self.clock.saturating_add(1);
        frame
    }
}

impl Default for System55FixedFilterBank {
    fn default() -> Self {
        Self::new(System55FixedFilterBankSettings::default())
    }
}

impl DiscreteComponent for System55FixedFilterBank {
    fn component_id(&self) -> Symbol {
        m55_fixed_filter_bank_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_fixed_filter_bank_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_fixed_filter_bank_params()
    }

    fn reset(&mut self) {
        self.bands = [BandState::default(); SYSTEM55_FIXED_FILTER_BAND_COUNT];
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_frame(input(block.in_audio, 0, frame));
            write_outputs(block.out_audio, frame, &[output.output]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_fixed_filter_bank_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("bands"), self.bands.len().to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for the System 55 frequency shifter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55FrequencyShifterSettings {
    /// Frequency offset in hertz applied to the input spectrum.
    pub shift_hz: f32,
    /// Output level applied to both sidebands.
    pub level: f32,
}

impl Default for System55FrequencyShifterSettings {
    fn default() -> Self {
        Self {
            shift_hz: 100.0,
            level: 1.0,
        }
    }
}

/// Per-sample output of the System 55 frequency shifter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System55FrequencyShifterFrame {
    /// Up-shifted sideband output.
    pub upper_sideband: f32,
    /// Down-shifted sideband output.
    pub lower_sideband: f32,
}

/// System 55 frequency shifter that translates an analytic (in-phase and
/// quadrature) input by a fixed frequency offset.
#[derive(Clone, Debug, PartialEq)]
pub struct System55FrequencyShifter {
    settings: System55FrequencyShifterSettings,
    sample_rate_hz: f32,
    phase: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55FrequencyShifter {
    /// Creates a frequency shifter from sanitized settings at the default
    /// sample rate.
    pub fn new(settings: System55FrequencyShifterSettings) -> Self {
        Self {
            settings: sanitize_frequency_shifter(settings),
            sample_rate_hz: 48_000.0,
            phase: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Sets the processing sample rate in hertz, clamped to at least 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Advances by one sample and returns the upper and lower sidebands of the
    /// shifted input.
    pub fn next_frame(&mut self, in_phase: f32, quadrature: f32) -> System55FrequencyShifterFrame {
        let sin = self.phase.sin();
        let cos = self.phase.cos();
        let frame = System55FrequencyShifterFrame {
            upper_sideband: (in_phase * cos - quadrature * sin) * self.settings.level,
            lower_sideband: (in_phase * cos + quadrature * sin) * self.settings.level,
        };
        self.phase = (self.phase + TAU * self.settings.shift_hz / self.sample_rate_hz) % TAU;
        self.last_trace = Some(trace_output(
            m55_frequency_shifter_component_id(),
            self.clock,
            frame.upper_sideband,
        ));
        self.clock = self.clock.saturating_add(1);
        frame
    }
}

impl Default for System55FrequencyShifter {
    fn default() -> Self {
        Self::new(System55FrequencyShifterSettings::default())
    }
}

impl DiscreteComponent for System55FrequencyShifter {
    fn component_id(&self) -> Symbol {
        m55_frequency_shifter_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_frequency_shifter_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_frequency_shifter_params()
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_frame(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_outputs(
                block.out_audio,
                frame,
                &[output.upper_sideband, output.lower_sideband],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_frequency_shifter_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("shift-hz"), self.settings.shift_hz.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for the System 55 ring modulator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55RingModulatorSettings {
    /// Output level applied to the modulated product.
    pub level: f32,
}

impl Default for System55RingModulatorSettings {
    fn default() -> Self {
        Self { level: 1.0 }
    }
}

/// System 55 ring modulator that multiplies a carrier and modulator input.
#[derive(Clone, Debug, PartialEq)]
pub struct System55RingModulator {
    settings: System55RingModulatorSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55RingModulator {
    /// Creates a ring modulator from sanitized settings.
    pub fn new(settings: System55RingModulatorSettings) -> Self {
        Self {
            settings: sanitize_ring(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Advances by one sample and returns the clamped product of the carrier
    /// and modulator.
    pub fn next_sample(&mut self, carrier: f32, modulator: f32) -> f32 {
        let output = (carrier * modulator * self.settings.level).clamp(-2.0, 2.0);
        self.last_trace = Some(trace_output(m55_ring_component_id(), self.clock, output));
        self.clock = self.clock.saturating_add(1);
        output
    }
}

impl Default for System55RingModulator {
    fn default() -> Self {
        Self::new(System55RingModulatorSettings::default())
    }
}

impl DiscreteComponent for System55RingModulator {
    fn component_id(&self) -> Symbol {
        m55_ring_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_ring_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_ring_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_outputs(block.out_audio, frame, &[output]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(m55_ring_component_id(), ComponentBackend::Algorithmic, true)
            .with_field(inspect_key("level"), self.settings.level.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the stable component id for the fixed filter bank module.
pub fn m55_fixed_filter_bank_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-907-fixed-filter-bank")
}

/// Returns the stable component id for the frequency shifter module.
pub fn m55_frequency_shifter_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-1630-frequency-shifter")
}

/// Returns the stable component id for the ring modulator module.
pub fn m55_ring_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-ring-modulator")
}

/// Returns the port descriptors for the fixed filter bank module.
pub fn m55_fixed_filter_bank_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the port descriptors for the frequency shifter module.
pub fn m55_frequency_shifter_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("in-phase-in", ComponentPortMedia::AudioRate),
        input_port("quadrature-in", ComponentPortMedia::AudioRate).optional(),
        output_port("upper-sideband-out", ComponentPortMedia::AudioRate),
        output_port("lower-sideband-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the port descriptors for the ring modulator module.
pub fn m55_ring_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("carrier-in", ComponentPortMedia::AudioRate),
        input_port("modulator-in", ComponentPortMedia::AudioRate),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the fixed filter bank module.
pub fn m55_fixed_filter_bank_params() -> Vec<ComponentParamDescriptor> {
    vec![gain_param("output-gain", "Output gain", 1.0)]
}

/// Returns the parameter descriptors for the frequency shifter module.
pub fn m55_frequency_shifter_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("shift-hz"), "Shift", ComponentParamUnit::Hertz)
            .with_range(ComponentParamRange::new(-5_000.0, 5_000.0, 100.0)),
        gain_param("level", "Level", 1.0),
    ]
}

/// Returns the parameter descriptors for the ring modulator module.
pub fn m55_ring_params() -> Vec<ComponentParamDescriptor> {
    vec![gain_param("level", "Level", 1.0)]
}

fn sanitize_fixed_filter_bank(
    settings: System55FixedFilterBankSettings,
) -> System55FixedFilterBankSettings {
    System55FixedFilterBankSettings {
        gains: settings.gains.map(|gain| gain.clamp(0.0, 2.0)),
        output_gain: settings.output_gain.clamp(0.0, 2.0),
    }
}

fn sanitize_frequency_shifter(
    settings: System55FrequencyShifterSettings,
) -> System55FrequencyShifterSettings {
    System55FrequencyShifterSettings {
        shift_hz: settings.shift_hz.clamp(-5_000.0, 5_000.0),
        level: settings.level.clamp(0.0, 2.0),
    }
}

fn sanitize_ring(settings: System55RingModulatorSettings) -> System55RingModulatorSettings {
    System55RingModulatorSettings {
        level: settings.level.clamp(0.0, 2.0),
    }
}

fn gain_param(name: &'static str, label: &'static str, default: f64) -> ComponentParamDescriptor {
    ComponentParamDescriptor::new(param_key(name), label, ComponentParamUnit::Unitless)
        .with_range(ComponentParamRange::new(0.0, 2.0, default))
}

fn trace_output(id: Symbol, clock: u64, output: f32) -> ComponentTraceFrame {
    ComponentTraceFrame::new(id, ComponentBackend::Algorithmic, clock).with_output(
        trace_key("output"),
        ComponentTraceValue::Float(f64::from(output)),
    )
}

fn input(channels: &[&[f32]], channel: usize, frame: usize) -> f32 {
    channels
        .get(channel)
        .and_then(|samples| samples.get(frame))
        .copied()
        .unwrap_or(0.0)
}

fn write_outputs(outputs: &mut [&mut [f32]], frame: usize, samples: &[f32]) {
    for (channel, output) in outputs.iter_mut().enumerate() {
        output[frame] = samples
            .get(channel)
            .copied()
            .or_else(|| samples.last().copied())
            .unwrap_or(0.0);
    }
}

fn input_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Input, 1)
}

fn output_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Output, 1)
}

fn port_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/m55-port", name)
}

fn param_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/m55-param", name)
}

fn inspect_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/m55-inspect", name)
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/m55-trace", name)
}
