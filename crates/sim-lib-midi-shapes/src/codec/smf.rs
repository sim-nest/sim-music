use sim_lib_midi_smf::{SmfDivision, SmfFile, SmfFormat, SmfTrack, SmpteRate};

use super::{MidiShapeError, decode_midi_event, encode_midi_event, split_top_level};

/// Encodes an [`SmfTrack`] as a `#(SmfTrack ...)` form.
pub fn encode_smf_track(track: &SmfTrack) -> String {
    if track.events.is_empty() {
        return "#(SmfTrack)".to_owned();
    }
    format!(
        "#(SmfTrack {})",
        track
            .events
            .iter()
            .map(encode_midi_event)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// Decodes an [`SmfTrack`] from a `#(SmfTrack ...)` form.
pub fn decode_smf_track(value: &str) -> Result<SmfTrack, MidiShapeError> {
    let inner = value
        .strip_prefix("#(SmfTrack")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidSmfTrack)?
        .trim();
    if inner.is_empty() {
        return Ok(SmfTrack { events: Vec::new() });
    }
    let events = split_top_level(inner)
        .into_iter()
        .map(decode_midi_event)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SmfTrack { events })
}

/// Encodes an [`SmfFile`] as a
/// `#(SmfFile <format> #(SmfDivision ...) ...)` form.
pub fn encode_smf_file(file: &SmfFile) -> String {
    let format = match file.format {
        SmfFormat::SingleTrack => "SingleTrack",
        SmfFormat::Simultaneous => "Simultaneous",
        SmfFormat::Independent => "Independent",
    };
    let division = encode_smf_division(file.division);
    if file.tracks.is_empty() {
        return format!("#(SmfFile {format} {division})");
    }
    format!(
        "#(SmfFile {} {} {})",
        format,
        division,
        file.tracks
            .iter()
            .map(encode_smf_track)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// Decodes an [`SmfFile`] from a `#(SmfFile ...)` form.
pub fn decode_smf_file(value: &str) -> Result<SmfFile, MidiShapeError> {
    let inner = value
        .strip_prefix("#(SmfFile ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidSmfFile)?;
    let parts = split_top_level(inner);
    if parts.len() < 2 {
        return Err(MidiShapeError::InvalidSmfFile);
    }
    let format = match parts[0] {
        "SingleTrack" => SmfFormat::SingleTrack,
        "Simultaneous" => SmfFormat::Simultaneous,
        "Independent" => SmfFormat::Independent,
        _ => return Err(MidiShapeError::InvalidSmfFile),
    };
    let division = decode_smf_division(parts[1])?;
    let tracks = parts[2..]
        .iter()
        .map(|part| decode_smf_track(part))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SmfFile {
        format,
        division,
        tracks,
    })
}

fn encode_smf_division(division: SmfDivision) -> String {
    match division {
        SmfDivision::Metrical { ticks_per_quarter } => {
            format!("#(SmfDivision Metrical {})", ticks_per_quarter.get())
        }
        SmfDivision::Smpte {
            frames_per_second,
            ticks_per_frame,
        } => {
            let rate = match frames_per_second {
                SmpteRate::Fps24 => "Fps24",
                SmpteRate::Fps25 => "Fps25",
                SmpteRate::Fps29Drop => "Fps29Drop",
                SmpteRate::Fps30 => "Fps30",
            };
            format!("#(SmfDivision Smpte {rate} {})", ticks_per_frame.get())
        }
    }
}

fn decode_smf_division(value: &str) -> Result<SmfDivision, MidiShapeError> {
    let inner = value
        .strip_prefix("#(SmfDivision ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidSmfFile)?;
    let parts = inner.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["Metrical", ticks] => ticks
            .parse::<u16>()
            .ok()
            .and_then(SmfDivision::metrical)
            .ok_or(MidiShapeError::InvalidSmfFile),
        ["Smpte", rate, ticks] => {
            let rate = match *rate {
                "Fps24" => SmpteRate::Fps24,
                "Fps25" => SmpteRate::Fps25,
                "Fps29Drop" => SmpteRate::Fps29Drop,
                "Fps30" => SmpteRate::Fps30,
                _ => return Err(MidiShapeError::InvalidSmfFile),
            };
            ticks
                .parse::<u8>()
                .ok()
                .and_then(|ticks| SmfDivision::smpte(rate, ticks))
                .ok_or(MidiShapeError::InvalidSmfFile)
        }
        _ => Err(MidiShapeError::InvalidSmfFile),
    }
}
