use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::{
    BlockEvent, Graph as AudioGraph, PrepareConfig, ProcessBlock, Processor,
};

use crate::system700::{
    System700Clock, System700ClockSettings, System700Envelope, System700EnvelopeSettings,
    System700PatchProfile, System700RenderMode, System700Sequencer, System700SequencerSettings,
    System700Vca, System700VcaResponse, System700VcaSettings, System700Vcf, System700VcfMode,
    System700VcfSettings, System700Vco, System700VcoSettings, System700VcoWaveform,
    system700_component_id, system700_default_patch, system700_params, system700_ports,
    system700_required_module_ids,
};
use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentPortDescriptor,
    ComponentPrepareConfig, ComponentRegistry, ComponentTraceFrame, ComponentTraceValue,
    DiscreteComponent, InstrumentPatch,
};

/// A playable System 700 instrument: the patch profile, render mode, and the
/// fixed VCO/VCF/VCA/envelope/clock/sequencer voice chain that renders it.
#[derive(Clone, Debug, PartialEq)]
pub struct System700 {
    patch: InstrumentPatch,
    profile: System700PatchProfile,
    render_mode: System700RenderMode,
    vco: System700Vco,
    vcf: System700Vcf,
    vca: System700Vca,
    envelope: System700Envelope,
    clock: System700Clock,
    sequencer: System700Sequencer,
    gate: bool,
    pitch_cv: f32,
    clock_frame: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700 {
    /// Builds an instrument from `patch` and `render_mode`, deriving the patch
    /// profile and seeding the voice chain with the default module settings.
    pub fn new(patch: InstrumentPatch, render_mode: System700RenderMode) -> Self {
        Self {
            profile: System700PatchProfile::from_patch(&patch),
            patch,
            render_mode,
            vco: System700Vco::new(System700VcoSettings {
                waveform: System700VcoWaveform::Saw,
                base_frequency_hz: 110.0,
                level: 0.7,
                ..System700VcoSettings::default()
            }),
            vcf: System700Vcf::new(System700VcfSettings {
                mode: System700VcfMode::LowPass,
                cutoff_hz: 1_600.0,
                resonance: 0.35,
                level: 1.0,
                ..System700VcfSettings::default()
            }),
            vca: System700Vca::new(System700VcaSettings {
                response: System700VcaResponse::Saturated,
                gain: 0.9,
                saturation_drive: 2.0,
            }),
            envelope: System700Envelope::new(System700EnvelopeSettings {
                attack_s: 0.0,
                decay_s: 0.03,
                sustain_level: 0.85,
                release_s: 0.04,
                level: 1.0,
            }),
            clock: System700Clock::new(System700ClockSettings {
                rate_hz: 1_000.0,
                pulse_width: 0.5,
            }),
            sequencer: System700Sequencer::new(System700SequencerSettings {
                steps: [0.0, 0.0833, 0.1667, 0.25, 0.3333, 0.25, 0.1667, 0.0833],
                step_count: 8,
                gate_mask: 0xff,
            }),
            gate: false,
            pitch_cv: 0.0,
            clock_frame: 0,
            last_trace: None,
        }
    }

    /// Builds an instrument after verifying that every required module is
    /// present and implemented in `registry`, returning an error otherwise.
    pub fn from_registry(
        registry: &ComponentRegistry,
        patch: InstrumentPatch,
        render_mode: System700RenderMode,
    ) -> Result<Self> {
        for id in system700_required_module_ids() {
            let entry = registry.get(&id).ok_or_else(|| {
                Error::Eval(format!(
                    "missing System 700 registry module: {}",
                    id.as_qualified_str()
                ))
            })?;
            if !entry.is_implemented() {
                return Err(Error::Eval(format!(
                    "System 700 registry module is not implemented: {}",
                    id.as_qualified_str()
                )));
            }
        }
        Ok(Self::new(patch, render_mode))
    }

    /// Returns the instrument's patch.
    pub fn patch(&self) -> &InstrumentPatch {
        &self.patch
    }

    /// Returns the instrument's render mode.
    pub fn render_mode(&self) -> System700RenderMode {
        self.render_mode
    }

    /// Returns the patch id symbol for the System 700 default patch.
    pub fn default_patch_id() -> Symbol {
        crate::system700::system700_default_patch_id()
    }

    fn handle_event(&mut self, event: BlockEvent<'_>) {
        match event {
            BlockEvent::NoteOn { key, velocity, .. } if velocity > 0.0 => {
                self.gate = true;
                self.pitch_cv = (f32::from(key) - 60.0) / 12.0;
            }
            BlockEvent::NoteOn { .. } | BlockEvent::NoteOff { .. } => {
                self.gate = false;
            }
            BlockEvent::Midi { .. } | BlockEvent::MidiLong { .. } | BlockEvent::ParamSet { .. } => {
            }
        }
    }

    fn next_sample(&mut self) -> f32 {
        let sequence = if self.profile == System700PatchProfile::SequencerDriven {
            let clock = self.clock.next_frame(true);
            let sequencer = self.sequencer.next_frame(clock.trigger, false);
            Some(sequencer)
        } else {
            None
        };

        let gate = sequence.map(|frame| frame.gate).unwrap_or(self.gate);
        let pitch_cv = sequence.map(|frame| frame.cv).unwrap_or(self.pitch_cv);
        let envelope = self.envelope.next_sample(gate);
        let source = self.vco.next_sample(pitch_cv, 0.0, 0.0, false);
        let shaped = match self.profile {
            System700PatchProfile::SingleModule => source,
            System700PatchProfile::TwoModule => self.vcf.next_sample(source, 0.0),
            System700PatchProfile::DefaultVoice
            | System700PatchProfile::SequencerDriven
            | System700PatchProfile::PatchRoundTrip => {
                let filtered = self.vcf.next_sample(source, envelope * 0.5);
                self.vca.next_sample(filtered, envelope)
            }
        };
        let output = match self.render_mode {
            System700RenderMode::Ideal => shaped * 0.98,
            System700RenderMode::Modeled | System700RenderMode::Trace => (shaped * 1.04).tanh(),
        };
        self.last_trace = Some(self.trace_frame(output, gate, pitch_cv));
        self.clock_frame = self.clock_frame.saturating_add(1);
        output
    }

    fn trace_frame(&self, output: f32, gate: bool, pitch_cv: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(system700_component_id(), self.backend(), self.clock_frame)
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

impl Default for System700 {
    fn default() -> Self {
        Self::new(system700_default_patch(), System700RenderMode::Modeled)
    }
}

impl Processor for System700 {
    fn prepare(&mut self, cfg: PrepareConfig) {
        let config: ComponentPrepareConfig = cfg.into();
        self.vco.prepare(config);
        self.vcf.prepare(config);
        self.vca.prepare(config);
        self.envelope.prepare(config);
        self.clock.prepare(config);
        self.sequencer.prepare(config);
        Processor::reset(self);
    }

    fn reset(&mut self) {
        DiscreteComponent::reset(&mut self.vco);
        DiscreteComponent::reset(&mut self.vcf);
        DiscreteComponent::reset(&mut self.vca);
        DiscreteComponent::reset(&mut self.envelope);
        DiscreteComponent::reset(&mut self.clock);
        DiscreteComponent::reset(&mut self.sequencer);
        self.gate = false;
        self.pitch_cv = 0.0;
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

impl DiscreteComponent for System700 {
    fn component_id(&self) -> Symbol {
        system700_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        match self.render_mode {
            System700RenderMode::Ideal => ComponentBackend::Algorithmic,
            System700RenderMode::Modeled | System700RenderMode::Trace => ComponentBackend::Modeled,
        }
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        system700_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        system700_params()
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
        ComponentInspection::new(system700_component_id(), self.backend(), true)
            .with_field(
                Symbol::qualified("audio-synth/system700-inspect", "patch"),
                self.patch.name.as_qualified_str(),
            )
            .with_field(
                Symbol::qualified("audio-synth/system700-inspect", "mode"),
                self.render_mode.as_str(),
            )
            .with_field(
                Symbol::qualified("audio-synth/system700-inspect", "profile"),
                self.profile.as_str(),
            )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Builds a single-node audio graph wrapping a [`System700`] for `patch` and
/// `render_mode`.
pub fn system700_audio_graph(
    patch: InstrumentPatch,
    render_mode: System700RenderMode,
) -> Result<AudioGraph> {
    let mut graph = AudioGraph::new();
    graph.add_node(
        "system700",
        Box::new(System700::new(patch, render_mode)),
        0,
        1,
    )?;
    Ok(graph)
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/system700-trace", name)
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
