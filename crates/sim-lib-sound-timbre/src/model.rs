use std::time::Duration;

use sim_lib_sound_core::{
    Amplitude, Envelope, EnvelopeShape, Frequency, Partial, PartialTag, Phase, Tone,
};

use crate::Filter;

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
    /// A mix of two recipes.
    Layered {
        /// Primary recipe, weighted by `1.0 - mix`.
        primary: Box<TimbreRecipe>,
        /// Secondary recipe, weighted by `mix`.
        secondary: Box<TimbreRecipe>,
        /// Blend ratio in `0.0..=1.0`.
        mix: f64,
    },
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
        let mut tone = render_recipe(&self.recipe, frequency, duration);
        tone.envelope = self.default_envelope.clone();
        for filter in &self.filters {
            tone = filter.apply(tone);
        }
        tone
    }

    /// Returns a hybrid timbre layering `self` and `other` at blend ratio
    /// `mix`, concatenating their filter chains.
    pub fn layer(self, other: Timbre, mix: f64) -> Timbre {
        let mut filters = self.filters.clone();
        filters.extend(other.filters.clone());
        Timbre {
            name: format!("{}+{}", self.name, other.name),
            recipe: TimbreRecipe::Layered {
                primary: Box::new(self.recipe),
                secondary: Box::new(other.recipe),
                mix,
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

fn render_recipe(recipe: &TimbreRecipe, frequency: Frequency, duration: Duration) -> Tone {
    match recipe {
        TimbreRecipe::PureSine => Tone::sine(frequency, duration),
        TimbreRecipe::Sawtooth { partials } => Tone::sawtooth(frequency, duration, *partials),
        TimbreRecipe::Square { partials } => Tone::square(frequency, duration, *partials),
        TimbreRecipe::Triangle { partials } => Tone::triangle(frequency, duration, *partials),
        TimbreRecipe::OrganPipe { stops } => {
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
            Tone::from_partials(partials, default_env(), duration).expect("organ tone")
        }
        TimbreRecipe::KarplusStrong { damping } => {
            let partials = (1..=8)
                .map(|n| Partial {
                    frequency: Frequency(frequency.0 * n as f64),
                    amplitude: Amplitude(damping.powi(n).clamp(0.0, 1.0)),
                    phase: Phase(0.0),
                    tag: PartialTag::Harmonic(n as u32),
                })
                .collect();
            Tone::from_partials(partials, default_env(), duration).expect("karplus strong tone")
        }
        TimbreRecipe::FmPair {
            modulator_ratio,
            index,
        } => {
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
            Tone::from_partials(partials, default_env(), duration).expect("fm tone")
        }
        TimbreRecipe::BellInharmonic { ratios } => {
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
            Tone::from_partials(partials, default_env(), duration).expect("bell tone")
        }
        TimbreRecipe::Layered {
            primary,
            secondary,
            mix,
        } => {
            render_recipe(primary, frequency, duration).amplify(1.0 - mix.clamp(0.0, 1.0))
                + render_recipe(secondary, frequency, duration).amplify(mix.clamp(0.0, 1.0))
        }
    }
}

fn default_env() -> Envelope {
    Envelope::new(
        Duration::from_millis(15),
        Duration::from_millis(60),
        0.75,
        Duration::from_millis(120),
        EnvelopeShape::Linear,
    )
    .expect("default timbre envelope")
}
