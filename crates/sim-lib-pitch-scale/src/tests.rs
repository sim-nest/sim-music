use super::*;
use sim_lib_discrete_search::{SearchControl, SearchStatus};
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_set::{PitchClassMask, PitchSetMovePolicy, ThirdStep};

#[test]
fn c_major_scale_degrees_match_expected_pitch_classes() {
    let scale = Scale::major(PitchClass::C);
    assert_eq!(
        scale.pitch_classes(),
        vec![
            PitchClass::C,
            PitchClass::D,
            PitchClass::E,
            PitchClass::F,
            PitchClass::G,
            PitchClass::A,
            PitchClass::B,
        ]
    );
    assert_eq!(scale.degree_of(PitchClass::G), Some(5));
    assert_eq!(scale.pitch_at_degree(7), Ok(PitchClass::B));
}

#[test]
fn degree_zero_is_rejected() {
    let scale = Scale::major(PitchClass::C);
    assert_eq!(
        scale.pitch_at_degree(0),
        Err(PitchScaleError::InvalidScaleDegree(0))
    );
    assert_eq!(
        PlayerScale::from_scale(scale).pitch_at_degree(0),
        Err(PitchScaleError::InvalidScaleDegree(0))
    );
    assert_eq!(
        Scale::chord_tone_to_scale_tone(0),
        Err(PitchScaleError::InvalidScaleDegree(0))
    );
    assert_eq!(
        Scale::scale_tone_to_diatonic(0),
        Err(PitchScaleError::InvalidScaleDegree(0))
    );
}

#[test]
fn diatonic_transpose_preserves_scale_membership() {
    let scale = Scale::major(PitchClass::C);
    let source = Pitch::from_midi(60);
    let transposed = scale.transpose_diatonic(source, 2).unwrap();
    assert_eq!(transposed.to_midi(), Some(64));
    assert!(scale.degree_of(transposed.class).is_some());
}

#[test]
fn relative_modes_share_a_mask() {
    let major = Scale::major(PitchClass::C).mask();
    let dorian = Scale::dorian(PitchClass::D).mask();
    assert_eq!(major, dorian);
}

#[test]
fn scale_lock_quantizes_to_nearest_scale_pitch() {
    let player =
        ScaleLockPlayer::from_scale(Scale::major(PitchClass::C), ScaleLockPolicy::Quantize);

    assert_eq!(
        player.process_pitch(Pitch::from_midi(61)),
        Some(Pitch::from_midi(62))
    );
    assert_eq!(
        player.process_pitch(Pitch::from_midi(63)),
        Some(Pitch::from_midi(64))
    );
}

#[test]
fn scale_lock_filters_outside_pitch_classes() {
    let player = ScaleLockPlayer::from_scale(Scale::major(PitchClass::C), ScaleLockPolicy::Filter);

    assert_eq!(
        player.process_pitches([
            Pitch::from_midi(60),
            Pitch::from_midi(61),
            Pitch::from_midi(62)
        ]),
        vec![Pitch::from_midi(60), Pitch::from_midi(62)]
    );
}

#[test]
fn scale_lock_remaps_chromatic_offsets_to_scale_degrees() {
    let player = ScaleLockPlayer::from_scale(Scale::major(PitchClass::C), ScaleLockPolicy::Remap);

    assert_eq!(
        player.process_pitches([
            Pitch::from_midi(60),
            Pitch::from_midi(61),
            Pitch::from_midi(63)
        ]),
        vec![
            Pitch::from_midi(60),
            Pitch::from_midi(62),
            Pitch::from_midi(65)
        ]
    );
}

#[test]
fn custom_scales_validate_and_keep_root_selection() {
    let scale = PlayerScale::custom(PitchClass::D, vec![0, 3, 5, 7, 10]).unwrap();
    assert_eq!(
        scale.pitch_classes(),
        vec![
            PitchClass::D,
            PitchClass::F,
            PitchClass::G,
            PitchClass::A,
            PitchClass::C,
        ]
    );
    assert_eq!(
        PlayerScale::custom(PitchClass::C, vec![0, 12]).unwrap_err(),
        PitchScaleError::InvalidScaleInterval(12)
    );
}

#[test]
fn tetrachord_generation_uses_stable_order_and_search_limits() {
    let tetrachords = [
        Tetrachord::new([0, 2, 4, 5]).unwrap(),
        Tetrachord::new([0, 2, 3, 5]).unwrap(),
    ];
    let joins = [TetrachordJoin::Whole, TetrachordJoin::Half];
    let run = generate_scales(
        &tetrachords,
        &joins,
        SearchControl::default()
            .with_max_work(10_000)
            .with_max_results(32)
            .with_seed(0),
    );

    assert_eq!(run.receipt.status, SearchStatus::Complete);
    assert_eq!(run.receipt.seed, 0);
    assert_eq!(
        run.outputs
            .iter()
            .map(|scale| scale.intervals.as_slice())
            .collect::<Vec<_>>(),
        vec![
            &[0, 2, 4, 5, 7, 9, 11][..],
            &[0, 2, 4, 5, 6, 8, 10, 11][..],
            &[0, 2, 4, 5, 7, 9, 10][..],
            &[0, 2, 4, 5, 6, 8, 9, 11][..],
            &[0, 2, 3, 5, 7, 9, 11][..],
            &[0, 2, 3, 5, 6, 8, 10, 11][..],
            &[0, 2, 3, 5, 7, 9, 10][..],
            &[0, 2, 3, 5, 6, 8, 9, 11][..],
        ]
    );

    let limited = generate_scales(
        &tetrachords,
        &joins,
        SearchControl::default()
            .with_max_work(10_000)
            .with_max_results(2)
            .with_seed(0),
    );
    assert_eq!(limited.receipt.status, SearchStatus::Partial);
    assert_eq!(limited.outputs.len(), 2);
}

#[test]
fn catalog_wxyz_scale_fixtures_are_preserved() {
    let expected = [
        ('w', &[0, 2, 4, 5, 7, 9, 11][..], "MmMmmMm"),
        ('x', &[0, 2, 4, 5, 7, 8, 11][..], "MmMmmmM"),
        ('y', &[0, 2, 4, 5, 8, 9, 11][..], "MMmmmMm"),
        ('z', &[0, 2, 3, 5, 7, 9, 11][..], "mMMmmMm"),
    ];

    for (fixture, (name, intervals, gap_text)) in SCALE_FIXTURES.iter().zip(expected) {
        assert_eq!(fixture.name, name);
        assert_eq!(fixture.intervals, intervals);
        assert_eq!(render_steps(fixture.third_stack_gaps), gap_text);
        let decoded = decode_scale_third_stack(fixture.intervals).unwrap();
        assert_eq!(decoded.steps, fixture.third_stack_gaps);
    }
}

#[test]
fn neighborhood_nudge_reuses_pitch_set_graph_controls() {
    let scale = Scale::major(PitchClass::C).mask();
    let neighbors = nudge_scale_neighborhood(
        scale,
        PitchSetMovePolicy::NonJumping,
        SearchControl::default()
            .with_max_work(100_000)
            .with_max_results(1_000),
    )
    .unwrap();

    assert!(neighbors.contains(&PitchClassMask::from_pitch_classes(&[
        PitchClass::CS,
        PitchClass::D,
        PitchClass::E,
        PitchClass::F,
        PitchClass::G,
        PitchClass::A,
        PitchClass::B,
    ])));
    assert!(neighbors.iter().all(|mask| mask.count_bits() == 7));
}

fn render_steps(steps: &[ThirdStep]) -> String {
    steps
        .iter()
        .map(|step| match step {
            ThirdStep::Minor => 'm',
            ThirdStep::Major => 'M',
        })
        .collect()
}
