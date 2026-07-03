use sim_kernel::{Ref, Symbol, Tick as KernelTick};
use sim_lib_midi_core::{DEFAULT_US_PER_QUARTER, TickTime};

use crate::{MusicError, Time};

/// A musical position or duration measured in beats, as a rational [`Time`].
pub type Beat = Time;
/// A musical position or duration in beats, as a rational [`Time`].
pub type MusicalTime = Time;
/// A position or duration in MIDI-style ticks, re-exporting `TickTime`.
pub type Tick = TickTime;

/// A reference to a tempo map, carrying its id and constant tempo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempoMapRef {
    /// The qualified identifier of the tempo map.
    pub id: Symbol,
    /// Microseconds per quarter note for the (constant) tempo.
    pub us_per_quarter: u32,
}

impl TempoMapRef {
    /// Builds a constant-tempo reference from an id and microseconds per quarter.
    pub fn constant(id: impl Into<String>, us_per_quarter: u32) -> Self {
        Self {
            id: Symbol::qualified("music/tempo", id.into()),
            us_per_quarter,
        }
    }
}

impl Default for TempoMapRef {
    fn default() -> Self {
        Self::constant("default", DEFAULT_US_PER_QUARTER)
    }
}

/// A half-open span of ticks `[start, end)`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TimeRange {
    /// The inclusive start tick of the range.
    pub start: Tick,
    /// The exclusive end tick of the range.
    pub end: Tick,
}

impl TimeRange {
    /// Builds a range, quantizing `end` to `start`'s resolution.
    ///
    /// Returns [`MusicError::InvalidTimeRange`] when `end` precedes `start`.
    pub fn new(start: Tick, end: Tick) -> Result<Self, MusicError> {
        let end = end.quantize(start.tpq);
        if end.ticks < start.ticks {
            return Err(MusicError::InvalidTimeRange);
        }
        Ok(Self { start, end })
    }

    /// Builds a range from raw tick counts at the given pulses-per-quarter.
    ///
    /// Returns [`MusicError::InvalidPpq`] when `ppq` is zero.
    pub fn from_ticks(start: i64, end: i64, ppq: u32) -> Result<Self, MusicError> {
        if ppq == 0 {
            return Err(MusicError::InvalidPpq);
        }
        Self::new(
            Tick {
                ticks: start,
                tpq: ppq,
            },
            Tick {
                ticks: end,
                tpq: ppq,
            },
        )
    }

    /// Builds a range from tick zero to `end`, sharing `end`'s resolution.
    pub fn starts_at_zero(end: Tick) -> Result<Self, MusicError> {
        Self::new(
            Tick {
                ticks: 0,
                tpq: end.tpq,
            },
            end,
        )
    }

    /// Reports whether `tick` falls within the half-open range.
    pub fn contains(self, tick: Tick) -> bool {
        let tick = tick.quantize(self.start.tpq);
        tick.ticks >= self.start.ticks && tick.ticks < self.end.ticks
    }

    /// Clips a `(start, duration)` span to this range.
    ///
    /// Returns the clipped start and duration, or `None` when the span lies
    /// entirely outside the range.
    pub fn clip_span(self, start: Tick, duration: Tick) -> Option<(Tick, Tick)> {
        let start = start.quantize(self.start.tpq);
        let end = (start + duration).quantize(self.start.tpq);
        if end.ticks <= self.start.ticks || start.ticks >= self.end.ticks {
            return None;
        }
        let clipped_start = Tick::new(start.ticks.max(self.start.ticks), self.start.tpq).ok()?;
        let clipped_end = Tick::new(end.ticks.min(self.end.ticks), self.start.tpq).ok()?;
        let clipped_duration =
            Tick::new(clipped_end.ticks - clipped_start.ticks, self.start.tpq).ok()?;
        Some((clipped_start, clipped_duration))
    }
}

/// Converts a musical time in beats to ticks at the given pulses-per-quarter.
///
/// Returns [`MusicError::InvalidPpq`] when `ppq` is zero.
pub fn time_to_tick(time: MusicalTime, ppq: u32) -> Result<Tick, MusicError> {
    if ppq == 0 {
        return Err(MusicError::InvalidPpq);
    }
    let ticks = *time.numer() * 4 * i64::from(ppq) / *time.denom();
    Ok(Tick { ticks, tpq: ppq })
}

/// Encodes a tick on a named clock as a kernel `Tick` reference.
pub fn tick_to_kernel_tick(tick: Tick, clock: Symbol) -> KernelTick {
    KernelTick::new(
        clock,
        Ref::Symbol(Symbol::qualified(
            "music/tick",
            format!("{}@{}", tick.ticks, tick.tpq),
        )),
    )
}
