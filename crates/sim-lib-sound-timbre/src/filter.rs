use sim_lib_sound_core::{Amplitude, Frequency, Partial, Tone};

/// A spectral filter applied to a tone's partials.
#[derive(Clone, Debug, PartialEq)]
pub enum Filter {
    /// Attenuates partials above the cutoff frequency.
    LowPass {
        /// Cutoff frequency.
        cutoff: Frequency,
        /// Resonance (quality factor).
        q: f64,
    },
    /// Attenuates partials below the cutoff frequency.
    HighPass {
        /// Cutoff frequency.
        cutoff: Frequency,
        /// Resonance (quality factor).
        q: f64,
    },
    /// Passes a band around the center frequency, attenuating the rest.
    BandPass {
        /// Center frequency of the pass band.
        center: Frequency,
        /// Resonance (quality factor) controlling band width.
        q: f64,
        /// Pass-band gain.
        gain: Amplitude,
    },
    /// Attenuates a narrow band around the center frequency.
    Notch {
        /// Center frequency of the notch.
        center: Frequency,
        /// Resonance (quality factor) controlling notch width.
        q: f64,
    },
    /// Shapes the spectrum with one or more formant resonances.
    Formant {
        /// Formant bands as `(center, width, gain)` tuples.
        bands: Vec<(Frequency, f64, Amplitude)>,
    },
}

impl Filter {
    /// Returns `tone` with this filter applied to every partial.
    pub fn apply(&self, mut tone: Tone) -> Tone {
        tone.partials = tone
            .partials
            .into_iter()
            .map(|partial| self.apply_partial(partial))
            .collect();
        tone
    }

    fn apply_partial(&self, mut partial: Partial) -> Partial {
        match self {
            Self::LowPass { cutoff, .. } => {
                if partial.frequency.0 > cutoff.0 {
                    partial.amplitude.0 *= (cutoff.0 / partial.frequency.0).clamp(0.0, 1.0);
                }
            }
            Self::HighPass { cutoff, .. } => {
                if partial.frequency.0 < cutoff.0 {
                    partial.amplitude.0 *= (partial.frequency.0 / cutoff.0).clamp(0.0, 1.0);
                }
            }
            Self::BandPass { center, q, gain } => {
                let width = (center.0 / q.max(0.1)).max(1.0);
                let distance = (partial.frequency.0 - center.0).abs();
                let factor = (1.0 - distance / width).clamp(0.0, 1.0) * gain.0;
                partial.amplitude.0 *= factor;
            }
            Self::Notch { center, q } => {
                let width = (center.0 / q.max(0.1)).max(1.0);
                let distance = (partial.frequency.0 - center.0).abs();
                let notch = (distance / width).clamp(0.0, 1.0);
                partial.amplitude.0 *= notch;
            }
            Self::Formant { bands } => {
                let factor = bands.iter().fold(0.0_f64, |best, (center, width, gain)| {
                    let distance = (partial.frequency.0 - center.0).abs();
                    let local = (1.0 - distance / width.max(1.0)).clamp(0.0, 1.0) * gain.0;
                    best.max(local)
                });
                partial.amplitude.0 *= factor;
            }
        }
        partial
    }
}
