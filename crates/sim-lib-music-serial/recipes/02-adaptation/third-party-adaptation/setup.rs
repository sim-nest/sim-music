use std::collections::BTreeMap;
use std::sync::Arc;

use sim_codec::{Input, decode_eval_expr_with_codec};
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{
    CapabilitySet, Cx, DefaultFactory, EagerPolicy, Expr, ReadPolicy, Symbol, TrustLevel,
};
use sim_lib_music_core::{Articulation, Channel, ObjectId, PitchClass, Time};
use sim_lib_music_serial::{
    DeclaredWaivers, EventPlacement, EvidenceId, InvariantLedger, InvariantLedgerEntry,
    InvariantStatus, OrdinalRef, PlannedSerialEvent, PracticeId, PracticeRule, PracticeRuleId,
    PracticeRuleKind, PracticeRuleParameter, PracticeRuleSpec, RealizationContext, RealizedSerialNote,
    RealizerId, RowInstanceId, SerialEventId, SerialOrigin, SerialPlan, SerialPractice,
    SerialReading, SerialRealization, SerialRealizer, SerialRole,
    StrictEventSpec, StrictRealizationContext, StrictRealizationError, StructuralLicense,
    StructuralReadingId, default_realizer_registry,
};
use sim_lib_music_shapes::{decode_serial_series, encode_serial_series, install_music_shapes_lib};
use sim_lib_pitch_scale::{PlayerScale, Scale};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};
use sim_lib_serial_core::{
    AggregateRule, AlphabetId, AlphabetRegistry, FiniteAlphabet, SerialAlphabet, Series,
};

pub fn third_party_adaptation() -> Result<(), Box<dyn std::error::Error>> {
    let alphabet = seven_symbol_alphabet()?;
    let mut alphabet_registry = AlphabetRegistry::new();
    alphabet_registry.insert(alphabet.clone())?;
    assert_eq!(
        alphabet_registry
            .get(alphabet.id())
            .expect("registered alphabet")
            .symbols()
            .len(),
        7
    );

    let series = seven_symbol_series(alphabet)?;
    let encoded_series = encode_serial_series(&series)?;
    assert_eq!(decode_serial_series(&encoded_series)?, series);
    assert_eq!(
        lisp_validate_series(&encoded_series)?,
        series.permutation_rank()?.to_string()
    );
    assert_shape_reports_serial_diagnostics(&encoded_series)?;

    let plan = third_party_plan()?;
    let practice_rule: Arc<dyn PracticeRule> = Arc::new(VoiceBalanceRule::new()?);
    let practice = SerialPractice::new(
        PracticeId::new("practice/third-party/voice-balance")?,
        vec![practice_rule],
    );
    let report = practice.evaluate(
        &plan,
        SerialReading::StructuralPlan,
        &DeclaredWaivers::default(),
    );
    assert!(!report.has_unwaived_violations());
    assert_eq!(report.ledger.entries().len(), 1);
    assert!(matches!(
        report.ledger.entries()[0].status,
        InvariantStatus::Preserved
    ));

    let context = third_party_context()?;
    let mut registry = default_realizer_registry();
    registry
        .register(Arc::new(ThirdPartySpineRealizer::new()?))
        .map_err(|id| format!("duplicate realizer registration for {}", id.as_str()))?;
    let realization =
        registry.realize_named("realizer/third-party/modal-token", &plan, &context)?;
    assert_eq!(realization.plan(), &plan);
    assert_eq!(realization.events().len(), plan.events().len());
    assert_eq!(realization.notes().len(), 12);
    assert!(realization.ledger().is_preserved("serial/third-party-adaptation"));
    assert!(realization
        .notes()
        .iter()
        .all(|note| note.origin.realizer_id.as_str() == "realizer/third-party/modal-token"));

    match registry.realize_named("realizer/third-party/missing", &plan, &context) {
        Err(StrictRealizationError::UnknownRealizer(id)) => {
            assert_eq!(id.as_str(), "realizer/third-party/missing");
        }
        other => panic!("expected unknown realizer, found {other:?}"),
    }

    Ok(())
}

fn seven_symbol_alphabet() -> Result<FiniteAlphabet<String>, Box<dyn std::error::Error>> {
    Ok(FiniteAlphabet::try_new(
        AlphabetId::try_new("gesture/seven-v1")?,
        [
            "pulse", "sustain", "glint", "slide", "bend", "drone", "rest",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    )?)
}

fn seven_symbol_series(
    alphabet: FiniteAlphabet<String>,
) -> Result<Series<FiniteAlphabet<String>>, Box<dyn std::error::Error>> {
    Ok(Series::try_new(
        alphabet,
        AggregateRule::exhaustive_exactly_once(),
        [
            "slide", "pulse", "drone", "rest", "bend", "glint", "sustain",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    )?)
}

fn lisp_validate_series(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut cx = serial_cx()?;
    let expression = decode_eval_expr_with_codec(
        &mut cx,
        &Symbol::qualified("codec", "lisp"),
        Input::Text(format!("(music/serial/validate {source:?})")),
        ReadPolicy {
            trust: TrustLevel::TrustedSource,
            capabilities: CapabilitySet::new(),
        },
    )?;
    let result = cx.eval_expr(expression)?;
    let Expr::Map(fields) = result.object().as_expr(&mut cx)? else {
        return Err("music/serial/validate must return a map".into());
    };
    let alphabet_id = map_string(&fields, "alphabet-id")?;
    let rank = map_string(&fields, "permutation-rank")?;
    assert_eq!(alphabet_id, "gesture/seven-v1");
    Ok(rank)
}

fn assert_shape_reports_serial_diagnostics(
    source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cx = serial_cx()?;
    let shape = cx
        .registry()
        .shape_by_symbol(&Symbol::qualified("music", "SerialSeries"))
        .expect("registered SerialSeries shape")
        .clone();
    let accepted = shape
        .object()
        .as_shape()
        .expect("shape protocol")
        .check_expr(&mut cx, &Expr::String(source.to_owned()))?;
    assert!(accepted.accepted, "{:?}", accepted.diagnostics);

    let (prefix, suffix) = source
        .rsplit_once("\"sustain\"")
        .ok_or("expected sustain in encoded series order")?;
    let rejected_source = format!("{prefix}\"unknown\"{suffix}");
    let rejected = shape
        .object()
        .as_shape()
        .expect("shape protocol")
        .check_expr(&mut cx, &Expr::String(rejected_source))?;
    assert!(!rejected.accepted);
    assert!(rejected
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("shape-serial-series")));
    Ok(())
}

fn serial_cx() -> Result<Cx, Box<dyn std::error::Error>> {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory), sim_kernel::HandleSeed::new(0x0a2c_b6d4_a64a_15b3));
    install_music_shapes_lib(&mut cx)?;
    let lisp = LispCodecLib::new(cx.registry_mut().fresh_codec_id())?;
    cx.load_lib(&lisp)?;
    Ok(cx)
}

fn map_string(
    fields: &[(Expr, Expr)],
    key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    fields
        .iter()
        .find_map(|(candidate, value)| match (candidate, value) {
            (Expr::Symbol(symbol), Expr::String(text)) if symbol.name.as_ref() == key => {
                Some(text.clone())
            }
            _ => None,
        })
        .ok_or_else(|| format!("missing {key} result field").into())
}

fn third_party_plan() -> Result<SerialPlan, Box<dyn std::error::Error>> {
    let row = ToneRow::try_from_classes([
        PitchClass::E,
        PitchClass::F,
        PitchClass::G,
        PitchClass::CS,
        PitchClass::FS,
        PitchClass::DS,
        PitchClass::GS,
        PitchClass::D,
        PitchClass::B,
        PitchClass::C,
        PitchClass::A,
        PitchClass::AS,
    ])?
    .apply(RowOperation::new(RowFamily::P, 0));
    let row_id = RowInstanceId::new("row/third-party/p0")?;
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), row);
    let license = StructuralLicense::new(
        StructuralReadingId::new("reading/third-party/external")?,
        "third-party adaptation reading",
    )?;
    let event = |id: &str, ordinals: [usize; 2], voice: &str| -> Result<PlannedSerialEvent, Box<dyn std::error::Error>> {
        Ok(PlannedSerialEvent {
            id: SerialEventId::new(id)?,
            ordinals: ordinals
                .into_iter()
                .map(|ordinal| OrdinalRef::new(row_id.clone(), ordinal))
                .collect(),
            role: SerialRole::Structural,
            origin: SerialOrigin::Structural {
                rationale: "third-party adaptation event".to_owned(),
            },
            voice: ObjectId::new(voice)?,
            placement: EventPlacement::independent(),
            parents: vec![],
            licenses: vec![license.clone()],
        })
    };
    let events = [
        event("event/a", [0, 1], "voice/high")?,
        event("event/b", [2, 3], "voice/low")?,
        event("event/c", [4, 5], "voice/high")?,
        event("event/d", [6, 7], "voice/low")?,
        event("event/e", [8, 9], "voice/high")?,
        event("event/f", [10, 11], "voice/low")?,
    ];
    SerialPlan::try_new(
        rows,
        events
            .into_iter()
            .map(|event| (event.id.clone(), event))
            .collect(),
        [
            (SerialEventId::new("event/a")?, SerialEventId::new("event/b")?),
            (SerialEventId::new("event/b")?, SerialEventId::new("event/c")?),
            (SerialEventId::new("event/c")?, SerialEventId::new("event/d")?),
            (SerialEventId::new("event/d")?, SerialEventId::new("event/e")?),
            (SerialEventId::new("event/e")?, SerialEventId::new("event/f")?),
        ],
    )
    .map_err(Into::into)
}

fn third_party_context() -> Result<RealizationContext, Box<dyn std::error::Error>> {
    let channel = Channel::new(0)?;
    let quarter = Time::new(1, 4);
    let specs = [
        ("event/a", StrictEventSpec::notes(4, quarter, 92, channel, Articulation::Accent)),
        ("event/b", StrictEventSpec::notes(3, quarter, 88, channel, Articulation::Tenuto)),
        ("event/c", StrictEventSpec::notes(4, quarter, 84, channel, Articulation::Legato)),
        ("event/d", StrictEventSpec::notes(3, quarter, 84, channel, Articulation::Legato)),
        ("event/e", StrictEventSpec::notes(5, quarter, 90, channel, Articulation::Normal)),
        ("event/f", StrictEventSpec::notes(4, quarter, 78, channel, Articulation::Staccato)),
    ]
    .into_iter()
    .map(|(id, spec)| Ok((SerialEventId::new(id)?, spec)))
    .collect::<Result<BTreeMap<_, _>, Box<dyn std::error::Error>>>()?;
    let mut context = StrictRealizationContext::new(specs);
    context.modal_scale = Some(PlayerScale::from_scale(Scale::dorian(PitchClass::C)));
    context.services.insert(
        "third-party/adaptation-tag",
        Arc::new(String::from("tag/non-pitch-seven")),
    );
    Ok(context)
}

struct VoiceBalanceRule {
    id: PracticeRuleId,
}

impl VoiceBalanceRule {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            id: PracticeRuleId::new("rule/third-party/voice-balance")?,
        })
    }
}

impl PracticeRule for VoiceBalanceRule {
    fn id(&self) -> &PracticeRuleId {
        &self.id
    }

    fn spec(&self) -> PracticeRuleSpec {
        PracticeRuleSpec {
            id: self.id.clone(),
            kind: PracticeRuleKind::Order,
            expected_fact: "structural coverage alternates across the declared external voices"
                .to_owned(),
            parameters: vec![PracticeRuleParameter {
                name: "voices".to_owned(),
                value: "voice/high,voice/low".to_owned(),
            }],
        }
    }

    fn evaluate(
        &self,
        plan: &SerialPlan,
        _reading: SerialReading,
        _waivers: &DeclaredWaivers,
    ) -> InvariantLedgerEntry<PracticeRuleId> {
        let observed = plan
            .events()
            .values()
            .map(|event| event.voice.as_str().to_owned())
            .collect::<Vec<_>>();
        let alternating = observed
            .windows(2)
            .all(|window| window[0] != window[1])
            && observed.iter().any(|voice| voice == "voice/high")
            && observed.iter().any(|voice| voice == "voice/low");
        InvariantLedgerEntry {
            rule_id: self.id.clone(),
            invariant_id: Some("serial/third-party-voice-balance".to_owned()),
            expected_fact:
                "structural coverage alternates across the declared external voices".to_owned(),
            observed_fact: observed.join(" -> "),
            status: if alternating {
                InvariantStatus::Preserved
            } else {
                InvariantStatus::Violated
            },
            evidence_ids: vec![EvidenceId::new("evidence/third-party-voice-balance")
                .expect("stable evidence id")],
            declared_waiver: None,
        }
    }
}

struct ThirdPartySpineRealizer {
    id: RealizerId,
}

impl ThirdPartySpineRealizer {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            id: RealizerId::new("realizer/third-party/modal-token")?,
        })
    }
}

impl SerialRealizer for ThirdPartySpineRealizer {
    fn id(&self) -> &RealizerId {
        &self.id
    }

    fn realize(
        &self,
        plan: &SerialPlan,
        context: &RealizationContext,
    ) -> Result<SerialRealization, StrictRealizationError> {
        let adaptation_tag = context
            .services
            .get::<String>("third-party/adaptation-tag")
            .cloned()
            .ok_or_else(|| {
                StrictRealizationError::PitchMap(
                    "missing third-party/adaptation-tag realization service".to_owned(),
                )
            })?;
        let delegated = default_realizer_registry().realize_named(
            "realizer/modal-non-pitch-spine",
            plan,
            context,
        )?;
        let notes = delegated
            .notes()
            .iter()
            .cloned()
            .map(|mut note: RealizedSerialNote| {
                note.origin.realizer_id = self.id.clone();
                note
            })
            .collect();
        let ledger = InvariantLedger::new(vec![InvariantLedgerEntry {
            rule_id: self.id.clone(),
            invariant_id: Some("serial/third-party-adaptation".to_owned()),
            expected_fact:
                "caller-defined modal adaptation stays loadable through public registration"
                    .to_owned(),
            observed_fact: format!(
                "custom realizer delegated non-pitch spine through public registry with {adaptation_tag}"
            ),
            status: InvariantStatus::Preserved,
            evidence_ids: vec![EvidenceId::new("evidence/third-party-adaptation")
                .expect("stable evidence id")],
            declared_waiver: None,
        }]);
        Ok(SerialRealization::new_with_spine(
            plan.clone(),
            delegated.events().to_vec(),
            notes,
            ledger,
            delegated.spine_report().cloned(),
        ))
    }
}
