use sim_lib_music_core::{Articulation, Time};
use sim_lib_serial_core::SeriesTransform;

use crate::{
    ArticulationTrack, DurationTrack, DynamicsTrack, Exhaustion, IntegralPlan, ParameterTrack,
    RegisterTrack, TimbreTrack,
};

#[test]
fn parameter_series_bindings_keep_generic_ledgers() {
    let mut plan = IntegralPlan::new(12);
    let custom = ParameterTrack::try_new("bow-pressure", vec![1_u8, 3, 2, 4], Exhaustion::Cycle)
        .expect("custom track");
    plan.bind_parameter(custom).expect("bind custom");

    let bound = plan
        .typed_parameter::<u8>("bow-pressure")
        .expect("typed custom parameter");
    let ordinals = bound
        .ordinal_ledger()
        .into_iter()
        .map(|entry| (entry.plan_ordinal, entry.parameter_ordinal, entry.cycle))
        .collect::<Vec<_>>();
    assert_eq!(
        ordinals[..6],
        [
            (0, 0, 0),
            (1, 1, 0),
            (2, 2, 0),
            (3, 3, 0),
            (4, 0, 1),
            (5, 1, 1)
        ]
    );
    assert_eq!(
        bound.projection().steps()[..4]
            .iter()
            .map(|step| step.value)
            .collect::<Vec<_>>(),
        vec![1, 3, 2, 4]
    );
}

#[test]
fn integral_plan_accepts_builtin_and_custom_parameter_tracks_through_one_path() {
    let mut plan = IntegralPlan::new(12);
    plan.bind_parameter(
        DurationTrack::try_new(
            "duration",
            vec![
                Time::new(1, 8),
                Time::new(1, 4),
                Time::new(3, 8),
                Time::new(1, 2),
            ],
            Exhaustion::Cycle,
        )
        .expect("duration track"),
    )
    .expect("bind duration");
    plan.bind_parameter(
        DynamicsTrack::try_new("dynamics", vec![48, 64, 96, 112], Exhaustion::Cycle)
            .expect("dynamics track"),
    )
    .expect("bind dynamics");
    plan.bind_parameter(
        RegisterTrack::try_new("register", vec![3, 4, 5], Exhaustion::Cycle)
            .expect("register track"),
    )
    .expect("bind register");
    plan.bind_parameter(
        ArticulationTrack::try_new(
            "articulation",
            vec![
                Articulation::Normal,
                Articulation::Accent,
                Articulation::Marcato,
            ],
            Exhaustion::Cycle,
        )
        .expect("articulation track"),
    )
    .expect("bind articulation");
    plan.bind_parameter(
        TimbreTrack::try_new(
            "timbre",
            vec!["clarinet".to_owned(), "oboe".to_owned(), "horn".to_owned()],
            Exhaustion::Cycle,
        )
        .expect("timbre track"),
    )
    .expect("bind timbre");
    plan.bind_parameter(
        ParameterTrack::try_new("bow-pressure", vec![1_u8, 3, 2, 4], Exhaustion::Cycle)
            .expect("custom track"),
    )
    .expect("bind custom");

    assert!(plan.typed_parameter::<Time>("duration").is_some());
    assert!(plan.typed_parameter::<u8>("dynamics").is_some());
    assert!(plan.typed_parameter::<i8>("register").is_some());
    assert!(
        plan.typed_parameter::<Articulation>("articulation")
            .is_some()
    );
    assert!(plan.typed_parameter::<String>("timbre").is_some());
    assert!(plan.typed_parameter::<u8>("bow-pressure").is_some());
    assert_eq!(
        plan.parameter("timbre")
            .expect("timbre binding")
            .debug_values()[..3],
        ["\"clarinet\"", "\"oboe\"", "\"horn\""]
    );
}

#[test]
fn parameter_tracks_support_independent_phasing_exhaustion_and_transforms() {
    let duration = DurationTrack::try_new(
        "duration",
        vec![
            Time::new(1, 8),
            Time::new(1, 4),
            Time::new(3, 8),
            Time::new(1, 2),
        ],
        Exhaustion::Cycle,
    )
    .expect("duration")
    .with_phase(1);
    let register = RegisterTrack::try_new("register", vec![3, 4, 5], Exhaustion::Truncate)
        .expect("register")
        .with_phase(1);
    let articulation = ArticulationTrack::try_new(
        "articulation",
        vec![
            Articulation::Normal,
            Articulation::Accent,
            Articulation::Marcato,
        ],
        Exhaustion::OneShot,
    )
    .expect("articulation")
    .with_phase(1);
    let dynamics = DynamicsTrack::try_new("dynamics", vec![40, 60, 80, 100], Exhaustion::Cycle)
        .expect("dynamics");
    let transformed = dynamics
        .transformed(&SeriesTransform::retrograde(4))
        .expect("transform");

    let duration_projection = duration.project(12);
    let register_projection = register.project(6);
    let articulation_projection = articulation.project(6);
    let transformed_projection = transformed.project(4);
    let original_projection = dynamics.project(4);

    assert_eq!(
        duration_projection
            .steps()
            .iter()
            .map(|step| step.parameter_ordinal)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0]
    );
    assert_eq!(
        register_projection
            .steps()
            .iter()
            .map(|step| (step.plan_ordinal, step.parameter_ordinal, step.value))
            .collect::<Vec<_>>(),
        vec![(0, 1, 4), (1, 2, 5)]
    );
    assert_eq!(
        articulation_projection
            .steps()
            .iter()
            .map(|step| step.parameter_ordinal)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        articulation_projection.omitted_plan_ordinals(),
        &[2, 3, 4, 5]
    );
    assert_eq!(
        original_projection
            .steps()
            .iter()
            .map(|step| step.value)
            .collect::<Vec<_>>(),
        vec![40, 60, 80, 100]
    );
    assert_eq!(
        transformed_projection
            .steps()
            .iter()
            .map(|step| step.value)
            .collect::<Vec<_>>(),
        vec![100, 80, 60, 40]
    );
}
