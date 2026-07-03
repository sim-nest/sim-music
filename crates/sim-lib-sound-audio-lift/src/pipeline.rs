use std::collections::BTreeMap;

use sim_kernel::{Diagnostic, Severity};
use sim_lib_pitch_core::Pitch;
use sim_lib_sound_core::{Amplitude, Frequency};
use sim_lib_sound_spectrum::Spectrum;
use sim_lib_sound_tuning::Tuning;

use crate::{
    AudioLiftError, AudioLiftFrame, AudioLiftOptions, AudioLiftReport, AudioLiftResult,
    AudioNoteCandidate, PitchCandidate,
};

pub(crate) fn analyze(
    samples: &[f32],
    sample_rate: u32,
    tuning: &dyn Tuning,
    opts: &AudioLiftOptions,
    harmonic_comb: bool,
) -> Result<AudioLiftReport<AudioLiftResult>, AudioLiftError> {
    if sample_rate == 0 {
        return Err(AudioLiftError::InvalidSampleRate);
    }
    opts.validate()?;

    if samples.is_empty() {
        return Ok(AudioLiftReport {
            value: AudioLiftResult::default(),
            diagnostics: vec![warning("audio lift received empty pcm buffer")],
        });
    }

    let mut frames = Vec::new();
    let mut diagnostics = Vec::new();
    for (index, onset) in frame_starts(samples.len(), opts.hop_size)
        .into_iter()
        .enumerate()
    {
        let end = (onset + opts.window_size).min(samples.len());
        let window = &samples[onset..end];
        let spectrum = Spectrum::from_pcm(window, sample_rate, opts.window_size);
        let peaks = detect_peaks(&spectrum, opts);
        let mut frame_diagnostics = Vec::new();
        if peaks.is_empty() {
            let message = format!("frame {index} contained no stable spectral peaks");
            diagnostics.push(warning(message.clone()));
            frame_diagnostics.push(message);
        }
        let candidates = if harmonic_comb {
            harmonic_candidates(&spectrum, &peaks, tuning, opts)
        } else {
            fft_candidates(&spectrum, &peaks, tuning)
        };
        if candidates.is_empty() && !peaks.is_empty() {
            let message = format!("frame {index} peaks did not map cleanly onto pitch space");
            diagnostics.push(warning(message.clone()));
            frame_diagnostics.push(message);
        }
        frames.push(AudioLiftFrame {
            index,
            onset_sample: onset,
            duration_samples: end - onset,
            spectrum,
            pitch_candidates: candidates,
            diagnostics: frame_diagnostics,
        });
        if end == samples.len() {
            break;
        }
    }

    let notes = connect_notes(&frames, sample_rate, opts, &mut diagnostics);
    if notes.is_empty() {
        diagnostics.push(warning("audio lift produced no note candidates"));
    }

    Ok(AudioLiftReport {
        value: AudioLiftResult { frames, notes },
        diagnostics,
    })
}

fn frame_starts(len: usize, hop_size: usize) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut onset = 0usize;
    while onset < len {
        starts.push(onset);
        onset = onset.saturating_add(hop_size);
    }
    if starts.is_empty() {
        starts.push(0);
    }
    starts
}

#[derive(Copy, Clone, Debug)]
struct Peak {
    frequency: Frequency,
    amplitude: Amplitude,
}

fn detect_peaks(spectrum: &Spectrum, opts: &AudioLiftOptions) -> Vec<Peak> {
    let Spectrum {
        bins,
        source: _source,
    } = spectrum;
    if bins.len() < 3 {
        return Vec::new();
    }
    let max_amplitude = bins.iter().map(|(_, amp)| amp.0).fold(0.0_f64, f64::max);
    if max_amplitude <= f64::EPSILON {
        return Vec::new();
    }

    let mut peaks = Vec::new();
    for index in 1..(bins.len() - 1) {
        let left = bins[index - 1].1.0;
        let mid = bins[index].1.0;
        let right = bins[index + 1].1.0;
        if mid < left || mid < right || mid < max_amplitude * opts.min_peak_ratio {
            continue;
        }
        let base_frequency = bins[index].0.0;
        if base_frequency <= 0.0 {
            continue;
        }
        let step = bins[index].0.0 - bins[index - 1].0.0;
        let delta = parabolic_delta(left, mid, right);
        peaks.push(Peak {
            frequency: Frequency(base_frequency + delta * step),
            amplitude: bins[index].1,
        });
    }

    peaks.sort_by(|left, right| right.amplitude.0.total_cmp(&left.amplitude.0));
    let mut deduped = Vec::new();
    for peak in peaks {
        let is_duplicate = deduped.iter().any(|seen: &Peak| {
            let cents = peak.frequency.cents_above(seen.frequency).abs();
            cents < 30.0
        });
        if !is_duplicate {
            deduped.push(peak);
        }
        if deduped.len() == opts.max_peaks {
            break;
        }
    }
    deduped
}

fn parabolic_delta(left: f64, center: f64, right: f64) -> f64 {
    let denom = left - 2.0 * center + right;
    if denom.abs() <= f64::EPSILON {
        0.0
    } else {
        (0.5 * (left - right) / denom).clamp(-1.0, 1.0)
    }
}

fn fft_candidates(spectrum: &Spectrum, peaks: &[Peak], tuning: &dyn Tuning) -> Vec<PitchCandidate> {
    let tonality = (1.0 - spectrum.flatness()).clamp(0.0, 1.0);
    let max_amp = peaks
        .iter()
        .map(|peak| peak.amplitude.0)
        .fold(0.0_f64, f64::max);
    let mut best_by_pitch = BTreeMap::<i32, PitchCandidate>::new();
    for peak in peaks {
        let pitch = tuning.pitch_of(peak.frequency);
        let tuned = tuning.frequency_of(pitch);
        let cents_error = peak.frequency.cents_above(tuned);
        let amplitude_norm = if max_amp <= f64::EPSILON {
            0.0
        } else {
            peak.amplitude.0 / max_amp
        };
        let alignment = (1.0 - (cents_error.abs() / 80.0)).clamp(0.0, 1.0);
        let confidence = ((0.6 * amplitude_norm) + (0.4 * tonality)) * alignment;
        let candidate = PitchCandidate {
            pitch,
            frequency: peak.frequency,
            amplitude: peak.amplitude,
            confidence: confidence.clamp(0.0, 1.0),
            cents_error,
            harmonic_count: 1,
        };
        let key = pitch.semitone();
        if best_by_pitch
            .get(&key)
            .is_none_or(|prev| prev.confidence < candidate.confidence)
        {
            best_by_pitch.insert(key, candidate);
        }
    }
    let mut candidates = best_by_pitch.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
    candidates
}

fn harmonic_candidates(
    spectrum: &Spectrum,
    peaks: &[Peak],
    tuning: &dyn Tuning,
    opts: &AudioLiftOptions,
) -> Vec<PitchCandidate> {
    let mut candidates = fft_candidates(spectrum, peaks, tuning);
    for candidate in &mut candidates {
        let mut harmonic_count = 0usize;
        let mut harmonic_score = 0.0;
        for harmonic in 1..=6 {
            let expected = Frequency(candidate.frequency.0 * harmonic as f64);
            if let Some(peak) = peaks.iter().find(|peak| {
                peak.frequency.cents_above(expected).abs() <= opts.harmonic_tolerance_cents
            }) {
                harmonic_count += 1;
                harmonic_score += peak.amplitude.0 / harmonic as f64;
            }
        }
        candidate.harmonic_count = harmonic_count.max(1);
        let comb_bonus = (harmonic_score / candidate.amplitude.0.max(1e-9)).min(3.0) / 3.0;
        let boosted = candidate.confidence + 0.18 * comb_bonus;
        candidate.confidence = boosted.clamp(candidate.confidence, 1.0);
    }
    candidates.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
    candidates
}

#[derive(Clone, Debug)]
struct ActiveNote {
    track: usize,
    onset_sample: usize,
    last_end_sample: usize,
    pitch: Pitch,
    sum_frequency: f64,
    sum_amplitude: f64,
    sum_confidence: f64,
    windows: usize,
}

fn connect_notes(
    frames: &[AudioLiftFrame],
    sample_rate: u32,
    opts: &AudioLiftOptions,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<AudioNoteCandidate> {
    let mut active = BTreeMap::<i32, ActiveNote>::new();
    let mut notes = Vec::new();
    let mut next_track = 0usize;

    for frame in frames {
        let mut seen = Vec::new();
        for candidate in frame
            .pitch_candidates
            .iter()
            .filter(|candidate| candidate.confidence >= opts.min_note_confidence)
        {
            let key = candidate.pitch.semitone();
            seen.push(key);
            let entry = active.entry(key).or_insert_with(|| {
                let track = next_track;
                next_track += 1;
                ActiveNote {
                    track,
                    onset_sample: frame.onset_sample,
                    last_end_sample: frame.onset_sample + frame.duration_samples,
                    pitch: candidate.pitch,
                    sum_frequency: 0.0,
                    sum_amplitude: 0.0,
                    sum_confidence: 0.0,
                    windows: 0,
                }
            });
            entry.last_end_sample = frame.onset_sample + frame.duration_samples;
            entry.sum_frequency += candidate.frequency.0;
            entry.sum_amplitude += candidate.amplitude.0;
            entry.sum_confidence += candidate.confidence;
            entry.windows += 1;
        }

        let stale = active
            .keys()
            .copied()
            .filter(|key| !seen.contains(key))
            .collect::<Vec<_>>();
        for key in stale {
            if let Some(note) = active.remove(&key) {
                finalize_note(note, sample_rate, opts, diagnostics, &mut notes);
            }
        }
    }

    for note in active.into_values() {
        finalize_note(note, sample_rate, opts, diagnostics, &mut notes);
    }
    notes.sort_by(|left, right| {
        left.onset_sample
            .cmp(&right.onset_sample)
            .then_with(|| left.track.cmp(&right.track))
    });
    notes
}

fn finalize_note(
    note: ActiveNote,
    sample_rate: u32,
    opts: &AudioLiftOptions,
    diagnostics: &mut Vec<Diagnostic>,
    out: &mut Vec<AudioNoteCandidate>,
) {
    if note.windows < opts.min_note_windows {
        diagnostics.push(warning(format!(
            "discarded unstable lifted note {} after {} window(s)",
            note.pitch.semitone(),
            note.windows
        )));
        return;
    }
    let diagnostics_text = if note.windows == 1 {
        vec!["single-frame estimate".to_owned()]
    } else {
        Vec::new()
    };
    out.push(AudioNoteCandidate {
        track: note.track,
        onset_sample: note.onset_sample,
        duration_samples: note.last_end_sample.saturating_sub(note.onset_sample),
        sample_rate,
        pitch: note.pitch,
        mean_frequency: Frequency(note.sum_frequency / note.windows as f64),
        mean_amplitude: Amplitude(note.sum_amplitude / note.windows as f64),
        confidence: (note.sum_confidence / note.windows as f64).clamp(0.0, 1.0),
        diagnostics: diagnostics_text,
    });
}

fn warning(message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message: message.into(),
        source: None,
        span: None,
        code: None,
        related: Vec::new(),
    }
}
