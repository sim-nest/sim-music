use sim_lib_music_core::{Articulation, Time};
use sim_lib_music_serial::{
    ArticulationTrack, DurationTrack, DynamicsTrack, Exhaustion, IntegralPlan, ParameterTrack,
    RegisterTrack, TimbreTrack,
};
use sim_lib_serial_core::SeriesTransform;

pub fn integral_parameters() -> Result<(), Box<dyn std::error::Error>> {
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
        )?,
    )?;
    plan.bind_parameter(DynamicsTrack::try_new("dynamics", vec![48, 64, 96, 112], Exhaustion::Cycle)?)?;
    plan.bind_parameter(RegisterTrack::try_new("register", vec![3, 4, 5], Exhaustion::Cycle)?)?;
    plan.bind_parameter(
        ArticulationTrack::try_new(
            "articulation",
            vec![Articulation::Normal, Articulation::Accent, Articulation::Marcato],
            Exhaustion::Cycle,
        )?,
    )?;
    plan.bind_parameter(TimbreTrack::try_new(
        "timbre",
        vec!["clarinet".to_owned(), "oboe".to_owned(), "horn".to_owned()],
        Exhaustion::Cycle,
    )?)?;
    plan.bind_parameter(ParameterTrack::try_new("bow-pressure", vec![1_u8, 3, 2, 4], Exhaustion::Cycle)?)?;

    let bound = plan.typed_parameter::<u8>("bow-pressure").expect("typed custom parameter");
    assert_eq!(bound.projection().steps()[..4].iter().map(|step| step.value).collect::<Vec<_>>(), vec![1, 3, 2, 4]);

    let transformed = DynamicsTrack::try_new("dynamics", vec![40, 60, 80, 100], Exhaustion::Cycle)?
        .transformed(&SeriesTransform::retrograde(4))?;
    assert_eq!(
        transformed.project(4).steps().iter().map(|step| step.value).collect::<Vec<_>>(),
        vec![100, 80, 60, 40]
    );
    Ok(())
}
