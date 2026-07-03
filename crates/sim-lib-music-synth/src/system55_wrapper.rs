use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::{
    BlockEvent, Graph as AudioGraph, PrepareConfig, ProcessBlock, Processor,
};

use crate::system55::{
    System55Envelope, System55EnvelopeSettings, System55FixedFilterBank, System55Keyboard,
    System55LadderLpf, System55LadderLpfSettings, System55Mixer, System55MixerSettings,
    System55Sequencer, System55SequencerSettings, System55Vca, System55VcaResponse,
    System55VcaSettings, System55Vco, System55VcoDriver, System55VcoSettings, System55VcoWaveform,
};
use crate::system55_patch::{
    System55PatchProfile, System55RenderMode, system55_component_id, system55_default_patch,
    system55_default_patch_id, system55_params, system55_ports, system55_required_module_ids,
};
use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentPortDescriptor,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    GateConvention, InstrumentPatch,
};

/// Complete Moog System 55 voice: an instrument wrapper that wires the modeled
/// modules (keyboard, oscillators, mixer, ladder filter, envelope, VCA, filter
/// bank, and sequencer) into a single playable `Processor` and
/// [`DiscreteComponent`].
#[derive(Clone, Debug, PartialEq)]
pub struct System55 {
    patch: InstrumentPatch,
    profile: System55PatchProfile,
    render_mode: System55RenderMode,
    keyboard: System55Keyboard,
    driver: System55VcoDriver,
    vco_1: System55Vco,
    vco_2: System55Vco,
    vco_3: System55Vco,
    mixer: System55Mixer,
    ladder: System55LadderLpf,
    envelope: System55Envelope,
    vca: System55Vca,
    filter_bank: System55FixedFilterBank,
    sequencer: System55Sequencer,
    gate: bool,
    active_key: Option<u8>,
    clock_frame: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55 {
    /// Builds a System 55 voice from a patch and render mode, instantiating every
    /// modeled module with its default voice tuning.
    pub fn new(patch: InstrumentPatch, render_mode: System55RenderMode) -> Self {
        Self {
            profile: System55PatchProfile::from_patch(&patch),
            patch,
            render_mode,
            keyboard: System55Keyboard::default(),
            driver: System55VcoDriver::default(),
            vco_1: System55Vco::new(System55VcoSettings {
                waveform: System55VcoWaveform::Saw,
                base_frequency_hz: 110.0,
                level: 0.42,
                ..System55VcoSettings::default()
            }),
            vco_2: System55Vco::new(System55VcoSettings {
                waveform: System55VcoWaveform::Pulse,
                base_frequency_hz: 110.8,
                pulse_width: 0.42,
                level: 0.34,
                ..System55VcoSettings::default()
            }),
            vco_3: System55Vco::new(System55VcoSettings {
                waveform: System55VcoWaveform::Triangle,
                base_frequency_hz: 55.0,
                level: 0.28,
                ..System55VcoSettings::default()
            }),
            mixer: System55Mixer::new(System55MixerSettings {
                gains: [0.85, 0.72, 0.58, 0.0],
                output_gain: 0.8,
                drive: 1.25,
            }),
            ladder: System55LadderLpf::new(System55LadderLpfSettings {
                cutoff_hz: 1_250.0,
                resonance: 0.58,
                resonance_cv_depth: 0.75,
                drive: 1.8,
                ..System55LadderLpfSettings::default()
            }),
            envelope: System55Envelope::new(System55EnvelopeSettings {
                attack_s: 0.0,
                decay_s: 0.035,
                sustain_level: 0.88,
                release_s: 0.04,
                level: 1.0,
            }),
            vca: System55Vca::new(System55VcaSettings {
                response: System55VcaResponse::Saturated,
                gain: 0.95,
                saturation_drive: 2.5,
            }),
            filter_bank: System55FixedFilterBank::default(),
            sequencer: System55Sequencer::new(System55SequencerSettings {
                steps: [0.0, 0.0833, 0.1667, 0.25, 0.3333, 0.25, 0.1667, 0.0833],
                step_count: 8,
                gate_mask: 0xff,
            }),
            gate: false,
            active_key: None,
            clock_frame: 0,
            last_trace: None,
        }
    }

    /// Builds a System 55 voice after verifying that every required module is
    /// present and implemented in the given registry, erroring otherwise.
    pub fn from_registry(
        registry: &crate::ComponentRegistry,
        patch: InstrumentPatch,
        render_mode: System55RenderMode,
    ) -> Result<Self> {
        for id in system55_required_module_ids() {
            let entry = registry.get(&id).ok_or_else(|| {
                Error::Eval(format!(
                    "missing System 55 registry module: {}",
                    id.as_qualified_str()
                ))
            })?;
            if !entry.is_implemented() {
                return Err(Error::Eval(format!(
                    "System 55 registry module is not implemented: {}",
                    id.as_qualified_str()
                )));
            }
        }
        Ok(Self::new(patch, render_mode))
    }

    /// Returns the patch this voice was built from.
    pub fn patch(&self) -> &InstrumentPatch {
        &self.patch
    }

    /// Returns the render mode this voice runs in.
    pub fn render_mode(&self) -> System55RenderMode {
        self.render_mode
    }

    /// Returns the qualified id of the default System 55 voice patch.
    pub fn default_patch_id() -> Symbol {
        system55_default_patch_id()
    }

    fn handle_event(&mut self, event: BlockEvent<'_>) {
        match event {
            BlockEvent::NoteOn { key, velocity, .. } if velocity > 0.0 => {
                self.gate = true;
                self.active_key = Some(key);
            }
            BlockEvent::NoteOn { .. } | BlockEvent::NoteOff { .. } => {
                self.gate = false;
            }
            BlockEvent::Midi { .. } | BlockEvent::MidiLong { .. } | BlockEvent::ParamSet { .. } => {
            }
        }
    }

    fn next_sample(&mut self) -> f32 {
        let sequence = if self.profile == System55PatchProfile::SequencerDriven {
            let pulse = self.clock_frame.is_multiple_of(16);
            Some(
                self.sequencer
                    .next_frame(GateConvention::s_trigger().native_voltage(pulse), 5.0),
            )
        } else {
            None
        };
        let keyboard = self.keyboard.next_frame(self.active_key, self.gate, 0.0);
        let pitch_cv = sequence.map(|frame| frame.cv).unwrap_or(keyboard.pitch_cv);
        let s_trigger_v = sequence
            .map(|frame| GateConvention::s_trigger().native_voltage(frame.gate))
            .unwrap_or(keyboard.s_trigger_v);
        let driver = self.driver.next_frame(pitch_cv, 0.0, false);
        let oscillator_stack = self.mixer.next_sample([
            self.vco_1.next_sample(driver.pitch_cv_v, 0.0, 0.0, false),
            self.vco_2.next_sample(driver.pitch_cv_v, 0.0, 0.0, false),
            self.vco_3.next_sample(driver.pitch_cv_v, 0.0, 0.0, false),
            0.0,
        ]);
        let envelope = self.envelope.next_sample(s_trigger_v);
        let shaped = match self.profile {
            System55PatchProfile::OscillatorStack => oscillator_stack,
            System55PatchProfile::LadderSelfOscillation => self.ladder.next_sample(0.0, 0.0, 0.85),
            System55PatchProfile::FilterBank => {
                self.filter_bank.next_frame(oscillator_stack).output
            }
            System55PatchProfile::DefaultVoice
            | System55PatchProfile::SequencerDriven
            | System55PatchProfile::PatchRoundTrip => {
                let filtered = self
                    .ladder
                    .next_sample(oscillator_stack, envelope * 0.35, 0.0);
                self.vca.next_sample(filtered, envelope)
            }
        };
        let output = match self.render_mode {
            System55RenderMode::Ideal => shaped * 0.97,
            System55RenderMode::Modeled | System55RenderMode::Trace => (shaped * 1.05).tanh(),
        };
        self.last_trace = Some(self.trace_frame(output, pitch_cv, s_trigger_v <= 0.5));
        self.clock_frame = self.clock_frame.saturating_add(1);
        output
    }

    fn trace_frame(&self, output: f32, pitch_cv: f32, gate: bool) -> ComponentTraceFrame {
        ComponentTraceFrame::new(system55_component_id(), self.backend(), self.clock_frame)
            .with_state(
                trace_key("mode"),
                ComponentTraceValue::Text(self.render_mode.as_str().to_owned()),
            )
            .with_state(
                trace_key("profile"),
                ComponentTraceValue::Text(self.profile.as_str().to_owned()),
            )
            .with_state(trace_key("gate"), ComponentTraceValue::Bool(gate))
            .with_state(
                trace_key("pitch-cv"),
                ComponentTraceValue::Float(f64::from(pitch_cv)),
            )
            .with_output(
                trace_key("output"),
                ComponentTraceValue::Float(f64::from(output)),
            )
    }
}

impl Default for System55 {
    fn default() -> Self {
        Self::new(system55_default_patch(), System55RenderMode::Modeled)
    }
}

impl Processor for System55 {
    fn prepare(&mut self, cfg: PrepareConfig) {
        let config: ComponentPrepareConfig = cfg.into();
        self.vco_1.prepare(config);
        self.vco_2.prepare(config);
        self.vco_3.prepare(config);
        self.ladder.prepare(config);
        self.envelope.prepare(config);
        self.filter_bank.prepare(config);
        Processor::reset(self);
    }

    fn reset(&mut self) {
        DiscreteComponent::reset(&mut self.keyboard);
        DiscreteComponent::reset(&mut self.driver);
        DiscreteComponent::reset(&mut self.vco_1);
        DiscreteComponent::reset(&mut self.vco_2);
        DiscreteComponent::reset(&mut self.vco_3);
        DiscreteComponent::reset(&mut self.mixer);
        DiscreteComponent::reset(&mut self.ladder);
        DiscreteComponent::reset(&mut self.envelope);
        DiscreteComponent::reset(&mut self.vca);
        DiscreteComponent::reset(&mut self.filter_bank);
        DiscreteComponent::reset(&mut self.sequencer);
        self.gate = false;
        self.active_key = None;
        self.clock_frame = 0;
        self.last_trace = None;
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for channel in &mut *block.out_audio {
            channel[..frames].fill(0.0);
        }
        for frame in 0..frames {
            for event in block.in_events {
                if event_offset(*event) == frame as u32 {
                    self.handle_event(*event);
                }
            }
            let sample = self.next_sample();
            for channel in &mut *block.out_audio {
                channel[frame] = sample;
            }
        }
    }

    fn tail_frames(&self) -> u64 {
        2_048
    }
}

impl DiscreteComponent for System55 {
    fn component_id(&self) -> Symbol {
        system55_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        match self.render_mode {
            System55RenderMode::Ideal => ComponentBackend::Algorithmic,
            System55RenderMode::Modeled | System55RenderMode::Trace => ComponentBackend::Modeled,
        }
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        system55_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        system55_params()
    }

    fn reset(&mut self) {
        <Self as Processor>::reset(self);
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        <Self as Processor>::prepare(self, config.into());
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        <Self as Processor>::process(self, block);
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(system55_component_id(), self.backend(), true)
            .with_field(
                Symbol::qualified("audio-synth/system55-inspect", "patch"),
                self.patch.name.as_qualified_str(),
            )
            .with_field(
                Symbol::qualified("audio-synth/system55-inspect", "mode"),
                self.render_mode.as_str(),
            )
            .with_field(
                Symbol::qualified("audio-synth/system55-inspect", "profile"),
                self.profile.as_str(),
            )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Builds a single-node audio graph hosting a System 55 voice for the given patch
/// and render mode.
pub fn system55_audio_graph(
    patch: InstrumentPatch,
    render_mode: System55RenderMode,
) -> Result<AudioGraph> {
    let mut graph = AudioGraph::new();
    graph.add_node(
        "system55",
        Box::new(System55::new(patch, render_mode)),
        0,
        1,
    )?;
    Ok(graph)
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/system55-trace", name)
}

fn event_offset(event: BlockEvent<'_>) -> u32 {
    match event {
        BlockEvent::Midi { offset, .. }
        | BlockEvent::MidiLong { offset, .. }
        | BlockEvent::ParamSet { offset, .. }
        | BlockEvent::NoteOn { offset, .. }
        | BlockEvent::NoteOff { offset, .. } => offset,
    }
}
