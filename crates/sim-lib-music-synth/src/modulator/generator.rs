use sim_lib_music_core::{PlayContext, PlayEvent, Tick};

use super::{
    ModulationOperator, ModulationRate, ModulatorConfig, ModulatorSample, ModulatorSource,
};
use crate::{AdsrEnvelope, Lfo, Oscillator, PhaseOscillator};

pub(super) fn output_ticks(config: &ModulatorConfig, cx: &PlayContext) -> Vec<Tick> {
    if config.rate == ModulationRate::PerNote {
        let ticks = cx
            .upstream
            .iter()
            .filter_map(|event| match event {
                PlayEvent::Note(note) if cx.range.contains(note.time) => Some(note.time),
                _ => None,
            })
            .collect::<Vec<_>>();
        return if ticks.is_empty() {
            vec![cx.range.start]
        } else {
            ticks
        };
    }

    let step = ticks_per_output(config.rate, cx);
    let mut ticks = Vec::new();
    let mut tick = cx.range.start.ticks;
    while tick < cx.range.end.ticks {
        ticks.push(Tick {
            ticks: tick,
            tpq: cx.range.start.tpq,
        });
        tick += step;
    }
    ticks
}

fn ticks_per_output(rate: ModulationRate, cx: &PlayContext) -> i64 {
    match rate {
        ModulationRate::Audio | ModulationRate::Tick => 1,
        ModulationRate::Control => (i64::from(cx.ppq) / 24).max(1),
        ModulationRate::Step => (i64::from(cx.ppq) / 4).max(1),
        ModulationRate::PerNote => 1,
    }
}

pub(super) struct ModulatorGenerator {
    source: ModulatorSource,
    lfo: Option<Lfo>,
    envelope: Option<AdsrEnvelope>,
    oscillator: Option<PhaseOscillator>,
    random: PatternRng,
    random_value: f32,
    envelope_release_tick: i64,
    envelope_released: bool,
}

impl ModulatorGenerator {
    pub(super) fn new(config: &ModulatorConfig, cx: &PlayContext) -> Self {
        let mut lfo = None;
        let mut envelope = None;
        let mut oscillator = None;
        match &config.source {
            ModulatorSource::Lfo(settings) => {
                let mut source = Lfo::new(*settings);
                source.set_sample_rate(cx.sample_rate as f32);
                source.set_tempo_bpm(tempo_bpm(cx));
                lfo = Some(source);
            }
            ModulatorSource::Envelope(settings) => {
                let mut source = AdsrEnvelope::new(*settings);
                source.set_sample_rate(cx.sample_rate as f32);
                source.note_on();
                envelope = Some(source);
            }
            ModulatorSource::Oscillator {
                kind, frequency_hz, ..
            } => {
                let mut source = PhaseOscillator::new(*kind, frequency_hz.max(0.0));
                source.set_sample_rate(cx.sample_rate as f32);
                oscillator = Some(source);
            }
            ModulatorSource::RandomWalk(_) | ModulatorSource::AutomationCurve(_) => {}
        }
        Self {
            source: config.source.clone(),
            lfo,
            envelope,
            oscillator,
            random: PatternRng::new(config.seed ^ cx.seed),
            random_value: random_start(&config.source),
            envelope_release_tick: cx.range.start.ticks
                + (cx.range.end.ticks - cx.range.start.ticks) / 2,
            envelope_released: false,
        }
    }

    pub(super) fn next(&mut self, tick: i64) -> f32 {
        match &self.source {
            ModulatorSource::Lfo(_) => self.lfo.as_mut().map(Lfo::next_sample).unwrap_or(0.0),
            ModulatorSource::Envelope(_) => {
                let envelope = self.envelope.as_mut().expect("envelope source");
                if tick >= self.envelope_release_tick && !self.envelope_released {
                    envelope.note_off();
                    self.envelope_released = true;
                }
                envelope.next_sample()
            }
            ModulatorSource::Oscillator { amplitude, .. } => self
                .oscillator
                .as_mut()
                .map(|oscillator| oscillator.next_sample() * amplitude.max(0.0))
                .unwrap_or(0.0),
            ModulatorSource::RandomWalk(settings) => {
                let (min, max) = ordered(settings.min, settings.max);
                let direction = if self.random.next_bool() { 1.0 } else { -1.0 };
                self.random_value =
                    (self.random_value + direction * settings.step.abs()).clamp(min, max);
                self.random_value
            }
            ModulatorSource::AutomationCurve(curve) => curve.value_at(tick),
        }
    }
}

fn random_start(source: &ModulatorSource) -> f32 {
    match source {
        ModulatorSource::RandomWalk(settings) => {
            let (min, max) = ordered(settings.min, settings.max);
            settings.start.clamp(min, max)
        }
        _ => 0.0,
    }
}

fn tempo_bpm(cx: &PlayContext) -> f64 {
    60_000_000.0 / f64::from(cx.tempo.us_per_quarter.max(1))
}

pub(super) fn apply_operator(samples: &mut [ModulatorSample], operator: &ModulationOperator) {
    match *operator {
        ModulationOperator::Sum(offset) => {
            for sample in samples {
                sample.value += offset;
            }
        }
        ModulationOperator::Multiply(factor) => {
            for sample in samples {
                sample.value *= factor;
            }
        }
        ModulationOperator::SampleHold { samples: span } => {
            let span = span.max(1);
            let mut held = None;
            for (index, sample) in samples.iter_mut().enumerate() {
                if index % span == 0 {
                    held = Some(sample.value);
                } else if let Some(value) = held {
                    sample.value = value;
                }
            }
        }
        ModulationOperator::Quantize { step } => {
            let step = step.abs();
            if step > 0.0 {
                for sample in samples {
                    sample.value = (sample.value / step).round() * step;
                }
            }
        }
        ModulationOperator::Smooth { amount } => {
            let amount = amount.clamp(0.0, 1.0);
            let mut state = None;
            for sample in samples {
                let current = state.unwrap_or(sample.value);
                let next = current + (sample.value - current) * amount;
                sample.value = next;
                state = Some(next);
            }
        }
        ModulationOperator::Clip { min, max } => {
            let (min, max) = ordered(min, max);
            for sample in samples {
                sample.value = sample.value.clamp(min, max);
            }
        }
        ModulationOperator::Lag { step } => {
            let step = step.abs();
            let mut state = None;
            for sample in samples {
                let current = state.unwrap_or(sample.value);
                let delta = (sample.value - current).clamp(-step, step);
                let next = current + delta;
                sample.value = next;
                state = Some(next);
            }
        }
    }
}

pub(super) fn scaled_control(value: f32) -> i64 {
    (value * 1_000_000.0).round() as i64
}

fn ordered(left: f32, right: f32) -> (f32, f32) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

pub(super) fn modulator_hash(config: &ModulatorConfig, events: &[PlayEvent], seed: u64) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in format!("{config:?}:{events:?}:{seed}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[derive(Clone, Debug)]
struct PatternRng {
    state: u64,
}

impl PatternRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }
}
