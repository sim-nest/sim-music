use std::collections::BTreeSet;

use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::{
    BlockEvent, Graph as AudioGraph, PrepareConfig, ProcessBlock, Processor,
};

use crate::ps3300::{
    Ps3300ExternalProcessor, Ps3300KeyboardController, Ps3300ModulationGenerator, Ps3300PinMatrix,
    Ps3300PinMatrixFrame, Ps3300PinMatrixInputs, Ps3300SampleHold, Ps3300Section,
    Ps3300SectionGenerator, Ps3300SectionGeneratorSettings, Ps3300ThreeSectionSummer,
    Ps3300TripleResonator, ps3300_component_id,
};
use crate::ps3300_patch::{
    Ps3300PatchProfile, Ps3300RenderMode, ps3300_default_patch, ps3300_default_patch_id,
    ps3300_params, ps3300_ports, ps3300_required_module_ids,
};
use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentPortDescriptor,
    ComponentPrepareConfig, ComponentRegistry, ComponentTraceFrame, ComponentTraceValue,
    DiscreteComponent, InstrumentPatch,
};

/// Korg PS-3300 instrument: a complete polyphonic synthesizer processor that
/// wires the keyboard, pin matrix, modulation, three sections, resonator, and
/// output mixer together and renders audio under a chosen render mode.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300 {
    patch: InstrumentPatch,
    profile: Ps3300PatchProfile,
    render_mode: Ps3300RenderMode,
    keyboard: Ps3300KeyboardController,
    matrix: Ps3300PinMatrix,
    modulation: Ps3300ModulationGenerator,
    sample_hold: Ps3300SampleHold,
    external: Ps3300ExternalProcessor,
    sections: [Ps3300SectionGenerator; 3],
    resonator: Ps3300TripleResonator,
    summer: Ps3300ThreeSectionSummer,
    active_keys: BTreeSet<u8>,
    clock_frame: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300 {
    /// Builds an instrument from a patch and render mode, deriving the patch profile.
    pub fn new(patch: InstrumentPatch, render_mode: Ps3300RenderMode) -> Self {
        Self {
            profile: Ps3300PatchProfile::from_patch(&patch),
            patch,
            render_mode,
            keyboard: Ps3300KeyboardController::default(),
            matrix: Ps3300PinMatrix::default(),
            modulation: Ps3300ModulationGenerator::default(),
            sample_hold: Ps3300SampleHold::default(),
            external: Ps3300ExternalProcessor::default(),
            sections: [
                section_generator(Ps3300Section::A, 0.82),
                section_generator(Ps3300Section::B, 0.76),
                section_generator(Ps3300Section::C, 0.7),
            ],
            resonator: Ps3300TripleResonator::default(),
            summer: Ps3300ThreeSectionSummer::default(),
            active_keys: BTreeSet::new(),
            clock_frame: 0,
            last_trace: None,
        }
    }

    /// Builds an instrument after verifying every required module is implemented
    /// in `registry`, returning an error if any is missing or unimplemented.
    pub fn from_registry(
        registry: &ComponentRegistry,
        patch: InstrumentPatch,
        render_mode: Ps3300RenderMode,
    ) -> Result<Self> {
        for id in ps3300_required_module_ids() {
            let entry = registry.get(&id).ok_or_else(|| {
                Error::Eval(format!(
                    "missing PS-3300 registry module: {}",
                    id.as_qualified_str()
                ))
            })?;
            if !entry.is_implemented() {
                return Err(Error::Eval(format!(
                    "PS-3300 registry module is not implemented: {}",
                    id.as_qualified_str()
                )));
            }
        }
        Ok(Self::new(patch, render_mode))
    }

    /// Returns the patch this instrument was built from.
    pub fn patch(&self) -> &InstrumentPatch {
        &self.patch
    }

    /// Returns the active render mode.
    pub fn render_mode(&self) -> Ps3300RenderMode {
        self.render_mode
    }

    /// Returns the patch id of the default PS-3300 patch.
    pub fn default_patch_id() -> Symbol {
        ps3300_default_patch_id()
    }

    fn handle_event(&mut self, event: BlockEvent<'_>) {
        match event {
            BlockEvent::NoteOn { key, velocity, .. } if velocity > 0.0 => {
                self.active_keys.insert(key);
            }
            BlockEvent::NoteOn { key, .. } | BlockEvent::NoteOff { key, .. } => {
                self.active_keys.remove(&key);
            }
            BlockEvent::Midi { .. } | BlockEvent::MidiLong { .. } | BlockEvent::ParamSet { .. } => {
            }
        }
    }

    fn next_sample(&mut self) -> f32 {
        let keys = self.active_keys_for_render();
        let keyboard = self.keyboard.next_chord(&keys);
        let modulation = self.modulation.next_sample(0.0);
        let external = self.external.next_sample(0.0, modulation.unipolar);
        let control = self.matrix.route(Ps3300PinMatrixInputs {
            keyboard_pitch_cv: keyboard.pitch_cv,
            keyboard_gate: keyboard.gate,
            modulation_cv: modulation.bipolar,
            sample_hold_cv: 0.0,
            external_cv: external.cv,
            external_gate: external.gate,
            ..Ps3300PinMatrixInputs::default()
        });
        let held = self.sample_hold.next_sample(
            control.sample_hold_signal,
            if control.sample_hold_trigger {
                1.0
            } else {
                0.0
            },
        );
        let section_outputs = self.section_outputs(&keys, keyboard.gate, &control, held.held);
        let audio_matrix = self.matrix.route(Ps3300PinMatrixInputs {
            keyboard_pitch_cv: keyboard.pitch_cv,
            keyboard_gate: keyboard.gate,
            modulation_cv: modulation.bipolar,
            sample_hold_cv: held.held,
            external_cv: external.cv,
            external_gate: external.gate,
            section_audio: section_outputs,
            ..Ps3300PinMatrixInputs::default()
        });
        let resonator = self.resonator.next_sample(
            self.profile_resonator_input(audio_matrix.resonator_audio, section_outputs),
            self.profile_formant_cv(audio_matrix.resonator_formant_cv, modulation.bipolar),
        );
        let output_matrix = self.matrix.route(Ps3300PinMatrixInputs {
            resonator_audio: resonator.output,
            section_audio: section_outputs,
            ..Ps3300PinMatrixInputs::default()
        });
        let summer = self.summer.next_sample(
            self.profile_sections(section_outputs),
            resonator.output + output_matrix.output_audio,
        );
        let sample = self.render_mode_sample(summer.output);
        self.last_trace = Some(self.trace_frame(sample, keys.len(), keyboard.gate));
        self.clock_frame = self.clock_frame.saturating_add(1);
        sample
    }

    fn active_keys_for_render(&self) -> Vec<u8> {
        let keys = self.active_keys.iter().copied().collect::<Vec<_>>();
        if !keys.is_empty() {
            return keys;
        }
        match self.profile {
            Ps3300PatchProfile::OneCell => vec![60],
            Ps3300PatchProfile::OneSectionChord => vec![48, 52, 55],
            Ps3300PatchProfile::ResonatorSweep => vec![52],
            Ps3300PatchProfile::ThreeSectionStack
            | Ps3300PatchProfile::DefaultPolyphonic
            | Ps3300PatchProfile::PatchRoundTrip => vec![48, 55, 60],
        }
    }

    fn section_outputs(
        &mut self,
        keys: &[u8],
        gate: bool,
        control: &Ps3300PinMatrixFrame,
        held_cv: f32,
    ) -> [f32; 3] {
        let active = active_keys_for_profile(self.profile, keys);
        let mut outputs = [0.0; 3];
        for (index, section) in self.sections.iter_mut().enumerate() {
            let frame = section.next_chord(
                active,
                control.section_pitch_cv[index],
                control.section_gate[index] || gate,
                held_cv * 0.05,
            );
            outputs[index] = frame.output;
        }
        outputs
    }

    fn profile_sections(&self, sections: [f32; 3]) -> [f32; 3] {
        match self.profile {
            Ps3300PatchProfile::OneCell | Ps3300PatchProfile::OneSectionChord => {
                [sections[0], 0.0, 0.0]
            }
            _ => sections,
        }
    }

    fn profile_resonator_input(&self, routed: f32, sections: [f32; 3]) -> f32 {
        match self.profile {
            Ps3300PatchProfile::ResonatorSweep if routed.abs() <= f32::EPSILON => sections[0],
            _ => routed,
        }
    }

    fn profile_formant_cv(&self, routed: f32, modulation: f32) -> f32 {
        match self.profile {
            Ps3300PatchProfile::ResonatorSweep => routed + modulation * 0.25,
            _ => routed,
        }
    }

    fn render_mode_sample(&self, sample: f32) -> f32 {
        match self.render_mode {
            Ps3300RenderMode::Ideal => (sample * 0.98).clamp(-1.0, 1.0),
            Ps3300RenderMode::Modeled | Ps3300RenderMode::Trace => (sample * 1.04).tanh(),
        }
    }

    fn trace_frame(&self, output: f32, active_count: usize, gate: bool) -> ComponentTraceFrame {
        ComponentTraceFrame::new(ps3300_component_id(), self.backend(), self.clock_frame)
            .with_state(
                trace_key("mode"),
                ComponentTraceValue::Text(self.render_mode.as_str().to_owned()),
            )
            .with_state(
                trace_key("profile"),
                ComponentTraceValue::Text(self.profile.as_str().to_owned()),
            )
            .with_state(
                trace_key("active-count"),
                ComponentTraceValue::Float(active_count as f64),
            )
            .with_state(trace_key("gate"), ComponentTraceValue::Bool(gate))
            .with_output(
                trace_key("output"),
                ComponentTraceValue::Float(f64::from(output)),
            )
    }
}

impl Default for Ps3300 {
    fn default() -> Self {
        Self::new(ps3300_default_patch(), Ps3300RenderMode::Modeled)
    }
}

impl Processor for Ps3300 {
    fn prepare(&mut self, cfg: PrepareConfig) {
        let config: ComponentPrepareConfig = cfg.into();
        DiscreteComponent::prepare(&mut self.keyboard, config);
        DiscreteComponent::prepare(&mut self.matrix, config);
        DiscreteComponent::prepare(&mut self.modulation, config);
        DiscreteComponent::prepare(&mut self.sample_hold, config);
        DiscreteComponent::prepare(&mut self.external, config);
        for section in &mut self.sections {
            DiscreteComponent::prepare(section, config);
        }
        DiscreteComponent::prepare(&mut self.resonator, config);
        DiscreteComponent::prepare(&mut self.summer, config);
        Processor::reset(self);
    }

    fn reset(&mut self) {
        DiscreteComponent::reset(&mut self.keyboard);
        DiscreteComponent::reset(&mut self.matrix);
        DiscreteComponent::reset(&mut self.modulation);
        DiscreteComponent::reset(&mut self.sample_hold);
        DiscreteComponent::reset(&mut self.external);
        for section in &mut self.sections {
            DiscreteComponent::reset(section);
        }
        DiscreteComponent::reset(&mut self.resonator);
        DiscreteComponent::reset(&mut self.summer);
        self.active_keys.clear();
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
        4_096
    }
}

impl DiscreteComponent for Ps3300 {
    fn component_id(&self) -> Symbol {
        ps3300_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        match self.render_mode {
            Ps3300RenderMode::Ideal => ComponentBackend::Algorithmic,
            Ps3300RenderMode::Modeled | Ps3300RenderMode::Trace => ComponentBackend::Modeled,
        }
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3300_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3300_params()
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
        ComponentInspection::new(ps3300_component_id(), self.backend(), true)
            .with_field(
                Symbol::qualified("audio-synth/ps3300-inspect", "patch"),
                self.patch.name.as_qualified_str(),
            )
            .with_field(
                Symbol::qualified("audio-synth/ps3300-inspect", "mode"),
                self.render_mode.as_str(),
            )
            .with_field(
                Symbol::qualified("audio-synth/ps3300-inspect", "profile"),
                self.profile.as_str(),
            )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Builds a single-node audio graph hosting a [`Ps3300`] instrument.
pub fn ps3300_audio_graph(
    patch: InstrumentPatch,
    render_mode: Ps3300RenderMode,
) -> Result<AudioGraph> {
    let mut graph = AudioGraph::new();
    graph.add_node("ps3300", Box::new(Ps3300::new(patch, render_mode)), 0, 1)?;
    Ok(graph)
}

fn active_keys_for_profile(profile: Ps3300PatchProfile, keys: &[u8]) -> &[u8] {
    match profile {
        Ps3300PatchProfile::OneCell if !keys.is_empty() => &keys[..1],
        _ => keys,
    }
}

fn section_generator(section: Ps3300Section, level: f32) -> Ps3300SectionGenerator {
    Ps3300SectionGenerator::new(Ps3300SectionGeneratorSettings {
        section,
        level,
        ..Ps3300SectionGeneratorSettings::default()
    })
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300-trace", name)
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
