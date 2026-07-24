use std::time::Duration;

use sim_lib_sound_core::{Amplitude, Envelope, Frequency, PartialTag, Phase, Tone};

use crate::{
    Filter, TimbreCache, TimbreCacheKey,
    render::{default_env, recipe_fingerprint, render_recipe, render_recipe_lossy},
};

/// The character of a timbre's onset.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AttackKind {
    /// Gradual onset, as in flutes or organs.
    Soft,
    /// Sharp plucked onset, as in guitars or harpsichords.
    Plucked,
    /// Sustained bowed onset, as in strings.
    Bowed,
    /// Percussive struck onset, as in bells or mallets.
    Struck,
}

/// Descriptive metadata characterizing a [`Timbre`].
#[derive(Clone, Debug, PartialEq)]
pub struct TimbreMeta {
    /// Relative spectral brightness (higher is brighter).
    pub brightness: f64,
    /// Relative perceived roughness in `0.0..`.
    pub roughness: f64,
    /// Onset character of the timbre.
    pub attack_kind: AttackKind,
    /// Coarse instrument family label.
    pub category: String,
}

/// Interpolation curve used between neighboring sampled partial breakpoints.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SampleInterpolation {
    /// Use the lower breakpoint unchanged until the next breakpoint.
    Step,
    /// Linearly interpolate amplitude and phase between breakpoints.
    Linear,
}

/// Policy applied when a sampled timbre is requested away from its root pitch.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SamplePitchPolicy {
    /// Reject non-root render requests through [`Timbre::try_render`].
    Reject,
    /// Clamp to the declared root pitch while preserving partial ratios.
    Clamp,
    /// Resample the declared spectrum by the requested/root frequency ratio.
    Resample,
}

/// Amplitude and phase captured at one normalized frequency ratio.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SampledPartial {
    /// Frequency ratio relative to the sampled root pitch.
    pub ratio: f64,
    /// Linear amplitude at this breakpoint.
    pub amplitude: Amplitude,
    /// Starting phase at this breakpoint.
    pub phase: Phase,
    /// Semantic source tag retained when the sample is rendered.
    pub tag: PartialTag,
}

/// Error raised when a timbre cannot be rendered under its declared policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimbreRenderError {
    /// A sampled timbre using [`SamplePitchPolicy::Reject`] was requested at a
    /// pitch other than its root.
    SamplePitchRejected,
    /// A sampled timbre had no usable partial breakpoints.
    EmptySample,
}

impl std::fmt::Display for TimbreRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SamplePitchRejected => f.write_str("sampled timbre rejects non-root pitch"),
            Self::EmptySample => f.write_str("sampled timbre has no usable partials"),
        }
    }
}

impl std::error::Error for TimbreRenderError {}

/// A synthesis recipe describing how to build the partials of a tone.
#[derive(Clone, Debug, PartialEq)]
pub enum TimbreRecipe {
    /// A single sinusoid.
    PureSine,
    /// A sawtooth built from the given number of harmonics.
    Sawtooth {
        /// Number of harmonic partials.
        partials: usize,
    },
    /// A square wave built from the given number of harmonics.
    Square {
        /// Number of harmonic partials.
        partials: usize,
    },
    /// A triangle wave built from the given number of harmonics.
    Triangle {
        /// Number of harmonic partials.
        partials: usize,
    },
    /// An organ-pipe blend of harmonic stops at the given frequency multiples.
    OrganPipe {
        /// Frequency multipliers for each pipe stop.
        stops: Vec<f64>,
    },
    /// A plucked-string model with the given per-harmonic damping factor.
    KarplusStrong {
        /// Per-harmonic amplitude damping factor.
        damping: f64,
    },
    /// A two-operator frequency-modulation pair.
    FmPair {
        /// Modulator-to-carrier frequency ratio.
        modulator_ratio: f64,
        /// Modulation index controlling sideband strength.
        index: f64,
    },
    /// An inharmonic bell spectrum at the given partial ratios.
    BellInharmonic {
        /// Frequency ratios of the inharmonic partials.
        ratios: Vec<f64>,
    },
    /// Expands explicitly tagged harmonic or undertone partials.
    TaggedPartials {
        /// Root-normalized source partials.
        partials: Vec<SampledPartial>,
    },
    /// Harmonic expansion with caller-declared amplitude and phase policy.
    HarmonicExpansion {
        /// Number of harmonic partials.
        partials: usize,
        /// Amplitude multiplier applied at each harmonic number.
        amplitude_decay: f64,
        /// Phase advance in radians per harmonic number.
        phase_step: f64,
    },
    /// Undertone expansion with caller-declared amplitude and phase policy.
    UndertoneExpansion {
        /// Number of undertone partials.
        partials: usize,
        /// Amplitude multiplier applied at each undertone number.
        amplitude_decay: f64,
        /// Phase advance in radians per undertone number.
        phase_step: f64,
    },
    /// Root-normalized sampled spectrum with declared interpolation and pitch
    /// policy.
    Sampled {
        /// Root pitch for the captured sample.
        root: Frequency,
        /// Root-normalized partial breakpoints.
        partials: Vec<SampledPartial>,
        /// Interpolation applied across the breakpoints.
        interpolation: SampleInterpolation,
        /// Out-of-range pitch policy.
        pitch_policy: SamplePitchPolicy,
    },
    /// A mix of two recipes.
    Layered {
        /// Primary recipe, weighted by `1.0 - mix`.
        primary: Box<TimbreRecipe>,
        /// Secondary recipe, weighted by `mix`.
        secondary: Box<TimbreRecipe>,
        /// Blend ratio in `0.0..=1.0`.
        mix: f64,
        /// Phase and amplitude policy used when combining coincident partials.
        policy: MergePolicy,
    },
}

/// How partials are combined when two timbres are layered.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MergePolicy {
    /// Keep all partials in source order after applying the layer amplitudes.
    PreservePartials,
    /// Sum amplitudes for matching frequency/tag pairs and keep the stronger
    /// partial's phase.
    SumCoincidentPreferLoudestPhase,
    /// Sum amplitudes for matching frequency/tag pairs and reset phase to zero.
    SumCoincidentResetPhase,
}

/// A named instrument timbre: a synthesis recipe plus a default envelope,
/// descriptive metadata, and a post-synthesis filter chain.
#[derive(Clone, Debug, PartialEq)]
pub struct Timbre {
    /// Identifier of the timbre.
    pub name: String,
    /// Synthesis recipe used to generate partials.
    pub recipe: TimbreRecipe,
    /// Envelope applied to rendered tones.
    pub default_envelope: Envelope,
    /// Descriptive metadata.
    pub metadata: TimbreMeta,
    /// Filters applied after synthesis.
    pub filters: Vec<Filter>,
}

impl Timbre {
    /// Renders a [`Tone`] at `frequency` for `duration`, applying the default
    /// envelope and the filter chain.
    pub fn render(&self, frequency: Frequency, duration: Duration) -> Tone {
        self.try_render(frequency, duration)
            .unwrap_or_else(|_| render_recipe_lossy(&self.recipe, frequency, duration))
    }

    /// Renders a [`Tone`], returning an error when the recipe's declared policy
    /// rejects the requested pitch.
    pub fn try_render(
        &self,
        frequency: Frequency,
        duration: Duration,
    ) -> Result<Tone, TimbreRenderError> {
        let mut tone = render_recipe(&self.recipe, frequency, duration)?;
        tone.envelope = self.default_envelope.clone();
        for filter in &self.filters {
            tone = filter.apply(tone);
        }
        Ok(tone)
    }

    /// Renders with a caller-owned deterministic cache.
    pub fn render_cached(
        &self,
        frequency: Frequency,
        duration: Duration,
        cache: &mut TimbreCache,
    ) -> Result<Tone, TimbreRenderError> {
        let key = TimbreCacheKey {
            name: self.name.clone(),
            recipe: recipe_fingerprint(&self.recipe),
            frequency_bits: frequency.0.to_bits(),
            duration_nanos: duration.as_nanos(),
        };
        if let Some(tone) = cache.get(&key) {
            return Ok(tone);
        }
        let tone = self.try_render(frequency, duration)?;
        cache.insert(key, tone.clone());
        Ok(tone)
    }

    /// Returns a hybrid timbre layering `self` and `other` at blend ratio
    /// `mix`, concatenating their filter chains.
    pub fn layer(self, other: Timbre, mix: f64) -> Timbre {
        self.layer_with_policy(other, mix, MergePolicy::PreservePartials)
    }

    /// Returns a hybrid timbre layering `self` and `other` with an explicit
    /// partial phase/amplitude merge policy.
    pub fn layer_with_policy(self, other: Timbre, mix: f64, policy: MergePolicy) -> Timbre {
        let mut filters = self.filters.clone();
        filters.extend(other.filters.clone());
        Timbre {
            name: format!("{}+{}", self.name, other.name),
            recipe: TimbreRecipe::Layered {
                primary: Box::new(self.recipe),
                secondary: Box::new(other.recipe),
                mix,
                policy,
            },
            default_envelope: self.default_envelope,
            metadata: TimbreMeta {
                brightness: (self.metadata.brightness + other.metadata.brightness) / 2.0,
                roughness: (self.metadata.roughness + other.metadata.roughness) / 2.0,
                attack_kind: self.metadata.attack_kind,
                category: "hybrid".to_owned(),
            },
            filters,
        }
    }

    /// Returns the timbre with its default envelope replaced by `env`.
    pub fn with_envelope(mut self, env: Envelope) -> Timbre {
        self.default_envelope = env;
        self
    }

    /// Returns the timbre with `filter` appended to its filter chain.
    pub fn with_filter(mut self, filter: Filter) -> Timbre {
        self.filters.push(filter);
        self
    }
}

/// Returns a pure-sine timbre.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use sim_lib_sound_core::Frequency;
/// use sim_lib_sound_timbre::pure_sine;
///
/// let tone = pure_sine().render(Frequency(440.0), Duration::from_millis(500));
/// assert_eq!(tone.partials.len(), 1);
/// ```
pub fn pure_sine() -> Timbre {
    Timbre {
        name: "pure_sine".to_owned(),
        recipe: TimbreRecipe::PureSine,
        default_envelope: default_env(),
        metadata: TimbreMeta {
            brightness: 1.0,
            roughness: 0.0,
            attack_kind: AttackKind::Soft,
            category: "pure".to_owned(),
        },
        filters: Vec::new(),
    }
}

/// Returns a sawtooth timbre with the given number of harmonics.
pub fn sawtooth(partials: usize) -> Timbre {
    harmonic_timbre("sawtooth", TimbreRecipe::Sawtooth { partials }, 3.5)
}

/// Returns a square-wave timbre with the given number of harmonics.
pub fn square(partials: usize) -> Timbre {
    harmonic_timbre("square", TimbreRecipe::Square { partials }, 2.8)
}

/// Returns a triangle-wave timbre with the given number of harmonics.
pub fn triangle(partials: usize) -> Timbre {
    harmonic_timbre("triangle", TimbreRecipe::Triangle { partials }, 2.0)
}

/// Returns an organ-pipe timbre blending the given harmonic stops.
pub fn organ_pipe(stops: &[f64]) -> Timbre {
    Timbre {
        name: "organ_pipe".to_owned(),
        recipe: TimbreRecipe::OrganPipe {
            stops: stops.to_vec(),
        },
        default_envelope: default_env(),
        metadata: TimbreMeta {
            brightness: 2.4,
            roughness: 0.15,
            attack_kind: AttackKind::Soft,
            category: "wind".to_owned(),
        },
        filters: Vec::new(),
    }
}

/// Returns a Karplus-Strong plucked-string timbre with the given damping.
pub fn karplus_strong(damping: f64) -> Timbre {
    Timbre {
        name: "karplus_strong".to_owned(),
        recipe: TimbreRecipe::KarplusStrong { damping },
        default_envelope: default_env(),
        metadata: TimbreMeta {
            brightness: 2.1,
            roughness: 0.35,
            attack_kind: AttackKind::Plucked,
            category: "string".to_owned(),
        },
        filters: Vec::new(),
    }
}

/// Returns a two-operator FM timbre with the given modulator ratio and index.
pub fn fm_pair(modulator_ratio: f64, index: f64) -> Timbre {
    Timbre {
        name: "fm_pair".to_owned(),
        recipe: TimbreRecipe::FmPair {
            modulator_ratio,
            index,
        },
        default_envelope: default_env(),
        metadata: TimbreMeta {
            brightness: 3.8,
            roughness: 0.45,
            attack_kind: AttackKind::Struck,
            category: "synth".to_owned(),
        },
        filters: Vec::new(),
    }
}

/// Returns an inharmonic bell timbre from the given partial ratios.
pub fn bell_inharmonic(ratios: &[f64]) -> Timbre {
    Timbre {
        name: "bell_inharmonic".to_owned(),
        recipe: TimbreRecipe::BellInharmonic {
            ratios: ratios.to_vec(),
        },
        default_envelope: default_env(),
        metadata: TimbreMeta {
            brightness: 4.6,
            roughness: 0.55,
            attack_kind: AttackKind::Struck,
            category: "bell".to_owned(),
        },
        filters: Vec::new(),
    }
}

/// Returns a timbre from explicit root-normalized tagged partials.
pub fn tagged_partials(partials: &[SampledPartial]) -> Timbre {
    harmonic_timbre(
        "tagged_partials",
        TimbreRecipe::TaggedPartials {
            partials: partials.to_vec(),
        },
        2.6,
    )
}

/// Returns a harmonic expansion with tagged partials.
pub fn harmonic_expansion(partials: usize, amplitude_decay: f64, phase_step: f64) -> Timbre {
    harmonic_timbre(
        "harmonic_expansion",
        TimbreRecipe::HarmonicExpansion {
            partials,
            amplitude_decay,
            phase_step,
        },
        3.2,
    )
}

/// Returns an undertone expansion with tagged partials.
pub fn undertone_expansion(partials: usize, amplitude_decay: f64, phase_step: f64) -> Timbre {
    harmonic_timbre(
        "undertone_expansion",
        TimbreRecipe::UndertoneExpansion {
            partials,
            amplitude_decay,
            phase_step,
        },
        1.8,
    )
}

/// Returns a root-normalized sampled timbre.
pub fn sampled_timbre(
    root: Frequency,
    partials: &[SampledPartial],
    interpolation: SampleInterpolation,
    pitch_policy: SamplePitchPolicy,
) -> Timbre {
    Timbre {
        name: "sampled_timbre".to_owned(),
        recipe: TimbreRecipe::Sampled {
            root,
            partials: partials.to_vec(),
            interpolation,
            pitch_policy,
        },
        default_envelope: default_env(),
        metadata: TimbreMeta {
            brightness: 2.5,
            roughness: 0.1,
            attack_kind: AttackKind::Soft,
            category: "sampled".to_owned(),
        },
        filters: Vec::new(),
    }
}

fn harmonic_timbre(name: &str, recipe: TimbreRecipe, brightness: f64) -> Timbre {
    Timbre {
        name: name.to_owned(),
        recipe,
        default_envelope: default_env(),
        metadata: TimbreMeta {
            brightness,
            roughness: 0.2,
            attack_kind: AttackKind::Soft,
            category: "harmonic".to_owned(),
        },
        filters: Vec::new(),
    }
}
