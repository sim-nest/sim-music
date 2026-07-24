use std::time::Duration;

use sim_lib_sound_core::{
    Amplitude, Envelope, EnvelopeShape, Frequency, Partial, PartialTag, Phase, Tone,
};

use crate::{
    MergePolicy, SampleInterpolation, SamplePitchPolicy, SampledPartial, TimbreRecipe,
    TimbreRenderError,
};

pub(crate) fn render_recipe(
    recipe: &TimbreRecipe,
    frequency: Frequency,
    duration: Duration,
) -> Result<Tone, TimbreRenderError> {
    match recipe {
        TimbreRecipe::PureSine => Ok(Tone::sine(frequency, duration)),
        TimbreRecipe::Sawtooth { partials } => Ok(Tone::sawtooth(frequency, duration, *partials)),
        TimbreRecipe::Square { partials } => Ok(Tone::square(frequency, duration, *partials)),
        TimbreRecipe::Triangle { partials } => Ok(Tone::triangle(frequency, duration, *partials)),
        TimbreRecipe::OrganPipe { stops } => organ_pipe_tone(stops, frequency, duration),
        TimbreRecipe::KarplusStrong { damping } => karplus_tone(*damping, frequency, duration),
        TimbreRecipe::FmPair {
            modulator_ratio,
            index,
        } => fm_tone(*modulator_ratio, *index, frequency, duration),
        TimbreRecipe::BellInharmonic { ratios } => bell_tone(ratios, frequency, duration),
        TimbreRecipe::TaggedPartials { partials } => {
            sampled_partials_to_tone(partials, frequency, 1.0, duration)
        }
        TimbreRecipe::HarmonicExpansion {
            partials,
            amplitude_decay,
            phase_step,
        } => numbered_tone(
            frequency,
            *partials,
            *amplitude_decay,
            *phase_step,
            PartialDirection::Harmonic,
            duration,
        ),
        TimbreRecipe::UndertoneExpansion {
            partials,
            amplitude_decay,
            phase_step,
        } => numbered_tone(
            frequency,
            *partials,
            *amplitude_decay,
            *phase_step,
            PartialDirection::Undertone,
            duration,
        ),
        TimbreRecipe::Sampled {
            root,
            partials,
            interpolation,
            pitch_policy,
        } => sampled_tone(
            *root,
            partials,
            *interpolation,
            *pitch_policy,
            frequency,
            duration,
        ),
        TimbreRecipe::Layered {
            primary,
            secondary,
            mix,
            policy,
        } => {
            let primary = render_recipe(primary, frequency, duration)?;
            let secondary = render_recipe(secondary, frequency, duration)?;
            Ok(merge_tones(primary, secondary, *mix, *policy))
        }
    }
}

pub(crate) fn render_recipe_lossy(
    recipe: &TimbreRecipe,
    frequency: Frequency,
    duration: Duration,
) -> Tone {
    match recipe {
        TimbreRecipe::Sampled {
            root,
            partials,
            interpolation,
            ..
        } => {
            let expanded = interpolate_sampled_partials(partials, *interpolation);
            sampled_partials_to_tone(&expanded, *root, 1.0, duration)
                .unwrap_or_else(|_| Tone::sine(frequency, duration))
        }
        other => render_recipe(other, frequency, duration)
            .unwrap_or_else(|_| Tone::sine(frequency, duration)),
    }
}

pub(crate) fn recipe_fingerprint(recipe: &TimbreRecipe) -> String {
    format!("{recipe:?}")
}

pub(crate) fn default_env() -> Envelope {
    Envelope::new(
        Duration::from_millis(15),
        Duration::from_millis(60),
        0.75,
        Duration::from_millis(120),
        EnvelopeShape::Linear,
    )
    .expect("default timbre envelope")
}

fn organ_pipe_tone(
    stops: &[f64],
    frequency: Frequency,
    duration: Duration,
) -> Result<Tone, TimbreRenderError> {
    let partials = stops
        .iter()
        .enumerate()
        .map(|(index, stop)| Partial {
            frequency: Frequency(frequency.0 * stop.max(0.25)),
            amplitude: Amplitude(1.0 / (index + 1) as f64),
            phase: Phase(0.0),
            tag: PartialTag::Harmonic((index + 1) as u32),
        })
        .collect();
    Ok(Tone::from_partials(partials, default_env(), duration).expect("organ tone"))
}

fn karplus_tone(
    damping: f64,
    frequency: Frequency,
    duration: Duration,
) -> Result<Tone, TimbreRenderError> {
    let partials = (1..=8)
        .map(|n| Partial {
            frequency: Frequency(frequency.0 * n as f64),
            amplitude: Amplitude(damping.powi(n).clamp(0.0, 1.0)),
            phase: Phase(0.0),
            tag: PartialTag::Harmonic(n as u32),
        })
        .collect();
    Ok(Tone::from_partials(partials, default_env(), duration).expect("karplus strong tone"))
}

fn fm_tone(
    modulator_ratio: f64,
    index: f64,
    frequency: Frequency,
    duration: Duration,
) -> Result<Tone, TimbreRenderError> {
    let partials = vec![
        Partial {
            frequency,
            amplitude: Amplitude(1.0),
            phase: Phase(0.0),
            tag: PartialTag::Source,
        },
        Partial {
            frequency: Frequency(frequency.0 * modulator_ratio),
            amplitude: Amplitude((index / 2.0).max(0.0)),
            phase: Phase(0.0),
            tag: PartialTag::Harmonic(1),
        },
        Partial {
            frequency: Frequency(frequency.0 * (1.0 + modulator_ratio)),
            amplitude: Amplitude((index / 3.0).max(0.0)),
            phase: Phase(0.0),
            tag: PartialTag::Harmonic(2),
        },
    ];
    Ok(Tone::from_partials(partials, default_env(), duration).expect("fm tone"))
}

fn bell_tone(
    ratios: &[f64],
    frequency: Frequency,
    duration: Duration,
) -> Result<Tone, TimbreRenderError> {
    let partials = ratios
        .iter()
        .enumerate()
        .map(|(index, ratio)| Partial {
            frequency: Frequency(frequency.0 * ratio),
            amplitude: Amplitude(1.0 / (index + 1) as f64),
            phase: Phase(0.0),
            tag: PartialTag::Harmonic((index + 1) as u32),
        })
        .collect();
    Ok(Tone::from_partials(partials, default_env(), duration).expect("bell tone"))
}

fn sampled_tone(
    root: Frequency,
    partials: &[SampledPartial],
    interpolation: SampleInterpolation,
    pitch_policy: SamplePitchPolicy,
    frequency: Frequency,
    duration: Duration,
) -> Result<Tone, TimbreRenderError> {
    let ratio = match pitch_policy {
        SamplePitchPolicy::Reject if frequency != root => {
            return Err(TimbreRenderError::SamplePitchRejected);
        }
        SamplePitchPolicy::Reject | SamplePitchPolicy::Clamp => 1.0,
        SamplePitchPolicy::Resample => frequency.0 / root.0,
    };
    let expanded = interpolate_sampled_partials(partials, interpolation);
    sampled_partials_to_tone(&expanded, root, ratio, duration)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PartialDirection {
    Harmonic,
    Undertone,
}

fn numbered_tone(
    root: Frequency,
    count: usize,
    amplitude_decay: f64,
    phase_step: f64,
    direction: PartialDirection,
    duration: Duration,
) -> Result<Tone, TimbreRenderError> {
    let partials = (1..=count)
        .filter_map(|index| {
            let ordinal = u32::try_from(index).ok()?;
            let ratio = match direction {
                PartialDirection::Harmonic => index as f64,
                PartialDirection::Undertone => 1.0 / index as f64,
            };
            let tag = match direction {
                PartialDirection::Harmonic => PartialTag::Harmonic(ordinal),
                PartialDirection::Undertone => PartialTag::Undertone(ordinal),
            };
            Partial::tagged(
                Frequency(root.0 * ratio),
                Amplitude(amplitude_decay.max(0.0).powi(index as i32)),
                Phase(phase_step * index as f64),
                tag,
            )
            .ok()
        })
        .collect();
    Tone::from_partials(partials, default_env(), duration)
        .map_err(|_| TimbreRenderError::EmptySample)
}

fn interpolate_sampled_partials(
    partials: &[SampledPartial],
    interpolation: SampleInterpolation,
) -> Vec<SampledPartial> {
    let mut ordered = partials
        .iter()
        .copied()
        .filter(|partial| partial.ratio.is_finite() && partial.ratio > 0.0)
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.ratio.total_cmp(&right.ratio));
    if !matches!(interpolation, SampleInterpolation::Linear) || ordered.len() < 2 {
        return ordered;
    }
    let mut expanded = Vec::with_capacity(ordered.len().saturating_mul(2).saturating_sub(1));
    for window in ordered.windows(2) {
        let left = window[0];
        let right = window[1];
        expanded.push(left);
        expanded.push(SampledPartial {
            ratio: (left.ratio + right.ratio) * 0.5,
            amplitude: Amplitude((left.amplitude.0 + right.amplitude.0) * 0.5),
            phase: Phase((left.phase.0 + right.phase.0) * 0.5),
            tag: left.tag,
        });
    }
    if let Some(last) = ordered.last() {
        expanded.push(*last);
    }
    expanded
}

fn sampled_partials_to_tone(
    partials: &[SampledPartial],
    root: Frequency,
    pitch_ratio: f64,
    duration: Duration,
) -> Result<Tone, TimbreRenderError> {
    let partials = partials
        .iter()
        .filter_map(|partial| {
            Partial::tagged(
                Frequency(root.0 * pitch_ratio * partial.ratio),
                partial.amplitude,
                partial.phase,
                partial.tag,
            )
            .ok()
        })
        .collect::<Vec<_>>();
    Tone::from_partials(partials, default_env(), duration)
        .map_err(|_| TimbreRenderError::EmptySample)
}

fn merge_tones(primary: Tone, secondary: Tone, mix: f64, policy: MergePolicy) -> Tone {
    let primary_gain = 1.0 - mix.clamp(0.0, 1.0);
    let secondary_gain = mix.clamp(0.0, 1.0);
    if matches!(policy, MergePolicy::PreservePartials) {
        return primary.amplify(primary_gain) + secondary.amplify(secondary_gain);
    }

    let mut partials = primary
        .partials
        .into_iter()
        .map(|mut partial| {
            partial.amplitude.0 *= primary_gain;
            partial
        })
        .collect::<Vec<_>>();
    for mut incoming in secondary.partials {
        incoming.amplitude.0 *= secondary_gain;
        if let Some(existing) = partials.iter_mut().find(|candidate| {
            candidate.tag == incoming.tag
                && (candidate.frequency.0 - incoming.frequency.0).abs() <= 1.0e-9
        }) {
            let existing_amp = existing.amplitude.0;
            existing.amplitude.0 += incoming.amplitude.0;
            match policy {
                MergePolicy::SumCoincidentPreferLoudestPhase
                    if incoming.amplitude.0 > existing_amp =>
                {
                    existing.phase = incoming.phase;
                }
                MergePolicy::SumCoincidentResetPhase => existing.phase = Phase(0.0),
                _ => {}
            }
        } else {
            partials.push(incoming);
        }
    }
    Tone::from_partials(partials, primary.envelope, primary.duration).expect("merged tone")
}
