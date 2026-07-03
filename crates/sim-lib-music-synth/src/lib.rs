#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Playable pure-Rust software synthesizer primitives for the SIM audio graph.
//!
//! This crate implements audio-synthesis voices and the discrete components
//! they are built from: oscillators, filters, envelopes, LFOs, modulation
//! routing, control-voltage conventions, and polyphonic voice allocation. On
//! top of these primitives it models several classic instruments -- a Yamaha
//! DX7-style FM engine, a Korg PS-3300-style synthesizer, and the System 55 and
//! System 700 modular systems -- together with their patches, render fixtures,
//! and a component [`ComponentRegistry`] that exposes them to the runtime via
//! [`AudioSynthLib`]. Components plug into the shared SIM audio graph; the
//! `system55`, `system700`, `ps3300`, and `daw` modules are the public
//! instrument and arrangement surfaces.
//!
//! # Examples
//!
//! MIDI key 69 (A4) maps to 440 Hz under equal temperament:
//!
//! ```
//! use sim_lib_music_synth::midi_key_to_hz;
//!
//! assert!((midi_key_to_hz(69) - 440.0).abs() < 1e-3);
//! ```

mod algorithm;
mod backend;
mod builder;
mod citizen;
mod component;
mod cv;
mod dac_float;
/// DAW-style arrangement and transport surface built on the synth components.
pub mod daw;
mod dsp_fixed;
mod dx7;
mod dx7_envelope;
mod dx7_fixture;
mod dx7_inspection;
mod dx7_lfo;
mod dx7_operator;
mod dx7_patch;
mod dx7_pitch;
mod dx7_scaling;
mod dx7_velocity;
mod editor;
mod egs;
mod envelope;
mod fixture;
mod graph_host;
mod lfo;
mod lut;
mod modeled;
mod modulation;
mod modulator;
mod modules;
mod ops;
mod oscillator;
mod param;
mod patch;
mod poly;
mod port;
mod preset;
mod processor;
/// Korg PS-3300-style synthesizer model and its building blocks.
pub mod ps3300;
mod ps3300_fixture;
mod ps3300_patch;
mod ps3300_wrapper;
mod registry;
mod registry_ps3300;
mod registry_system55;
mod runtime;
/// Roland System 55-style modular synthesizer model and modules.
pub mod system55;
mod system55_fixture;
mod system55_patch;
mod system55_wrapper;
/// Roland System 700-style modular synthesizer model and modules.
pub mod system700;
mod system700_fixture;
mod system700_wrapper;
mod trace;
mod voice;

pub use algorithm::{
    DX7_ALGORITHM_COUNT, DX7_ALGORITHM_TOPOLOGIES, DX7_GAIN_UNITY, DX7_OPERATOR_COUNT,
    Dx7AlgorithmEdge, Dx7AlgorithmGainPoint, Dx7AlgorithmTopology, Dx7CarrierOutput,
    dx7_algorithm_topology, dx7_algorithm_topology_for_patch, dx7_patch_algorithm_id,
};
pub use backend::{ComponentBackend, ComponentBackendSurface, assert_backend_surface_identity};
pub use citizen::{SynthPresetDescriptor, synth_preset_class_symbol};
pub use component::{
    ComponentDescriptor, ComponentInspection, ComponentPrepareConfig, ComponentTick,
    ComponentTickResult, DiscreteComponent,
};
pub use cv::{
    ControlVoltage, CvConvention, CvPolarity, GateConvention, GateConverter, GateFrame, GateMode,
    VoltsPerOctave,
};
pub use dac_float::{
    DX7_DAC_HELD_SAMPLE_BITS, DX7_DAC_INPUT_BITS, Dx7DacWordWidths, Dx7FloatingDac,
};
pub use dsp_fixed::{FixedFormat, FixedRounding, QLevel, QPhase};
pub use dx7::{
    Dx7Voice, Dx7VoiceControl, dx7_voice_audio_graph, dx7_voice_params, dx7_voice_ports,
};
pub use dx7_envelope::{Dx7EnvelopeGenerator, Dx7EnvelopeSettings, Dx7EnvelopeStage};
pub use dx7_fixture::{
    DX7_FIXTURE_REGENERATE_COMMAND, DX7_RENDER_FIXTURE_MANIFEST_PATH, Dx7RenderFixture,
    Dx7RenderFixtureKind, Dx7RenderFixtureMetadata, Dx7RenderTolerance, Dx7RenderToleranceReport,
    Dx7RendererAccuracyGate, Dx7RendererAccuracyStatus, Dx7SyntheticBankFixture,
    dx7_accurate_renderer_gate, dx7_compatible_renderer_tolerance_report,
    dx7_fixture_regeneration_command, dx7_render_fixture_ids, dx7_render_fixture_manifest,
    dx7_render_fixtures, dx7_synthetic_bank_fixture, render_dx7_fixture,
};
pub use dx7_inspection::{Dx7GraphEdgeInspection, Dx7GraphInspection, Dx7GraphNodeInspection};
pub use dx7_lfo::{Dx7AlgorithmicLfo, Dx7AlgorithmicLfoSettings, Dx7LfoFrame};
pub use dx7_operator::{
    DX7_OPERATOR_TRACE_FIXTURES, Dx7FmOperator, Dx7FmOperatorSettings, Dx7OperatorInput,
    Dx7OperatorOutput, dx7_operator_component_id, dx7_operator_params, dx7_operator_ports,
    dx7_operator_trace_fixture_names,
};
pub use dx7_patch::{
    Dx7Envelope, Dx7Lfo, Dx7Patch, Dx7PatchOperator, Dx7RawPatch, dx7_patch_component_kind,
};
pub use dx7_pitch::{
    Dx7FrequencyMode, Dx7PitchEnvelope, Dx7PitchEnvelopeSettings, Dx7PitchSettings,
};
pub use dx7_scaling::Dx7KeyboardScaling;
pub use dx7_velocity::Dx7VelocitySensitivity;
pub use editor::*;
pub use egs::{
    DX7_EGS_LEVEL_BITS, DX7_EGS_PITCH_BITS, DX7_EGS_RATE_BITS, Dx7EgsEnvelope, Dx7EgsPitch,
    Dx7EgsStage, Dx7EgsWordWidths, modeled_pitch_hz,
};
pub use envelope::{AdsrEnvelope, AdsrSettings, AdsrStage};
pub use fixture::{SynthOfflineFixture, r31_synth_note_fixture, render_synth_offline};
pub use graph_host::{
    ComponentGraphCable, ComponentGraphEndpoint, DiscreteComponentGraph,
    discrete_component_graph_id, discrete_component_graph_ports,
};
pub use lfo::{Lfo, LfoSettings, TempoSync};
pub use lut::{GeneratedLut, GeneratedLutKind};
pub use modeled::{
    DX7_MODELED_TRACE_FIXTURES, Dx7ModeledDivergenceReport, Dx7ModeledOperator,
    Dx7ModeledOperatorOutput, dx7_modeled_divergence_report, dx7_modeled_operator_component_id,
    dx7_modeled_trace_fixture_names, dx7_operator_backend_surfaces,
};
pub use modulation::{
    ModSource, ModTarget, ModulationInput, ModulationMatrix, ModulationRoute, SynthModulation,
};
pub use modulator::*;
pub use ops::{
    DX7_OPS_LOG_BITS, DX7_OPS_OUTPUT_BITS, DX7_OPS_PHASE_BITS, DX7_OPS_TABLE_BITS,
    DX7_OPS_TABLE_LEN, Dx7OpsDatapath, Dx7OpsInput, Dx7OpsOutput, Dx7OpsWordWidths,
    cascade_accumulate, exp_output_level, log_sine_lookup,
};
pub use oscillator::{Oscillator, OscillatorKind, PhaseOscillator};
pub use param::{ComponentParamDescriptor, ComponentParamRange, ComponentParamUnit};
pub use patch::{
    InstrumentPatch, PatchCord, PatchEndpoint, PatchJack, PatchModule, PatchRawView, PatchSetting,
    subtractive_synth_algorithm_patch,
};
pub use poly::{
    PerKeyGateBus, PerKeyGateInput, PolyKeySignal, PolyphonicArray, PolyphonicSectionSetting,
};
pub use port::{
    ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    component_port_media_symbols,
};
pub use preset::SynthPreset;
pub use processor::{
    SubtractiveSynth, subtractive_synth_backend_surfaces, subtractive_synth_component_id,
    subtractive_synth_params, subtractive_synth_ports,
};
pub use ps3300_fixture::*;
pub use ps3300_patch::*;
pub use ps3300_wrapper::*;
pub use registry::{
    ComponentCapability, ComponentInventory, ComponentInventoryItem, ComponentRegistry,
    ComponentRegistryCategory, ComponentRegistryEntry, InstrumentWrapperCategory,
    component_graph_registry_entry, default_audio_synth_registry, dx7_component_id,
    dx7_modeled_operator_registry_entry, dx7_operator_registry_entry, dx7_registry_entry,
    ps_3300_component_id, subtractive_synth_registry_entry, system_55_component_id,
    system_700_component_id,
};
pub use runtime::{
    AudioSynthLib, audio_synth_stream_profile_symbol, audio_synth_symbols, install_audio_synth_lib,
};
pub use sim_lib_audio_graph_core::{ClockDomain, LatencyClass, RateContract};
pub use system55::{
    SYSTEM55_CONTROL_FIXTURE_NAMES, SYSTEM55_FIXED_FILTER_BAND_COUNT,
    SYSTEM55_FIXED_FILTER_BANK_CENTERS_HZ, System55Attenuator, System55AttenuatorSettings,
    System55Envelope, System55EnvelopeFollower, System55EnvelopeFollowerFrame,
    System55EnvelopeFollowerSettings, System55EnvelopeSettings, System55EnvelopeStage,
    System55FixedFilterBank, System55FixedFilterBankFrame, System55FixedFilterBankSettings,
    System55FrequencyShifter, System55FrequencyShifterFrame, System55FrequencyShifterSettings,
    System55Interface, System55Keyboard, System55KeyboardFrame, System55KeyboardSettings,
    System55Mixer, System55MixerSettings, System55Multiple, System55MultipleFrame,
    System55MultipleSettings, System55Ribbon, System55RibbonFrame, System55RibbonSettings,
    System55RingModulator, System55RingModulatorSettings, System55SampleHold,
    System55SampleHoldSettings, System55Sequencer, System55SequencerFrame,
    System55SequencerSettings, System55TriggerDelay, System55TriggerDelayFrame,
    System55TriggerDelaySettings, System55Vca, System55VcaResponse, System55VcaSettings,
    m55_attenuator_component_id, m55_attenuator_params, m55_attenuator_ports,
    m55_env_follower_component_id, m55_env_follower_params, m55_env_follower_ports,
    m55_envelope_component_id, m55_envelope_params, m55_envelope_ports,
    m55_fixed_filter_bank_component_id, m55_fixed_filter_bank_params, m55_fixed_filter_bank_ports,
    m55_frequency_shifter_component_id, m55_frequency_shifter_params, m55_frequency_shifter_ports,
    m55_interface_component_id, m55_interface_params, m55_interface_ports,
    m55_keyboard_component_id, m55_keyboard_params, m55_keyboard_ports, m55_mixer_component_id,
    m55_mixer_params, m55_mixer_ports, m55_multiple_component_id, m55_multiple_params,
    m55_multiple_ports, m55_ribbon_component_id, m55_ribbon_params, m55_ribbon_ports,
    m55_ring_component_id, m55_ring_params, m55_ring_ports, m55_sample_hold_component_id,
    m55_sample_hold_params, m55_sample_hold_ports, m55_sequencer_component_id,
    m55_sequencer_params, m55_sequencer_ports, m55_trigger_delay_component_id,
    m55_trigger_delay_params, m55_trigger_delay_ports, m55_vca_component_id, m55_vca_params,
    m55_vca_ports, system55_control_fixture_names, system55_control_module_ids,
};
pub use system55::{
    SYSTEM55_FILTER_FIXTURE_NAMES, SYSTEM55_FILTER_MODEL_NOTES, SYSTEM55_OSCILLATOR_FIXTURE_NAMES,
    SYSTEM55_RECIPE_BOOK_PATH, SYSTEM55_RECIPE_CHAPTER_PATH, System55FilterCoupler,
    System55FilterCouplerSettings, System55HighPassFilter, System55HighPassFilterSettings,
    System55LadderLpf, System55LadderLpfSettings, System55ModuleDescriptor, System55ModuleRole,
    System55Noise, System55NoiseColor, System55NoiseFrame, System55NoiseSettings,
    System55STriggerFitEvidence, System55Vco, System55VcoDriver, System55VcoDriverFrame,
    System55VcoDriverSettings, System55VcoSettings, System55VcoWaveform, m55_coupler_component_id,
    m55_coupler_params, m55_coupler_ports, m55_hpf_component_id, m55_hpf_params, m55_hpf_ports,
    m55_ladder_lpf_component_id, m55_ladder_lpf_params, m55_ladder_lpf_ports,
    m55_noise_component_id, m55_noise_params, m55_noise_ports, m55_vco_component_id,
    m55_vco_driver_component_id, m55_vco_driver_params, m55_vco_driver_ports, m55_vco_params,
    m55_vco_ports, system55_filter_fixture_names, system55_filter_model_notes,
    system55_filter_module_ids, system55_gate_mode_symbols, system55_module_descriptors,
    system55_module_ids, system55_oscillator_fixture_names, system55_oscillator_module_ids,
    system55_s_trigger_convention, system55_s_trigger_fit_evidence,
    system55_s_trigger_voltage_gate_frames, system55_scaffold_patch, system55_scaffold_patch_id,
};
pub use system55_fixture::{
    SYSTEM55_FIXTURE_REGENERATE_COMMAND, SYSTEM55_RENDER_FIXTURE_IDS,
    SYSTEM55_RENDER_FIXTURE_MANIFEST_PATH, System55RenderFixture, System55RenderFixtureKind,
    System55RenderFixtureMetadata, System55RenderGate, System55RenderTolerance,
    System55RenderToleranceReport, render_system55_fixture, system55_fixture_regeneration_command,
    system55_mode_tolerance_report, system55_render_fixture_ids, system55_render_fixture_manifest,
    system55_render_fixtures, system55_render_gate,
};
pub use system55_patch::{
    SYSTEM55_PATCH_POINTS, SYSTEM55_RECIPE_PATH, SYSTEM55_USER_PATCH_PATH, System55PatchPoint,
    System55PatchProfile, System55RenderMode, system55_component_id, system55_default_patch,
    system55_default_patch_id, system55_filter_bank_patch, system55_ladder_self_oscillation_patch,
    system55_oscillator_stack_patch, system55_params, system55_patch_points,
    system55_patch_round_trip_patch, system55_ports, system55_recipe_path,
    system55_render_mode_symbols, system55_required_module_ids, system55_sequencer_patch,
    system55_user_patch_path,
};
pub use system55_wrapper::{System55, system55_audio_graph};
pub use system700::{
    SYSTEM700_USER_PATCH_PATH, System700, System700RenderMode, system700_audio_graph,
    system700_component_id, system700_default_patch, system700_default_patch_id, system700_params,
    system700_patch_round_trip_patch, system700_ports, system700_render_mode_symbols,
    system700_required_module_ids, system700_sequencer_patch, system700_single_module_patch,
    system700_two_module_patch, system700_user_patch_path,
};
pub use system700_fixture::{
    SYSTEM700_FIXTURE_REGENERATE_COMMAND, SYSTEM700_RECIPE_PATH, SYSTEM700_RENDER_FIXTURE_IDS,
    SYSTEM700_RENDER_FIXTURE_MANIFEST_PATH, System700RenderFixture, System700RenderFixtureKind,
    System700RenderFixtureMetadata, System700RenderGate, System700RenderTolerance,
    System700RenderToleranceReport, render_system700_fixture,
    system700_fixture_regeneration_command, system700_mode_tolerance_report, system700_recipe_path,
    system700_render_fixture_ids, system700_render_fixture_manifest, system700_render_fixtures,
    system700_render_gate,
};
pub use trace::{
    ComponentTraceFrame, ComponentTraceRecord, ComponentTraceRole, ComponentTraceValue,
};
pub use voice::{SynthVoice, VoiceAllocator, VoiceState, midi_key_to_hz};
#[cfg(test)]
mod tests;
