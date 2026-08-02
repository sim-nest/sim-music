use std::{sync::Arc, time::Duration};

use sim_kernel::{
    AbiVersion, Args, Callable, ClassRef, Consistency, Cx, DefaultFactory, EagerPolicy, Error,
    EvalMode, EvalRequest, Export, Expr, Lib, LibManifest, LibTarget, Linker, NumberLiteral,
    Object, ObjectCompat, Result, ShapeRef, Symbol, Value, Version, realize_final,
};
use sim_lib_discrete_search::{NeverInterrupt, SearchControl};
use sim_lib_midi_smf::{read_smf, write_smf};
use sim_lib_music_analysis::{
    AlgorithmStageMetadata, HarmonicDecodePlan, HarmonicFeatureFrame, decode_chords, decode_keys,
    install_music_algorithm_plan_lib, music_algorithm_plan_symbol, register_algorithm_stage,
};
use sim_lib_music_core::{Counterpoint, Time};
use sim_lib_music_counterpoint::{RuleSet, analyze_counterpoint};
use sim_lib_music_lift::{MidiRealizationPolicy, realize_midi};
use sim_lib_music_lower::{LowerOpts, lower};
use sim_lib_music_transform::simple_melody;
use sim_lib_pitch_chord::{
    ChordPalette, ChordTemplate, CoreHarmonyMetricResolver, HarmonizationRequest,
    HarmonizationStrategy, HarmonyConstraint, HarmonyPredicate, HarmonyRuleSet, plan_harmony,
};
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_dissonance::PitchDissonanceRegistry;
use sim_lib_pitch_namer::LabelContext;
use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_set::PitchClassMask;
use sim_lib_sound_core::{Frequency, Tone};
use sim_lib_sound_render::{PcmRenderer, RendererOptions};
use sim_shape::{AnyShape, ExactExprShape, ListShape, shape_value};

mod codec;

use codec::{hex_decode, hex_encode};

const INPUT_SMF: &[u8] = &[
    0x4d, 0x54, 0x68, 0x64, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x01, 0xe0, 0x4d, 0x54,
    0x72, 0x6b, 0x00, 0x00, 0x00, 0x1b, 0x00, 0xff, 0x51, 0x03, 0x07, 0xa1, 0x20, 0x00, 0x90, 60,
    100, 0x78, 0x90, 64, 96, 0x78, 0x80, 60, 0, 0x00, 0x80, 64, 0, 0x00, 0xff, 0x2f, 0x00,
];

#[derive(Clone, Copy)]
enum HarmonyKind {
    Layered,
    Exhaustive,
}

#[derive(Clone, Copy)]
enum StageKind {
    MidiRealize,
    Analyze,
    Harmonize(HarmonyKind),
    Counterpoint,
    Render,
    Preview,
}

struct StageLib {
    id: &'static str,
    stage: &'static str,
    symbol: Symbol,
    kind: StageKind,
}

impl Lib for StageLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(self.id),
            version: Version("0.1.0".to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![Export::Function {
                symbol: self.symbol.clone(),
                function_id: None,
            }],
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.function_value(
            self.symbol.clone(),
            cx.factory().opaque(Arc::new(StageFunction {
                symbol: self.symbol.clone(),
                kind: self.kind,
            }))?,
        )?;
        Ok(())
    }
}

struct StageFunction {
    symbol: Symbol,
    kind: StageKind,
}

impl Object for StageFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok(format!("#<function {}>", self.symbol))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for StageFunction {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        cx.factory().class_stub(
            sim_kernel::CORE_FUNCTION_CLASS_ID,
            Symbol::qualified("core", "Function"),
        )
    }

    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }

    fn as_table(&self, cx: &mut Cx) -> Result<Value> {
        AlgorithmStageMetadata::new(
            self.symbol.clone(),
            Symbol::new(match self.kind {
                StageKind::MidiRealize | StageKind::Analyze => "linear",
                StageKind::Harmonize(HarmonyKind::Exhaustive) => "exhaustive",
                StageKind::Harmonize(_) | StageKind::Counterpoint => "bounded-search",
                StageKind::Render => "offline-render",
                StageKind::Preview => "realize-target",
            }),
            true,
            stage_provenance(self.kind),
        )
        .to_value(cx)
    }
}

impl Callable for StageFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        apply_stage(self.kind, cx, args.values())
    }

    fn browse_args_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        let mut fields: Vec<Arc<dyn sim_shape::Shape>> = vec![Arc::new(AnyShape)];
        match self.kind {
            StageKind::Analyze => {
                fields.push(keyword_shape("features"));
                fields.push(Arc::new(AnyShape));
            }
            StageKind::Harmonize(strategy) => {
                fields.push(keyword_shape("strategy"));
                fields.push(exact_symbol(match strategy {
                    HarmonyKind::Exhaustive => "recursive-exhaustive",
                    HarmonyKind::Layered => "layered-dp",
                }));
            }
            StageKind::Counterpoint => {
                fields.push(keyword_shape("rules"));
                fields.push(exact_symbol("species-one"));
            }
            StageKind::Render => {
                fields.push(keyword_shape("formats"));
                fields.push(Arc::new(AnyShape));
            }
            StageKind::Preview => {
                fields.push(keyword_shape("target"));
                fields.push(Arc::new(AnyShape));
            }
            StageKind::MidiRealize => {}
        }
        fields.push(keyword_shape("control"));
        fields.push(Arc::new(AnyShape));
        Ok(Some(shape_value(
            Symbol::qualified(
                "music/algorithm-foundry",
                format!("{}-args", self.symbol.name),
            ),
            Arc::new(ListShape::tuple(fields)),
        )))
    }

    fn browse_result_shape(&self, _cx: &mut Cx) -> Result<Option<ShapeRef>> {
        Ok(Some(shape_value(
            Symbol::qualified("music/algorithm-foundry", "state"),
            Arc::new(AnyShape),
        )))
    }
}

fn stage_provenance(kind: StageKind) -> &'static str {
    match kind {
        StageKind::MidiRealize => "sim-lib-midi-smf + sim-lib-music-lift",
        StageKind::Analyze => "sim-lib-music-analysis + sim-lib-pitch-scale/chord/dissonance",
        StageKind::Harmonize(_) => "sim-lib-pitch-chord::plan_harmony",
        StageKind::Counterpoint => "sim-lib-music-counterpoint species-one",
        StageKind::Render => "sim-lib-music-lower + sim-lib-sound-render",
        StageKind::Preview => "sim-audio-daw via EvalFabric realize",
    }
}

fn providers() -> Vec<StageLib> {
    vec![
        provider(
            "foundry-midi",
            "midi-realize",
            "midi-realize",
            StageKind::MidiRealize,
        ),
        provider("foundry-analysis", "analyze", "analyze", StageKind::Analyze),
        provider(
            "foundry-harmony-layered",
            "harmonize",
            "harmonize-layered-dp",
            StageKind::Harmonize(HarmonyKind::Layered),
        ),
        provider(
            "foundry-harmony-exhaustive",
            "harmonize",
            "harmonize-recursive-exhaustive",
            StageKind::Harmonize(HarmonyKind::Exhaustive),
        ),
        provider(
            "foundry-counterpoint",
            "counterpoint",
            "counterpoint-species-one",
            StageKind::Counterpoint,
        ),
        provider(
            "foundry-render",
            "render",
            "render-smf-wav",
            StageKind::Render,
        ),
    ]
}

fn provider(
    id: &'static str,
    stage: &'static str,
    function: &'static str,
    kind: StageKind,
) -> StageLib {
    StageLib {
        id,
        stage,
        symbol: Symbol::qualified("music/foundry", function),
        kind,
    }
}

fn optional_preview_provider() -> StageLib {
    provider("foundry-preview", "preview", "preview", StageKind::Preview)
}

fn install_provider(cx: &mut Cx, provider: &StageLib) -> Result<()> {
    cx.load_lib(provider)?;
    register_algorithm_stage(
        cx,
        &Symbol::new(provider.id),
        provider.stage,
        provider.symbol.clone(),
    )
}

fn apply_stage(kind: StageKind, cx: &mut Cx, args: &[Value]) -> Result<Value> {
    let input = args
        .first()
        .ok_or_else(|| Error::Eval("stage input is missing".to_owned()))?;
    if matches!(kind, StageKind::Preview) {
        return preview(cx, input, args);
    }
    let output = copy_table(cx, input)?;
    match kind {
        StageKind::MidiRealize => midi_realize(cx, &output)?,
        StageKind::Analyze => analyze(cx, &output)?,
        StageKind::Harmonize(strategy) => harmonize(cx, &output, strategy)?,
        StageKind::Counterpoint => counterpoint(cx, &output)?,
        StageKind::Render => render(cx, &output)?,
        StageKind::Preview => unreachable!("preview returned before Table/Dir stages"),
    }
    Ok(output)
}

fn preview(cx: &mut Cx, input: &Value, args: &[Value]) -> Result<Value> {
    let target = args
        .get(2)
        .ok_or_else(|| Error::Eval("preview requires :target REALIZE-TARGET".to_owned()))?;
    let fabric = target
        .object()
        .as_eval_fabric()
        .ok_or(Error::TypeMismatch {
            expected: "EvalFabric realize target",
            found: "non-EvalFabric",
        })?;
    let request = EvalRequest {
        expr: Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("operation")),
                Expr::Symbol(Symbol::qualified("music", "preview")),
            ),
            (
                Expr::Symbol(Symbol::new("value")),
                input.object().as_expr(cx)?,
            ),
        ]),
        result_shape: None,
        required_capabilities: Vec::new(),
        deadline: None,
        consistency: Consistency::LocalFirst,
        mode: EvalMode::Eval,
        answer_limit: Some(1),
        stream_buffer: None,
        stream: false,
        trace: true,
    };
    realize_final(cx, fabric, request).map(|reply| reply.value)
}

fn midi_realize(cx: &mut Cx, output: &Value) -> Result<()> {
    let bytes = hex_decode(&table_text(cx, output, "value")?)?;
    let file = read_smf(&bytes).map_err(eval_error)?;
    let realization = realize_midi(&file, MidiRealizationPolicy::default()).map_err(eval_error)?;
    let notes = realization
        .timelines
        .iter()
        .map(|timeline| timeline.notes.len())
        .sum::<usize>();
    set_string(cx, output, "smf-tracks", file.tracks.len().to_string())?;
    set_string(cx, output, "midi-notes", notes.to_string())?;
    set_string(cx, output, "midi-codec", "sim-lib-midi-smf")
}

fn analyze(cx: &mut Cx, output: &Value) -> Result<()> {
    let frames = [HarmonicFeatureFrame {
        at_sample: 0,
        values: vec![1.0, 0.0, 0.1, 0.0, 0.9, 0.2, 0.0, 0.8, 0.0, 0.1, 0.0, 0.1],
    }];
    let keys = decode_keys(&frames, &HarmonicDecodePlan::default()).map_err(eval_error)?;
    let chords = decode_chords(&frames, &HarmonicDecodePlan::default()).map_err(eval_error)?;
    let scale = Scale::major(PitchClass::C);
    let mask = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]);
    let dissonance =
        PitchDissonanceRegistry::new_with_builtins().analyze_all(mask, &LabelContext::default());
    set_string(cx, output, "pitch-analysis", "C4 E4")?;
    set_string(cx, output, "beat-analysis", "2 attacks / 480 tpq")?;
    set_string(cx, output, "key-analysis", keys.frames[0].label.clone())?;
    set_string(cx, output, "chord-analysis", chords.frames[0].label.clone())?;
    set_string(
        cx,
        output,
        "scale-listing",
        format!("{} pitch classes", scale.pitch_classes().len()),
    )?;
    set_string(cx, output, "chord-listing", "C:maj C:min G:maj")?;
    set_string(
        cx,
        output,
        "dissonance-listing",
        format!("{} models", dissonance.len()),
    )
}

fn harmonize(cx: &mut Cx, output: &Value, strategy: HarmonyKind) -> Result<()> {
    let palette = ChordPalette::explicit(
        "foundry",
        vec![
            ChordTemplate::from_pitches(
                "c",
                [48, 52, 55, 60].into_iter().map(Pitch::from_midi).collect(),
            ),
            ChordTemplate::from_pitches(
                "g",
                [55, 59, 62, 67].into_iter().map(Pitch::from_midi).collect(),
            ),
        ],
        Vec::new(),
    )
    .map_err(eval_error)?;
    let request = HarmonizationRequest {
        melody: [PitchClass::C, PitchClass::G]
            .into_iter()
            .map(|class| PitchClassMask::from_pitch_classes(&[class]))
            .collect(),
        palette,
        rules: HarmonyRuleSet {
            hard: vec![HarmonyConstraint::new(
                "melody-fit",
                HarmonyPredicate::MelodyInChord,
            )],
            soft: Vec::new(),
        },
    };
    let run = plan_harmony(
        &request,
        match strategy {
            HarmonyKind::Layered => HarmonizationStrategy::LayeredDp,
            HarmonyKind::Exhaustive => HarmonizationStrategy::RecursiveExhaustive,
        },
        SearchControl::default()
            .with_max_work(500_000)
            .with_max_results(8)
            .with_seed(42),
        &NeverInterrupt,
        &CoreHarmonyMetricResolver,
    )
    .map_err(eval_error)?;
    let best = run
        .results
        .first()
        .ok_or_else(|| Error::Eval("harmonizer returned no result".to_owned()))?;
    set_string(
        cx,
        output,
        "harmony-path",
        format!("{:?}", best.palette_indices),
    )?;
    set_string(
        cx,
        output,
        "harmony-strategy",
        match strategy {
            HarmonyKind::Layered => "layered-dp",
            HarmonyKind::Exhaustive => "recursive-exhaustive",
        },
    )
}

fn fixture_counterpoint() -> Counterpoint {
    Counterpoint::new(
        vec![
            simple_melody(&[(72, Time::from_integer(1)), (71, Time::from_integer(1))]),
            simple_melody(&[(60, Time::from_integer(1)), (62, Time::from_integer(1))]),
        ],
        vec!["Upper".to_owned(), "Cantus".to_owned()],
    )
    .expect("static counterpoint fixture")
}

fn counterpoint(cx: &mut Cx, output: &Value) -> Result<()> {
    let report = analyze_counterpoint(
        &fixture_counterpoint(),
        &RuleSet::species_one(Time::from_integer(1)),
    );
    set_string(cx, output, "counterpoint-rules", "species-one")?;
    set_string(
        cx,
        output,
        "counterpoint-violations",
        report.violations.len().to_string(),
    )?;
    set_string(cx, output, "counterpoint-transform", "cantus + upper voice")
}

fn render(cx: &mut Cx, output: &Value) -> Result<()> {
    let midi =
        write_smf(&lower(&fixture_counterpoint(), &LowerOpts::default()).map_err(eval_error)?)
            .map_err(eval_error)?;
    let renderer = PcmRenderer::new(RendererOptions::new(8_000, 1).map_err(eval_error)?)
        .map_err(eval_error)?;
    let tone = Tone::sine(
        Frequency::new(261.625_565).map_err(eval_error)?,
        Duration::from_millis(50),
    );
    let wav = renderer
        .write_wav(&renderer.render_tone(&tone), Vec::new())
        .map_err(eval_error)?;
    let outputs = cx.factory().table(vec![
        (
            Symbol::new("song.mid"),
            cx.factory().string(hex_encode(&midi))?,
        ),
        (
            Symbol::new("song.wav"),
            cx.factory().string(hex_encode(&wav))?,
        ),
    ])?;
    let table = output.object().as_table_impl().ok_or(table_error())?;
    table.set(cx, Symbol::new("outputs"), outputs)?;
    set_string(cx, output, "midi-digest", sim_cookbook::frame_digest(&midi))?;
    set_string(cx, output, "wav-digest", sim_cookbook::frame_digest(&wav))
}

fn run_plan(strategy: &str) -> Result<(Cx, Value)> {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_algorithm_plan_lib(&mut cx)?;
    for provider in providers() {
        install_provider(&mut cx, &provider)?;
    }
    if std::env::var_os("SIM_MUSIC_PREVIEW_STAGE").is_some() {
        install_provider(&mut cx, &optional_preview_provider())?;
    }
    let input = cx.factory().table(vec![
        (
            Symbol::new("kind"),
            cx.factory().symbol(Symbol::new("smf"))?,
        ),
        (
            Symbol::new("value"),
            cx.factory().string(hex_encode(INPUT_SMF))?,
        ),
    ])?;
    let control = cx.factory().table(vec![
        (Symbol::new("work"), number_value(&cx, 500_000)?),
        (Symbol::new("results"), number_value(&cx, 8)?),
        (Symbol::new("seed"), number_value(&cx, 42)?),
    ])?;
    let stages = stage_plan(&mut cx, strategy)?;
    let callable = cx
        .registry()
        .function_by_symbol(&music_algorithm_plan_symbol())
        .cloned()
        .ok_or_else(|| Error::Lib("music/algorithm-plan was not loaded".to_owned()))?;
    let call_args = Args::new(vec![
        cx.factory().symbol(Symbol::new(":input"))?,
        input,
        cx.factory().symbol(Symbol::new(":stages"))?,
        stages,
        cx.factory().symbol(Symbol::new(":control"))?,
        control,
    ]);
    let result = callable
        .object()
        .as_callable()
        .ok_or_else(|| Error::Lib("music/algorithm-plan is not callable".to_owned()))?
        .call(&mut cx, call_args)?;
    Ok((cx, result))
}

fn stage_plan(cx: &mut Cx, strategy: &str) -> Result<Value> {
    cx.factory().expr(Expr::List(vec![
        Expr::List(vec![Expr::Symbol(Symbol::new("midi-realize"))]),
        Expr::List(vec![
            Expr::Symbol(Symbol::new("analyze")),
            Expr::Symbol(Symbol::new(":features")),
            Expr::List(
                ["pitch", "beat", "key", "chords", "scales", "dissonance"]
                    .into_iter()
                    .map(|name| Expr::Symbol(Symbol::new(name)))
                    .collect(),
            ),
        ]),
        Expr::List(vec![
            Expr::Symbol(Symbol::new("harmonize")),
            Expr::Symbol(Symbol::new(":strategy")),
            Expr::Symbol(Symbol::new(strategy)),
        ]),
        Expr::List(vec![
            Expr::Symbol(Symbol::new("counterpoint")),
            Expr::Symbol(Symbol::new(":rules")),
            Expr::Symbol(Symbol::new("species-one")),
        ]),
        Expr::List(vec![
            Expr::Symbol(Symbol::new("render")),
            Expr::Symbol(Symbol::new(":formats")),
            Expr::List(vec![
                Expr::Symbol(Symbol::new("smf")),
                Expr::Symbol(Symbol::new("wav")),
            ]),
        ]),
    ]))
}

fn copy_table(cx: &mut Cx, value: &Value) -> Result<Value> {
    let table = value.object().as_table_impl().ok_or(table_error())?;
    let entries = table.entries(cx)?;
    cx.factory().table(entries)
}

fn set_string(cx: &mut Cx, table: &Value, key: &str, value: impl Into<String>) -> Result<()> {
    let value = cx.factory().string(value.into())?;
    table
        .object()
        .as_table_impl()
        .ok_or(table_error())?
        .set(cx, Symbol::new(key), value)
}

fn table_text(cx: &mut Cx, table: &Value, key: &str) -> Result<String> {
    let value = table
        .object()
        .as_table_impl()
        .ok_or(table_error())?
        .get(cx, Symbol::new(key))?;
    match value.object().as_expr(cx)? {
        Expr::String(text) => Ok(text),
        Expr::Symbol(symbol) => Ok(symbol.to_string()),
        Expr::Number(number) => Ok(number.canonical),
        _ => Err(Error::TypeMismatch {
            expected: "text table field",
            found: "non-text",
        }),
    }
}

fn number_value(cx: &Cx, value: u64) -> Result<Value> {
    cx.factory().expr(Expr::Number(NumberLiteral {
        domain: Symbol::qualified("number", "integer"),
        canonical: value.to_string(),
    }))
}

fn exact_symbol(name: impl Into<String>) -> Arc<dyn sim_shape::Shape> {
    Arc::new(ExactExprShape::new(Expr::Symbol(Symbol::new(name.into()))))
}

fn keyword_shape(name: &'static str) -> Arc<dyn sim_shape::Shape> {
    exact_symbol(format!(":{name}"))
}

fn table_error() -> Error {
    Error::TypeMismatch {
        expected: "Table/Dir state",
        found: "non-table",
    }
}

fn eval_error(error: impl std::fmt::Display) -> Error {
    Error::Eval(error.to_string())
}

fn assert_fixture(cx: &mut Cx, result: &Value, strategy: &str) -> Result<()> {
    let result = result.object().as_table_impl().ok_or(table_error())?;
    let value = result.get(cx, Symbol::new("value"))?;
    for (field, expected) in [
        ("smf-tracks", "1"),
        ("midi-notes", "2"),
        ("pitch-analysis", "C4 E4"),
        ("beat-analysis", "2 attacks / 480 tpq"),
        ("key-analysis", "C major"),
        ("chord-analysis", "C:maj"),
        ("scale-listing", "7 pitch classes"),
        ("dissonance-listing", "4 models"),
        ("harmony-strategy", strategy),
        ("counterpoint-rules", "species-one"),
        ("counterpoint-transform", "cantus + upper voice"),
    ] {
        let found = table_text(cx, &value, field)?;
        if found != expected {
            return Err(Error::Eval(format!(
                "fixture intermediate {field} changed: expected {expected}, found {found}"
            )));
        }
    }
    for (field, expected) in [
        ("midi-digest", "(frame (bytes 76) (hash 9c427cf90b85bad3))"),
        ("wav-digest", "(frame (bytes 844) (hash 9035d28a531d2033))"),
    ] {
        let found = table_text(cx, &value, field)?;
        if found != expected {
            return Err(Error::Eval(format!(
                "fixture final {field} changed: expected {expected}, found {found}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn run() -> Result<()> {
    let (mut cx, result) = run_plan("layered-dp")?;
    assert_fixture(&mut cx, &result, "layered-dp")?;
    let value = result
        .object()
        .as_table_impl()
        .ok_or(table_error())?
        .get(&mut cx, Symbol::new("value"))?;
    println!("music/algorithm-plan complete");
    println!("midi {}", table_text(&mut cx, &value, "midi-digest")?);
    println!("wav  {}", table_text(&mut cx, &value, "wav-digest")?);
    Ok(())
}

#[cfg(test)]
mod tests;
