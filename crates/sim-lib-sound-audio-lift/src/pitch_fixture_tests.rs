use sim_lib_pitch_core::Pitch;
use sim_lib_sound_core::Frequency;
use sim_lib_sound_tuning::EqualTemperament;

use crate::{
    AudioLiftOptions, AudioLifter, FftPeakLifter, PartialCrossingPolicy, PartialTrackPolicy,
    PitchFramePolicy, PitchFrameTail, PitchRange, PitchTrackControl, PitchTrackMethod,
    PitchTrackPlan, pitch_track, polyphonic_pitch_track, track_partials,
};

const SAMPLE_RATE: u32 = 8_000;

// conformance: generated audio covers silence, noise, vibrato, missing fundamentals, crossings, and tuning offsets.

#[test]
fn silence_and_seeded_noise_remain_unvoiced() {
    let plan = mono_plan();
    let silence = pitch_track(
        &vec![0.0; 2_048],
        SAMPLE_RATE,
        &EqualTemperament::default(),
        &plan,
    )
    .unwrap();
    assert!(silence.value.contour.iter().all(Option::is_none));

    let noise = (0..2_048)
        .map(|index| (0.25 * pseudo_noise(index)) as f32)
        .collect::<Vec<_>>();
    let noisy = pitch_track(&noise, SAMPLE_RATE, &EqualTemperament::default(), &plan).unwrap();
    assert!(
        noisy.value.contour.iter().all(Option::is_none),
        "{:?}",
        noisy.value.contour
    );
    assert!(
        noisy
            .value
            .frames
            .iter()
            .all(|frame| !frame.rejected.is_empty())
    );
}

#[test]
fn pyin_follows_generated_vibrato_without_quantizing_the_contour() {
    let samples = vibrato(440.0, 55.0, 5.0, SAMPLE_RATE, 4_096);
    let report = pitch_track(
        &samples,
        SAMPLE_RATE,
        &EqualTemperament::default(),
        &mono_plan(),
    )
    .unwrap();
    let frequencies = report
        .value
        .contour
        .iter()
        .flatten()
        .map(|estimate| estimate.frequency.0)
        .collect::<Vec<_>>();
    assert!(frequencies.len() >= 20, "{frequencies:?}");
    let min = frequencies.iter().copied().fold(f64::INFINITY, f64::min);
    let max = frequencies.iter().copied().fold(0.0_f64, f64::max);
    assert!(min < 430.0, "minimum {min}");
    assert!(max > 450.0, "maximum {max}");
    assert!(
        frequencies
            .iter()
            .all(|frequency| (410.0..470.0).contains(frequency))
    );
}

#[test]
fn harmonic_comb_recovers_a_generated_missing_fundamental() {
    let samples = harmonic_sweep(
        &[
            (440.0, 1.0, 440.0),
            (660.0, 0.8, 660.0),
            (880.0, 0.6, 880.0),
        ],
        SAMPLE_RATE,
        4_096,
    );
    let report = polyphonic_pitch_track(
        &samples,
        SAMPLE_RATE,
        &EqualTemperament::default(),
        &lift_options(),
        &PartialTrackPolicy {
            min_points: 3,
            ..PartialTrackPolicy::default()
        },
    )
    .unwrap();
    let fundamental = report.value.tracks.iter().find(|track| {
        let mean = track
            .points
            .iter()
            .map(|point| point.candidate.frequency.0)
            .sum::<f64>()
            / track.points.len() as f64;
        (mean - 220.0).abs() < 8.0
    });
    let fundamental = fundamental.expect("220 Hz missing-fundamental track");
    assert!(
        fundamental
            .points
            .iter()
            .any(|point| point.candidate.harmonic_count >= 3)
    );
    assert!(fundamental.confidence > 0.75, "{fundamental:?}");
}

#[test]
fn generated_crossing_partials_keep_two_bounded_trajectories() {
    let samples = harmonic_sweep(
        &[(220.0, 0.9, 440.0), (440.0, 0.8, 220.0)],
        SAMPLE_RATE,
        8_000,
    );
    let lifted = FftPeakLifter {
        opts: lift_options(),
    }
    .lift(&samples, SAMPLE_RATE, &EqualTemperament::default())
    .unwrap();
    let report = track_partials(
        &lifted.frames,
        SAMPLE_RATE,
        &PartialTrackPolicy {
            max_tracks: 8,
            max_jump_cents: 240.0,
            crossing: PartialCrossingPolicy::Allow,
            min_points: 8,
            max_work: 2_000_000,
            ..PartialTrackPolicy::default()
        },
    )
    .unwrap();
    let long = report
        .tracks
        .iter()
        .filter(|track| track.points.len() >= 10)
        .collect::<Vec<_>>();
    assert!(long.len() >= 2, "tracks: {:?}", report.tracks);
    let trends = long.iter().map(trend).collect::<Vec<_>>();
    assert!(
        trends.iter().any(|trend| *trend > 300.0),
        "trends: {trends:?}"
    );
    assert!(
        trends.iter().any(|trend| *trend < -300.0),
        "trends: {trends:?}"
    );
    assert!(report.work_used <= report.policy.max_work);
}

#[test]
fn tuning_offset_is_measured_against_the_supplied_reference() {
    let tuning = EqualTemperament {
        divisions: 12,
        reference: (Pitch::from_midi(69), Frequency(442.0)),
    };
    let samples = harmonic_sweep(&[(442.0, 1.0, 442.0)], SAMPLE_RATE, 2_048);
    let report = pitch_track(&samples, SAMPLE_RATE, &tuning, &mono_plan()).unwrap();
    let estimate = report.value.contour.iter().flatten().next().unwrap();
    assert!((estimate.frequency.0 - 442.0).abs() < 1.0);
    assert!(estimate.cents_error.abs() < 4.0, "{estimate:?}");
}

fn mono_plan() -> PitchTrackPlan {
    PitchTrackPlan {
        method: PitchTrackMethod::Pyin,
        range: PitchRange::new(100.0, 1_000.0).unwrap(),
        frames: PitchFramePolicy {
            size: 512,
            hop: 128,
            tail: PitchFrameTail::Drop,
        },
        control: PitchTrackControl {
            max_work: 10_000_000,
            ..PitchTrackControl::default()
        },
        ..PitchTrackPlan::default()
    }
}

fn lift_options() -> AudioLiftOptions {
    AudioLiftOptions {
        window_size: 1_024,
        hop_size: 256,
        max_peaks: 12,
        min_peak_ratio: 0.08,
        min_note_confidence: 0.25,
        min_note_windows: 2,
        ..AudioLiftOptions::default()
    }
}

fn vibrato(
    center_hz: f64,
    depth_cents: f64,
    rate_hz: f64,
    sample_rate: u32,
    len: usize,
) -> Vec<f32> {
    let mut phase = 0.0;
    (0..len)
        .map(|index| {
            let time = index as f64 / f64::from(sample_rate);
            let cents = depth_cents * (std::f64::consts::TAU * rate_hz * time).sin();
            let frequency = center_hz * 2.0_f64.powf(cents / 1_200.0);
            phase += std::f64::consts::TAU * frequency / f64::from(sample_rate);
            phase.sin() as f32
        })
        .collect()
}

fn harmonic_sweep(partials: &[(f64, f64, f64)], sample_rate: u32, len: usize) -> Vec<f32> {
    let mut phases = vec![0.0; partials.len()];
    (0..len)
        .map(|index| {
            let progress = index as f64 / len.saturating_sub(1).max(1) as f64;
            let value = partials
                .iter()
                .enumerate()
                .map(|(partial, (start, amplitude, end))| {
                    let frequency = start + (end - start) * progress;
                    phases[partial] += std::f64::consts::TAU * frequency / f64::from(sample_rate);
                    amplitude * phases[partial].sin()
                })
                .sum::<f64>();
            (value / partials.len().max(1) as f64) as f32
        })
        .collect()
}

fn trend(track: &&crate::PartialTrack) -> f64 {
    let first = track.points.first().unwrap().candidate.frequency.0;
    let last = track.points.last().unwrap().candidate.frequency.0;
    1_200.0 * (last / first).log2()
}

fn pseudo_noise(index: usize) -> f64 {
    let mut value = (index as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    (value >> 11) as f64 / (1_u64 << 53) as f64 * 2.0 - 1.0
}
