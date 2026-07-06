use sim_lib_music_core::{
    ArrangerPlacement, FilterRef, LaneTarget, PitchRemap, PlacementTransform, PlayableRef,
    StretchPolicy, TracePolicy,
};

use super::{encode_music, encode_pitch, encode_string, encode_time};

pub(super) fn encode_arranger_placement(placement: &ArrangerPlacement) -> String {
    let duration = placement
        .duration
        .map(encode_time)
        .unwrap_or_else(|| "none".to_owned());
    let seed = placement
        .seed
        .map(|seed| seed.to_string())
        .unwrap_or_else(|| "none".to_owned());
    let filter = placement
        .filter
        .as_ref()
        .map(encode_filter_ref)
        .unwrap_or_else(|| "none".to_owned());
    format!(
        "#(ArrangerPlacement id={} at={} duration={} lane={} playable={} targets=[{}] stretch={} transforms=[{}] remap={} filter={} seed={} trace={})",
        encode_string(&placement.id.as_qualified_str()),
        encode_time(placement.at),
        duration,
        encode_string(&placement.lane.0),
        encode_playable_ref(&placement.playable),
        placement
            .targets
            .iter()
            .map(encode_lane_target)
            .collect::<Vec<_>>()
            .join(","),
        encode_stretch_policy(&placement.stretch),
        placement
            .transform
            .iter()
            .map(encode_placement_transform)
            .collect::<Vec<_>>()
            .join(","),
        encode_pitch_remap(&placement.remap_pitch),
        filter,
        seed,
        encode_trace_policy(placement.trace),
    )
}

fn encode_playable_ref(playable: &PlayableRef) -> String {
    match playable {
        PlayableRef::Inline(music) => encode_music(music),
        PlayableRef::Symbol(symbol) => {
            format!(
                "#(PlayableRef kind=symbol symbol={})",
                encode_string(&symbol.as_qualified_str())
            )
        }
    }
}

fn encode_lane_target(target: &LaneTarget) -> String {
    match target {
        LaneTarget::Instrument(symbol) => format!(
            "#(LaneTarget kind=instrument symbol={})",
            encode_string(&symbol.as_qualified_str())
        ),
        LaneTarget::Stream(symbol) => format!(
            "#(LaneTarget kind=stream symbol={})",
            encode_string(&symbol.as_qualified_str())
        ),
        LaneTarget::Control(symbol) => format!(
            "#(LaneTarget kind=control symbol={})",
            encode_string(&symbol.as_qualified_str())
        ),
        LaneTarget::None => "#(LaneTarget kind=none)".to_owned(),
    }
}

fn encode_stretch_policy(policy: &StretchPolicy) -> String {
    match policy {
        StretchPolicy::None => "#(StretchPolicy kind=none)".to_owned(),
        StretchPolicy::TempoRatio(ratio) => {
            format!(
                "#(StretchPolicy kind=tempo-ratio value={})",
                encode_time(*ratio)
            )
        }
        StretchPolicy::TimeRatio(ratio) => {
            format!(
                "#(StretchPolicy kind=time-ratio value={})",
                encode_time(*ratio)
            )
        }
        StretchPolicy::FitToDuration => "#(StretchPolicy kind=fit)".to_owned(),
    }
}

fn encode_placement_transform(transform: &PlacementTransform) -> String {
    match transform {
        PlacementTransform::TransposeSemitones(semitones) => {
            format!("#(PlacementTransform kind=transpose-semitones value={semitones})")
        }
        PlacementTransform::TransposeOctaves(octaves) => {
            format!("#(PlacementTransform kind=transpose-octaves value={octaves})")
        }
        PlacementTransform::InvertAroundPitch(pitch) => {
            format!(
                "#(PlacementTransform kind=invert-pitch value={})",
                encode_pitch(*pitch)
            )
        }
        PlacementTransform::InvertAroundPitchClass(axis) => {
            format!(
                "#(PlacementTransform kind=invert-pitch-class value={})",
                axis.0
            )
        }
        PlacementTransform::Retrograde => "#(PlacementTransform kind=retrograde)".to_owned(),
    }
}

fn encode_pitch_remap(remap: &PitchRemap) -> String {
    match remap {
        PitchRemap::None => "#(PitchRemap kind=none)".to_owned(),
        PitchRemap::Chromatic(semitones) => {
            format!("#(PitchRemap kind=chromatic value={semitones})")
        }
        PitchRemap::PitchClass { from, to } => {
            format!("#(PitchRemap kind=pitch-class from={} to={})", from.0, to.0)
        }
        PitchRemap::DrumKey(items) => format!(
            "#(PitchRemap kind=drum-key items=[{}])",
            items
                .iter()
                .map(|(from, to)| format!("#(DrumKey from={from} to={to})"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        PitchRemap::ScaleDegree(symbol) => encode_symbolic_remap("scale-degree", symbol),
        PitchRemap::ChordTone(symbol) => encode_symbolic_remap("chord-tone", symbol),
        PitchRemap::Tuning(symbol) => encode_symbolic_remap("tuning", symbol),
        PitchRemap::Vector(symbol) => encode_symbolic_remap("vector", symbol),
        PitchRemap::Matrix(symbol) => encode_symbolic_remap("matrix", symbol),
        PitchRemap::Callable(symbol) => encode_symbolic_remap("callable", symbol),
    }
}

fn encode_symbolic_remap(kind: &'static str, symbol: &sim_kernel::Symbol) -> String {
    format!(
        "#(PitchRemap kind={kind} symbol={})",
        encode_string(&symbol.as_qualified_str())
    )
}

fn encode_filter_ref(filter: &FilterRef) -> String {
    format!(
        "#(FilterRef id={} keep_lanes=[{}])",
        encode_string(&filter.id.as_qualified_str()),
        filter
            .keep_lanes
            .iter()
            .map(|lane| encode_string(&lane.0))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn encode_trace_policy(policy: TracePolicy) -> &'static str {
    match policy {
        TracePolicy::Off => "off",
        TracePolicy::Diagnostics => "diagnostics",
        TracePolicy::Full => "full",
    }
}
