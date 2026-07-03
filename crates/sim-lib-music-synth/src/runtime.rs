use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

const AUDIO_SYNTH_LIB_ID: &str = "audio-synth";

/// Host-registered lib exporting the playable synth cards, built on the shared
/// [`SurfacePackLib`] substrate.
pub struct AudioSynthLib;

impl Lib for AudioSynthLib {
    fn manifest(&self) -> LibManifest {
        audio_synth_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        audio_synth_pack().load(cx, linker)
    }
}

/// Installs the audio synth lib into the runtime context, idempotently.
pub fn install_audio_synth_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &AudioSynthLib)?;
    Ok(())
}

/// Returns every card symbol exported by the audio synth lib.
pub fn audio_synth_symbols() -> Vec<Symbol> {
    [
        "PhaseOscillator",
        "AdsrEnvelope",
        "Lfo",
        "ModulationMatrix",
        "VoiceAllocator",
        "SubtractiveSynth",
        "DiscreteComponentGraph",
        "ComponentGraphCable",
        "ComponentGraphEndpoint",
        "DiscreteComponent",
        "ComponentBackend",
        "ComponentPortDescriptor",
        "ComponentParamDescriptor",
        "ComponentRegistry",
        "ComponentRegistryEntry",
        "ComponentInventory",
        "ComponentInventoryItem",
        "ComponentRegistryCategory",
        "ComponentCapability",
        "InstrumentWrapperCategory",
        "ComponentTraceFrame",
        "ComponentTraceRole",
        "DawInstrumentGraphKind",
        "DawInstrumentGraphNodeDescriptor",
        "InstrumentStreamRecipeSpec",
        "InstrumentPlacementRecipeSpec",
        "CvConvention",
        "CvPolarity",
        "ControlVoltage",
        "VoltsPerOctave",
        "GateConvention",
        "GateMode",
        "GateConverter",
        "GateFrame",
        "PerKeyGateBus",
        "PerKeyGateInput",
        "PolyphonicArray",
        "PolyphonicSectionSetting",
        "PolyKeySignal",
        "FixedFormat",
        "FixedRounding",
        "QPhase",
        "QLevel",
        "GeneratedLut",
        "GeneratedLutKind",
        "InstrumentPatch",
        "PatchModule",
        "PatchJack",
        "PatchCord",
        "PatchRawView",
        "SynthPreset",
        "SynthOfflineFixture",
        "Dx7RenderFixture",
        "Dx7RenderFixtureMetadata",
        "Dx7RenderTolerance",
        "Dx7RenderToleranceReport",
        "Dx7RendererAccuracyGate",
        "Dx7SyntheticBankFixture",
        "System55ModuleDescriptor",
        "System55ModuleRole",
        "System55STriggerFitEvidence",
        "System55LadderLpf",
        "System55LadderLpfSettings",
        "System55HighPassFilter",
        "System55HighPassFilterSettings",
        "System55FilterCoupler",
        "System55FilterCouplerSettings",
        "System55Vca",
        "System55VcaSettings",
        "System55Envelope",
        "System55EnvelopeSettings",
        "System55TriggerDelay",
        "System55TriggerDelaySettings",
        "System55EnvelopeFollower",
        "System55SampleHold",
        "System55Sequencer",
        "System55FixedFilterBank",
        "System55FrequencyShifter",
        "System55RingModulator",
        "System55Mixer",
        "System55Multiple",
        "System55Attenuator",
        "System55Interface",
        "System55Ribbon",
        "System55Keyboard",
        "System55VcoDriver",
        "System55VcoDriverFrame",
        "System55Vco",
        "System55VcoWaveform",
        "System55Noise",
        "System55NoiseColor",
        "System55NoiseFrame",
        "System55",
        "System55RenderMode",
        "System55RenderFixture",
        "System55RenderFixtureMetadata",
        "System55RenderTolerance",
        "System55RenderToleranceReport",
        "System55RenderGate",
        "System700",
        "System700RenderMode",
        "System700RenderFixture",
        "System700RenderFixtureMetadata",
        "System700RenderTolerance",
        "System700RenderToleranceReport",
        "System700RenderGate",
        "Ps3300ModuleDescriptor",
        "Ps3300ModuleRole",
        "Ps3300Section",
        "Ps3300KeyboardAssignment",
        "Ps3300ResonatorSettings",
        "Ps3300PinMatrixRoute",
        "Ps3300ToneSource",
        "Ps3300ToneSourceSettings",
        "Ps3300ToneWaveform",
        "Ps3300Footage",
        "Ps3300AliasingPolicy",
        "Ps3300Noise",
        "Ps3300NoiseColor",
        "Ps3300NoteCell",
        "Ps3300NoteCellSettings",
        "Ps3300PolyArray",
        "Ps3300TripleResonator",
        "Ps3300ResonatorMode",
        "Ps3300ModulationGenerator",
        "Ps3300SampleHold",
        "Ps3300ExternalProcessor",
        "Ps3300KeyboardController",
        "Ps3300PinMatrix",
        "Ps3300SectionGenerator",
        "Ps3300ThreeSectionSummer",
        "Ps3300",
        "Ps3300RenderMode",
        "Ps3300PatchProfile",
        "Ps3300PolyphonySummary",
        "Ps3300RenderFixture",
        "Ps3300RenderFixtureMetadata",
        "Ps3300RenderTolerance",
        "Ps3300RenderToleranceReport",
        "Ps3300RenderGate",
    ]
    .into_iter()
    .map(|name| Symbol::qualified(AUDIO_SYNTH_LIB_ID, name))
    .collect()
}

/// Returns the stream profile symbol for real-time local audio used by the
/// synth's surface cards.
pub fn audio_synth_stream_profile_symbol() -> Symbol {
    Symbol::qualified("stream/profile", "realtime-local-audio")
}

fn audio_synth_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(AUDIO_SYNTH_LIB_ID),
            values: audio_synth_symbols()
                .into_iter()
                .map(|symbol| SurfaceValueSpec {
                    symbol: symbol.clone(),
                    fields: vec![
                        (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
                        (
                            Symbol::new("layer"),
                            SurfaceField::Str(AUDIO_SYNTH_LIB_ID.to_owned()),
                        ),
                        (
                            Symbol::new("kind"),
                            SurfaceField::Str("audio-graph-processor".to_owned()),
                        ),
                        (
                            Symbol::new("role"),
                            SurfaceField::Str("playable pure Rust synth descriptor".to_owned()),
                        ),
                        (
                            Symbol::new("contract"),
                            SurfaceField::Str("Processor".to_owned()),
                        ),
                        (
                            Symbol::new("stream-profile"),
                            SurfaceField::Symbol(audio_synth_stream_profile_symbol()),
                        ),
                    ],
                })
                .collect(),
        },
    }
}
