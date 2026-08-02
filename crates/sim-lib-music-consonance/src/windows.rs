use sim_lib_music_core::{Staff, Time};

use crate::source;
use crate::{ConsonanceError, SoundingNote, SoundingWindow, TimeSpan};

/// Builds exact, multiplicity-preserving windows from an identity-bearing staff.
///
/// Every positive-duration interval between consecutive onsets and releases is
/// returned, including silent intervals. A note belongs to a window exactly
/// when its source span contains the window start under half-open semantics.
pub fn sounding_windows(staff: &Staff) -> Result<Vec<SoundingWindow>, ConsonanceError> {
    let material = source::from_staff(staff)?;
    windows_from_notes(&material.notes, material.duration)
}

/// Intersects existing windows with an exact half-open span.
///
/// Source note onsets and releases remain unchanged; only window spans are
/// clipped. This makes repeated slicing associative without corrupting identity
/// or note lifetime evidence.
pub fn slice_sounding_windows(windows: &[SoundingWindow], span: TimeSpan) -> Vec<SoundingWindow> {
    windows
        .iter()
        .filter_map(|window| {
            let start = window.span.start.max(span.start);
            let end = window.span.end.min(span.end);
            (start < end).then(|| SoundingWindow {
                span: TimeSpan { start, end },
                notes: window.notes.clone(),
            })
        })
        .collect()
}

pub(crate) fn windows_from_notes(
    notes: &[SoundingNote],
    duration: Time,
) -> Result<Vec<SoundingWindow>, ConsonanceError> {
    let extent = TimeSpan::new(Time::from_integer(0), duration)?;
    if extent.start == extent.end {
        return Ok(Vec::new());
    }
    let mut boundaries = vec![extent.start, extent.end];
    boundaries.extend(
        notes
            .iter()
            .filter(|note| note.onset < note.release)
            .flat_map(|note| [note.onset, note.release]),
    );
    boundaries.sort();
    boundaries.dedup();
    boundaries
        .windows(2)
        .filter(|pair| pair[0] < pair[1])
        .map(|pair| {
            let span = TimeSpan::new(pair[0], pair[1])?;
            let mut sounding = notes
                .iter()
                .filter(|note| note.onset <= span.start && span.start < note.release)
                .cloned()
                .collect::<Vec<_>>();
            sounding.sort_by(|left, right| {
                left.onset
                    .cmp(&right.onset)
                    .then_with(|| left.pitch.cmp(&right.pitch))
                    .then_with(|| left.voice_id.cmp(&right.voice_id))
                    .then_with(|| left.event_id.cmp(&right.event_id))
            });
            Ok(SoundingWindow {
                span,
                notes: sounding,
            })
        })
        .collect()
}
