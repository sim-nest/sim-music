use sim_lib_pitch_core::Pitch;
use sim_lib_sound_core::{Amplitude, Frequency};
use sim_lib_sound_spectrum::{Spectrum, SpectrumSource};

use crate::{
    AudioLiftFrame, PartialCrossingPolicy, PartialDeathReason, PartialLinkRejectionReason,
    PartialTrackPolicy, PitchCandidate, track_partials,
};

// conformance: partial tracking retains bounded assignment, DTW, crossing, and provenance evidence.

#[test]
fn certified_assignment_can_follow_or_forbid_crossing_trajectories() {
    let frames = vec![
        frame(0, &[220.0, 440.0]),
        frame(1, &[300.0, 360.0]),
        frame(2, &[220.0, 440.0]),
    ];
    let base = PartialTrackPolicy {
        max_jump_cents: 1_200.0,
        birth_cost: 2_000.0,
        death_cost: 2_000.0,
        min_points: 3,
        ..PartialTrackPolicy::default()
    };
    let crossing = track_partials(
        &frames,
        8_000,
        &PartialTrackPolicy {
            crossing: PartialCrossingPolicy::Allow,
            ..base.clone()
        },
    )
    .unwrap();
    let ordered = track_partials(
        &frames,
        8_000,
        &PartialTrackPolicy {
            crossing: PartialCrossingPolicy::Forbid,
            ..base
        },
    )
    .unwrap();

    assert_eq!(crossing.tracks.len(), 2);
    assert_eq!(ordered.tracks.len(), 2);
    let rising = &crossing.tracks[0];
    assert_eq!(frequencies(rising), vec![220.0, 300.0, 440.0]);
    assert_eq!(frequencies(&ordered.tracks[0]), vec![220.0, 300.0, 220.0]);
    assert!(rising.continuity.receipt.work_used > 0);
    assert!(rising.continuity.steps >= rising.points.len());
    assert!(
        crossing.frames[1]
            .assignment_receipt
            .as_ref()
            .is_some_and(|receipt| receipt.work_used > 0)
    );
}

#[test]
fn birth_death_gap_and_track_caps_are_explicit() {
    let frames = vec![
        frame(0, &[220.0, 330.0, 440.0]),
        frame(1, &[]),
        frame(2, &[]),
    ];
    let report = track_partials(
        &frames,
        8_000,
        &PartialTrackPolicy {
            max_tracks: 2,
            max_gap_frames: 1,
            min_points: 1,
            ..PartialTrackPolicy::default()
        },
    )
    .unwrap();
    assert_eq!(report.tracks.len(), 2);
    assert!(
        report
            .tracks
            .iter()
            .all(|track| track.death == PartialDeathReason::GapLimit)
    );
    assert_eq!(report.frames[2].deaths.len(), 2);
    assert!(report.frames[0].rejected_links.iter().any(|rejected| {
        rejected.reason == PartialLinkRejectionReason::TrackLimit && rejected.track.is_none()
    }));
}

#[test]
fn jump_rejections_and_candidate_uncertainty_retain_frame_provenance() {
    let frames = vec![frame(0, &[220.0]), frame(1, &[880.0])];
    let report = track_partials(
        &frames,
        8_000,
        &PartialTrackPolicy {
            max_jump_cents: 100.0,
            min_points: 1,
            ..PartialTrackPolicy::default()
        },
    )
    .unwrap();
    let rejected = &report.frames[1].rejected_links[0];
    assert_eq!(rejected.reason, PartialLinkRejectionReason::JumpLimit);
    assert!(rejected.cents_distance.unwrap() > 2_300.0);
    let candidate = &report.frames[1].candidates[0];
    assert_eq!(candidate.provenance.frame_index, 1);
    assert_eq!(candidate.provenance.onset_sample, 256);
    assert!(candidate.lower_frequency.0 < candidate.frequency.0);
    assert!(candidate.upper_frequency.0 > candidate.frequency.0);
}

fn frame(index: usize, frequencies: &[f64]) -> AudioLiftFrame {
    AudioLiftFrame {
        index,
        onset_sample: index * 256,
        duration_samples: 1_024,
        spectrum: Spectrum {
            bins: Vec::new(),
            source: SpectrumSource::Synthetic,
        },
        pitch_candidates: frequencies
            .iter()
            .copied()
            .map(|frequency| PitchCandidate {
                pitch: Pitch::from_semitone(
                    (69.0 + 12.0 * (frequency / 440.0).log2()).round() as i32
                ),
                frequency: Frequency(frequency),
                amplitude: Amplitude(1.0),
                confidence: 0.9,
                cents_error: 0.0,
                harmonic_count: 3,
            })
            .collect(),
        diagnostics: Vec::new(),
    }
}

fn frequencies(track: &crate::PartialTrack) -> Vec<f64> {
    track
        .points
        .iter()
        .map(|point| point.candidate.frequency.0)
        .collect()
}
