use sim_lib_sound_tuning::EqualTemperament;

use crate::{
    AudioLiftError, PitchFramePolicy, PitchFrameTail, PitchRange, PitchRejectionReason,
    PitchTrackControl, PitchTrackMethod, PitchTrackPlan, pitch_track,
};

// conformance: YIN and pYIN retain interpolation, probability, range, framing, and work evidence.

#[test]
fn yin_and_pyin_interpolate_a_monophonic_tone_with_bounds() {
    let samples = sine(443.0, 8_000, 4_096);
    for method in [PitchTrackMethod::Yin, PitchTrackMethod::Pyin] {
        let plan = PitchTrackPlan {
            method,
            range: PitchRange::new(80.0, 1_000.0).unwrap(),
            frames: PitchFramePolicy {
                size: 1_024,
                hop: 512,
                tail: PitchFrameTail::Drop,
            },
            control: PitchTrackControl {
                max_work: 2_000_000,
                ..PitchTrackControl::default()
            },
            ..PitchTrackPlan::default()
        };
        let report = pitch_track(&samples, 8_000, &EqualTemperament::default(), &plan).unwrap();
        let estimate = report.value.contour[2].as_ref().expect("voiced estimate");
        assert!((estimate.frequency.0 - 443.0).abs() < 1.0, "{estimate:?}");
        assert!(estimate.lower_frequency.0 < estimate.frequency.0);
        assert!(estimate.upper_frequency.0 > estimate.frequency.0);
        assert!(estimate.voiced_probability >= plan.yin.min_voiced_probability);
        assert_eq!(estimate.pitch.to_midi(), Some(69));
        assert!(estimate.cents_error > 0.0);
        assert!(report.value.work_used <= plan.control.max_work);
    }
}

#[test]
fn frame_tail_and_silence_policies_are_retained_in_provenance() {
    let plan = PitchTrackPlan {
        range: PitchRange::new(100.0, 1_000.0).unwrap(),
        frames: PitchFramePolicy {
            size: 256,
            hop: 128,
            tail: PitchFrameTail::ZeroPad,
        },
        ..PitchTrackPlan::default()
    };
    let report = pitch_track(&[0.0; 300], 8_000, &EqualTemperament::default(), &plan).unwrap();
    assert_eq!(report.value.frames.len(), 2);
    let tail = &report.value.frames[1];
    assert!(tail.provenance.zero_padded);
    assert_eq!(tail.provenance.source_samples, 172);
    assert_eq!(tail.provenance.frame_size, 256);
    assert_eq!(tail.rejected[0].reason, PitchRejectionReason::Silence);
    assert!(tail.rejected[0].hypothesis.is_none());
}

#[test]
fn invalid_ranges_and_work_exhaustion_fail_closed() {
    assert_eq!(
        PitchRange::new(440.0, 55.0),
        Err(AudioLiftError::InvalidPitchRange)
    );
    let plan = PitchTrackPlan {
        range: PitchRange::new(100.0, 1_000.0).unwrap(),
        frames: PitchFramePolicy {
            size: 512,
            hop: 256,
            tail: PitchFrameTail::Drop,
        },
        control: PitchTrackControl {
            max_work: 1,
            ..PitchTrackControl::default()
        },
        ..PitchTrackPlan::default()
    };
    let error = pitch_track(
        &sine(440.0, 8_000, 512),
        8_000,
        &EqualTemperament::default(),
        &plan,
    )
    .unwrap_err();
    assert_eq!(error, AudioLiftError::PitchWorkLimit { limit: 1 });
}

fn sine(frequency: f64, sample_rate: u32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|index| {
            (std::f64::consts::TAU * frequency * index as f64 / f64::from(sample_rate)).sin() as f32
        })
        .collect()
}
