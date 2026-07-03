//! Component parameter descriptors, ranges, and units.
//!
//! Defines the measurement unit of a parameter ([`ComponentParamUnit`]), a
//! continuous min/max/default [`ComponentParamRange`], and the
//! [`ComponentParamDescriptor`] that ties together a parameter's id, label,
//! unit, range, enum values, and defaults for editor display and normalization.

use sim_kernel::Symbol;

/// Measurement unit a component parameter is expressed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentParamUnit {
    /// Dimensionless quantity.
    Unitless,
    /// Frequency in Hz.
    Hertz,
    /// Duration in seconds.
    Seconds,
    /// Pitch offset in semitones.
    Semitones,
    /// Normalized value in `[0, 1]`.
    Normalized,
    /// Raw integer value (not normalized).
    RawInteger,
}

impl ComponentParamUnit {
    /// Returns the stable lowercase name of the unit.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unitless => "unitless",
            Self::Hertz => "hz",
            Self::Seconds => "seconds",
            Self::Semitones => "semitones",
            Self::Normalized => "normalized",
            Self::RawInteger => "raw-integer",
        }
    }

    /// Returns the qualified symbol naming this unit.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/param-unit", self.as_str())
    }
}

/// A continuous parameter range with a minimum, maximum, and default value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComponentParamRange {
    min: f64,
    max: f64,
    default: f64,
}

impl ComponentParamRange {
    /// Builds a range, swapping `min` and `max` if reversed and clamping the
    /// default into `[min, max]`.
    pub fn new(min: f64, max: f64, default: f64) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        Self {
            min,
            max,
            default: default.clamp(min, max),
        }
    }

    /// Returns the range minimum.
    pub fn min(&self) -> f64 {
        self.min
    }

    /// Returns the range maximum.
    pub fn max(&self) -> f64 {
        self.max
    }

    /// Returns the default value within the range.
    pub fn default(&self) -> f64 {
        self.default
    }

    /// Maps `value` to `[0, 1]` across the range, clamping outside the bounds;
    /// returns 0 for a degenerate range.
    pub fn normalize(&self, value: f64) -> f64 {
        if self.max <= self.min {
            return 0.0;
        }
        ((value.clamp(self.min, self.max) - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }
}

/// Describes one editable component parameter: its id, label, unit, optional
/// range or enum values, and defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentParamDescriptor {
    id: Symbol,
    label: String,
    unit: ComponentParamUnit,
    range: Option<ComponentParamRange>,
    enum_values: Vec<Symbol>,
    normalized_default: f64,
    raw_default: Option<i64>,
}

impl ComponentParamDescriptor {
    /// Builds a descriptor with the given id, display label, and unit; range,
    /// enum values, and defaults start empty.
    pub fn new(id: Symbol, label: impl Into<String>, unit: ComponentParamUnit) -> Self {
        Self {
            id,
            label: label.into(),
            unit,
            range: None,
            enum_values: Vec::new(),
            normalized_default: 0.0,
            raw_default: None,
        }
    }

    /// Attaches a continuous range and sets the normalized default from its
    /// default value.
    pub fn with_range(mut self, range: ComponentParamRange) -> Self {
        self.normalized_default = range.normalize(range.default());
        self.range = Some(range);
        self
    }

    /// Attaches enumerated values and sets the normalized default from
    /// `default_index` (clamped to the value count).
    pub fn with_enum_values(mut self, values: Vec<Symbol>, default_index: usize) -> Self {
        self.normalized_default = if values.len() <= 1 {
            0.0
        } else {
            default_index.min(values.len() - 1) as f64 / (values.len() - 1) as f64
        };
        self.enum_values = values;
        self
    }

    /// Sets a raw integer default value for the parameter.
    pub fn with_raw_default(mut self, raw: i64) -> Self {
        self.raw_default = Some(raw);
        self
    }

    /// Returns the parameter's identity symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the parameter's display label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the parameter's unit.
    pub fn unit(&self) -> ComponentParamUnit {
        self.unit
    }

    /// Returns the parameter's continuous range, if any.
    pub fn range(&self) -> Option<ComponentParamRange> {
        self.range
    }

    /// Returns the parameter's enumerated values, if any.
    pub fn enum_values(&self) -> &[Symbol] {
        &self.enum_values
    }

    /// Returns the normalized default value in `[0, 1]`.
    pub fn normalized_default(&self) -> f64 {
        self.normalized_default
    }

    /// Returns the raw integer default value, if set.
    pub fn raw_default(&self) -> Option<i64> {
        self.raw_default
    }

    /// Normalizes a raw integer value against the parameter's range, returning
    /// `None` if the parameter has no range.
    pub fn normalize_raw(&self, raw: i64) -> Option<f64> {
        self.range.map(|range| range.normalize(raw as f64))
    }
}
