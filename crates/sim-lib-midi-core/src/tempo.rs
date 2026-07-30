//! MIDI tempo maps and exact conversions among ticks, quarter beats, and wall
//! time.

use sim_kernel::Symbol;
use sim_lib_stream_clock::{Clock, ClockIndex, Instant, TempoMap as ClockTempoMap, TempoSegment};

use crate::{MetaEvent, MidiError, MidiEvent, MidiPayload, TickTime};

/// The default MIDI tempo of 500_000 microseconds per quarter (120 BPM).
pub const DEFAULT_US_PER_QUARTER: u32 = 500_000;

/// Converts beats per minute to microseconds per quarter note (rounded).
pub fn bpm_to_us_per_quarter(bpm: f64) -> u32 {
    (60_000_000.0 / bpm).round() as u32
}

/// Converts microseconds per quarter note to beats per minute.
pub fn us_per_quarter_to_bpm(us_per_quarter: u32) -> f64 {
    60_000_000.0 / us_per_quarter as f64
}

/// An exact, non-negative position measured in MIDI quarter-note beats.
///
/// The reduced rational representation avoids rounding tuplets or rebased
/// ticks. MIDI tempo always uses a quarter note as its beat unit, independent
/// of the notated time signature.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MidiBeat {
    numerator: i128,
    denominator: i128,
}

impl MidiBeat {
    /// Builds `numerator / denominator` quarter-note beats and reduces it.
    ///
    /// Negative values and a zero denominator are rejected because SMF tempo
    /// timelines begin at tick zero.
    pub fn new(numerator: i128, denominator: i128) -> Result<Self, MidiError> {
        if denominator == 0 {
            return Err(MidiError::InvalidRatio(
                narrow_i128(numerator)?,
                narrow_i128(denominator)?,
            ));
        }
        let (numerator, denominator) = if denominator < 0 {
            (
                numerator.checked_neg().ok_or(MidiError::TempoOverflow)?,
                denominator.checked_neg().ok_or(MidiError::TempoOverflow)?,
            )
        } else {
            (numerator, denominator)
        };
        if numerator < 0 {
            return Err(MidiError::NegativeTempoTick);
        }
        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    /// Returns the reduced numerator in quarter-note beats.
    pub const fn numerator(self) -> i128 {
        self.numerator
    }

    /// Returns the reduced, positive denominator in quarter-note beats.
    pub const fn denominator(self) -> i128 {
        self.denominator
    }
}

/// A piecewise-constant MIDI tempo map at one ticks-per-quarter resolution.
///
/// Construction consumes already ordered MIDI events, applies the standard
/// 120-BPM default before the first tempo meta event, and lets the last tempo
/// meta event at a tick win. Wall-time conversion delegates to the generic
/// exact chart in `sim-lib-stream-clock`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiTempoMap {
    tpq: u32,
    clock_map: ClockTempoMap,
}

impl MidiTempoMap {
    /// Builds a tempo map from an ordered event stream.
    ///
    /// Every event must be non-negative, exactly expressible at `tpq`, and
    /// monotonic. Non-tempo events participate in the ordering check but do not
    /// create segments.
    pub fn from_ordered_events<'a>(
        tpq: u32,
        events: impl IntoIterator<Item = &'a MidiEvent>,
    ) -> Result<Self, MidiError> {
        if tpq == 0 {
            return Err(MidiError::ZeroTpq);
        }
        let mut segments = vec![TempoSegment::new(0, DEFAULT_US_PER_QUARTER).map_err(chart_error)?];
        let mut previous_tick = 0_u64;
        for event in events {
            let time = event.time.rebase(tpq)?;
            let tick = u64::try_from(time.ticks).map_err(|_| MidiError::NegativeTempoTick)?;
            if tick < previous_tick {
                return Err(MidiError::TempoEventsOutOfOrder);
            }
            previous_tick = tick;
            let MidiPayload::Meta(MetaEvent::Tempo { us_per_quarter }) = &event.payload else {
                continue;
            };
            if *us_per_quarter == 0 {
                return Err(MidiError::ZeroTempo);
            }
            if segments
                .last()
                .is_some_and(|segment| segment.start_tick == tick)
            {
                let last = segments
                    .last_mut()
                    .expect("the default tempo segment is always present");
                *last = TempoSegment::new(tick, *us_per_quarter).map_err(chart_error)?;
            } else {
                segments.push(TempoSegment::new(tick, *us_per_quarter).map_err(chart_error)?);
            }
        }
        let clock_map = ClockTempoMap::new(segments).map_err(chart_error)?;
        Ok(Self { tpq, clock_map })
    }

    /// Returns this map's ticks-per-quarter resolution.
    pub const fn tpq(&self) -> u32 {
        self.tpq
    }

    /// Returns the ordered constant-tempo segments.
    pub fn segments(&self) -> &[TempoSegment] {
        self.clock_map.segments()
    }

    /// Converts a tick position to an exact quarter-note beat.
    pub fn beat_for_tick(&self, tick: TickTime) -> Result<MidiBeat, MidiError> {
        let tick = tick.rebase(self.tpq)?;
        MidiBeat::new(i128::from(tick.ticks), i128::from(self.tpq))
    }

    /// Converts an exact quarter-note beat to an integer tick.
    ///
    /// Returns [`MidiError::InexactTempoTick`] when the beat lies between tick
    /// boundaries at this map's resolution.
    pub fn tick_for_beat(&self, beat: MidiBeat) -> Result<TickTime, MidiError> {
        let scaled = beat
            .numerator
            .checked_mul(i128::from(self.tpq))
            .ok_or(MidiError::TempoOverflow)?;
        if scaled % beat.denominator != 0 {
            return Err(MidiError::InexactTempoTick);
        }
        let ticks =
            i64::try_from(scaled / beat.denominator).map_err(|_| MidiError::TempoOverflow)?;
        TickTime::new(ticks, self.tpq)
    }

    /// Converts a tick position to exact non-negative wall time in seconds.
    pub fn wall_time_for_tick(&self, tick: TickTime) -> Result<Instant, MidiError> {
        let tick = tick.rebase(self.tpq)?;
        let index = u64::try_from(tick.ticks).map_err(|_| MidiError::NegativeTempoTick)?;
        self.clock()?
            .instant_for_index(ClockIndex::new(index))
            .map_err(chart_error)
    }

    /// Converts exact wall time to an integer tick.
    ///
    /// Returns [`MidiError::InexactTempoTick`] instead of rounding when the
    /// instant lies between MIDI tick boundaries.
    pub fn tick_for_wall_time(&self, wall_time: Instant) -> Result<TickTime, MidiError> {
        let conversion = self
            .clock()?
            .index_for_instant(wall_time)
            .map_err(chart_error)?;
        if !conversion.is_exact() {
            return Err(MidiError::InexactTempoTick);
        }
        let ticks =
            i64::try_from(conversion.index().value()).map_err(|_| MidiError::TempoOverflow)?;
        TickTime::new(ticks, self.tpq)
    }

    fn clock(&self) -> Result<Clock, MidiError> {
        Clock::midi(
            Symbol::qualified("midi/tempo", "timeline"),
            self.tpq,
            self.clock_map.clone(),
        )
        .map_err(chart_error)
    }
}

fn chart_error(error: sim_kernel::Error) -> MidiError {
    MidiError::TempoChart(error.to_string())
}

fn narrow_i128(value: i128) -> Result<i64, MidiError> {
    i64::try_from(value).map_err(|_| MidiError::TempoOverflow)
}

fn gcd(mut left: i128, mut right: i128) -> i128 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}
