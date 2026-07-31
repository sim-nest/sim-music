//! Bounded MusicXML `score-partwise` import.

use std::collections::{BTreeMap, BTreeSet};

use num_rational::Ratio;
use roxmltree::{Document, Node, ParsingOptions};
use sim_lib_music_core::{Counterpoint, Melody, MelodyItem, Music, Score, Time};

use crate::{
    model::{
        MusicXmlLimits, NotationError, NotationIdentity, NotationIdentityKind, NotationLoss,
        NotationLossKind, NotationReport, loss_diagnostic, musicxml_error,
    },
    musicxml_note::parse_note,
    musicxml_support::*,
};

/// Imports the bounded MusicXML partwise profile with explicit resource limits.
pub fn import_musicxml_partwise_report(
    source: &[u8],
    limits: MusicXmlLimits,
) -> Result<NotationReport<Score>, NotationError> {
    check_limit("bytes", source.len(), limits.bytes)?;
    let source = std::str::from_utf8(source).map_err(|_| NotationError::InvalidMusicXmlUtf8)?;
    if source.contains("<!DOCTYPE") {
        return Err(musicxml_error(
            "DTD declarations and entity definitions are outside the bounded MusicXML profile",
            None,
        ));
    }
    let nodes_limit = u32::try_from(limits.nodes).unwrap_or(u32::MAX);
    let document = Document::parse_with_options(
        source,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit,
            entity_resolver: None,
        },
    )
    .map_err(|error| NotationError::InvalidMusicXml(error.to_string()))?;
    check_tree_limits(&document, limits)?;

    let root = document.root_element();
    ensure_named(root, "score-partwise")?;
    ensure_namespace(root)?;
    ensure_attrs(root, &["version"])?;
    if root.attribute("version") != Some("4.0") {
        return Err(node_error(
            root,
            "bounded MusicXML profile requires score-partwise version=\"4.0\"",
        ));
    }
    ensure_children(root, &["part-list", "part"])?;

    let part_list = unique_child(root, "part-list")?;
    let part_names = parse_part_list(part_list, limits.parts)?;
    let part_nodes = children_named(root, "part").collect::<Vec<_>>();
    check_limit("parts", part_nodes.len(), limits.parts)?;
    if part_nodes.is_empty() {
        return Err(node_error(
            root,
            "score-partwise requires at least one part",
        ));
    }
    if part_nodes.len() != part_names.len() {
        return Err(node_error(
            root,
            "part-list and score part counts must agree",
        ));
    }

    let mut state = ImportState::new(limits);
    let mut parts = Vec::with_capacity(part_nodes.len());
    for (part_index, part) in part_nodes.into_iter().enumerate() {
        parts.push(parse_part(part, part_index, &part_names, &mut state)?);
    }
    let globals = merge_globals(&parts)?;
    let body = if parts.len() == 1 {
        let part = parts.remove(0);
        if part.name != "Music" {
            state.losses.push(NotationLoss {
                kind: NotationLossKind::PartName,
                canonical_path: Some("part/0".to_owned()),
                detail: format!(
                    "single MusicXML part name {:?} is not carried by a Melody score body",
                    part.name
                ),
            });
        }
        Music::Melody(part.melody)
    } else {
        Music::Counterpoint(Counterpoint::new(
            parts.iter().map(|part| part.melody.clone()).collect(),
            parts.iter().map(|part| part.name.clone()).collect(),
        )?)
    };
    let score = Score::new(globals.tempo, globals.time_signature, globals.key, body)?;
    let diagnostics = state.losses.iter().map(loss_diagnostic).collect();
    Ok(NotationReport {
        value: score,
        diagnostics,
        identities: state.identities,
        losses: state.losses,
    })
}

/// Imports the bounded profile and discards exchange sidecar evidence.
pub fn import_musicxml_partwise(
    source: &[u8],
    limits: MusicXmlLimits,
) -> Result<Score, NotationError> {
    Ok(import_musicxml_partwise_report(source, limits)?.value)
}

struct PartImport {
    id: String,
    name: String,
    melody: Melody,
    globals: Globals,
}

#[derive(Clone, PartialEq, Eq)]
struct Globals {
    tempo: u32,
    time_signature: (u8, u8),
    key: Option<String>,
}

struct ImportState {
    limits: MusicXmlLimits,
    events: usize,
    ids: BTreeSet<String>,
    identities: Vec<NotationIdentity>,
    losses: Vec<NotationLoss>,
}

impl ImportState {
    fn new(limits: MusicXmlLimits) -> Self {
        Self {
            limits,
            events: 0,
            ids: BTreeSet::new(),
            identities: Vec::new(),
            losses: Vec::new(),
        }
    }

    fn retain_id(
        &mut self,
        kind: NotationIdentityKind,
        canonical_path: String,
        xml_id: String,
        node: Node<'_, '_>,
    ) -> Result<(), NotationError> {
        validate_xml_id_at(&xml_id, node)?;
        if !self.ids.insert(xml_id.clone()) {
            return Err(node_error(node, format!("duplicate XML id {xml_id}")));
        }
        self.identities.push(NotationIdentity {
            kind,
            canonical_path,
            xml_id,
        });
        Ok(())
    }

    fn next_event(&mut self) -> Result<(), NotationError> {
        self.events = self.events.saturating_add(1);
        if self.events > self.limits.events {
            return Err(NotationError::MusicXmlLimit {
                limit: "events",
                actual: self.events,
                maximum: self.limits.events,
            });
        }
        Ok(())
    }
}

fn parse_part_list(
    node: Node<'_, '_>,
    parts_limit: usize,
) -> Result<BTreeMap<String, String>, NotationError> {
    ensure_attrs(node, &[])?;
    ensure_children(node, &["score-part"])?;
    let mut parts = BTreeMap::new();
    for score_part in children_named(node, "score-part") {
        check_limit("parts", parts.len().saturating_add(1), parts_limit)?;
        ensure_attrs(score_part, &["id"])?;
        ensure_children(score_part, &["part-name"])?;
        let id = required_attr(score_part, "id")?;
        validate_xml_id_at(id, score_part)?;
        let name = required_text(unique_child(score_part, "part-name")?)?.to_owned();
        if name.trim().is_empty() {
            return Err(node_error(score_part, "part-name cannot be empty"));
        }
        if parts.insert(id.to_owned(), name).is_some() {
            return Err(node_error(score_part, format!("duplicate part id {id}")));
        }
    }
    if parts.is_empty() {
        return Err(node_error(
            node,
            "part-list requires at least one score-part",
        ));
    }
    Ok(parts)
}

fn parse_part(
    node: Node<'_, '_>,
    part_index: usize,
    part_names: &BTreeMap<String, String>,
    state: &mut ImportState,
) -> Result<PartImport, NotationError> {
    ensure_attrs(node, &["id"])?;
    ensure_children(node, &["measure"])?;
    let id = required_attr(node, "id")?.to_owned();
    let name = part_names
        .get(&id)
        .ok_or_else(|| node_error(node, format!("part id {id} is absent from part-list")))?
        .clone();
    state.retain_id(
        NotationIdentityKind::Part,
        format!("part/{part_index}"),
        id.clone(),
        node,
    )?;
    let measures = children_named(node, "measure").collect::<Vec<_>>();
    if measures.is_empty() {
        return Err(node_error(node, "part requires at least one measure"));
    }
    let mut divisions = None;
    let mut time_signature = None;
    let mut key = None;
    let mut tempo = None;
    let mut items = Vec::new();
    for (measure_index, measure) in measures.into_iter().enumerate() {
        ensure_attrs(measure, &["number"])?;
        let number = required_attr(measure, "number")?;
        if number != (measure_index + 1).to_string() {
            return Err(node_error(
                measure,
                "measure numbers must be contiguous decimal integers starting at 1",
            ));
        }
        ensure_children(measure, &["attributes", "direction", "note"])?;
        for child in measure.children().filter(Node::is_element) {
            if !items.is_empty() && matches!(child.tag_name().name(), "attributes" | "direction") {
                return Err(node_error(
                    child,
                    "global attributes and tempo must precede all part events",
                ));
            }
            match child.tag_name().name() {
                "attributes" => parse_attributes(
                    child,
                    &mut divisions,
                    &mut time_signature,
                    &mut key,
                    state,
                    &format!("part/{part_index}"),
                )?,
                "direction" => parse_direction(child, &mut tempo)?,
                "note" => {
                    let event_index = items.len();
                    state.next_event()?;
                    let divisions = divisions.ok_or_else(|| {
                        node_error(child, "attributes/divisions must precede note events")
                    })?;
                    let path = format!("part/{part_index}/event/{event_index}");
                    let ((item, spelling_loss), id) = parse_note(
                        child,
                        divisions,
                        format!("P{}-E{}", part_index + 1, event_index + 1),
                        key.as_deref(),
                    )?;
                    if let Some(detail) = spelling_loss {
                        state.losses.push(NotationLoss {
                            kind: NotationLossKind::PitchSpelling,
                            canonical_path: Some(path.clone()),
                            detail,
                        });
                    }
                    state.retain_id(NotationIdentityKind::Event, path, id, child)?;
                    items.push(item);
                }
                _ => unreachable!("child vocabulary checked above"),
            }
        }
        let meter = time_signature.unwrap_or((4, 4));
        let expected = Ratio::new(i64::from(meter.0), i64::from(meter.1));
        let start = measure_start(&items, measure_index, expected)?;
        let actual = items[start..]
            .iter()
            .fold(Ratio::from_integer(0), |sum, item| {
                sum + item_duration(item)
            });
        if actual != expected {
            return Err(node_error(
                measure,
                format!(
                    "bounded profile requires complete measures; measure {} has duration {actual}, expected {expected}",
                    measure_index + 1
                ),
            ));
        }
    }
    let time_signature = match time_signature {
        Some(value) => value,
        None => {
            state.losses.push(NotationLoss {
                kind: NotationLossKind::DefaultedTimeSignature,
                canonical_path: Some(format!("part/{part_index}")),
                detail: "MusicXML omitted time signature; canonical Score uses 4/4".to_owned(),
            });
            (4, 4)
        }
    };
    let tempo = match tempo {
        Some(value) => value,
        None => {
            state.losses.push(NotationLoss {
                kind: NotationLossKind::DefaultedTempo,
                canonical_path: Some(format!("part/{part_index}")),
                detail: "MusicXML omitted tempo; canonical Score uses 120 BPM".to_owned(),
            });
            120
        }
    };
    Ok(PartImport {
        id,
        name,
        melody: Melody::new(items)?,
        globals: Globals {
            tempo,
            time_signature,
            key,
        },
    })
}

fn measure_start(
    items: &[MelodyItem],
    measure_index: usize,
    duration: Time,
) -> Result<usize, NotationError> {
    let target = duration * Ratio::from_integer(measure_index as i64);
    let mut elapsed = Ratio::from_integer(0);
    for (index, item) in items.iter().enumerate() {
        if elapsed == target {
            return Ok(index);
        }
        elapsed += item_duration(item);
        if elapsed > target {
            return Err(musicxml_error(
                "an event crosses a measure boundary in the bounded profile",
                None,
            ));
        }
    }
    if elapsed == target {
        Ok(items.len())
    } else {
        Err(musicxml_error(
            "measure boundaries do not match the active meter",
            None,
        ))
    }
}

fn parse_attributes(
    node: Node<'_, '_>,
    divisions: &mut Option<i64>,
    time_signature: &mut Option<(u8, u8)>,
    key: &mut Option<String>,
    state: &mut ImportState,
    path: &str,
) -> Result<(), NotationError> {
    ensure_attrs(node, &[])?;
    ensure_children(node, &["divisions", "key", "time", "clef"])?;
    for child in node.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "divisions" => {
                let parsed = parse_positive_i64(required_text(child)?, child, "divisions")?;
                if parsed > MAX_DIVISIONS {
                    return Err(node_error(
                        child,
                        format!("divisions exceeds profile maximum {MAX_DIVISIONS}"),
                    ));
                }
                set_consistent(divisions, parsed, child, "divisions")?;
            }
            "time" => {
                ensure_attrs(child, &[])?;
                ensure_children(child, &["beats", "beat-type"])?;
                let beats = parse_u8(
                    required_text(unique_child(child, "beats")?)?,
                    child,
                    "time beats",
                )?;
                let beat_type = parse_u8(
                    required_text(unique_child(child, "beat-type")?)?,
                    child,
                    "time beat-type",
                )?;
                if beats == 0 || beat_type == 0 {
                    return Err(node_error(child, "time signature values must be positive"));
                }
                set_consistent(time_signature, (beats, beat_type), child, "time signature")?;
            }
            "key" => {
                ensure_attrs(child, &[])?;
                ensure_children(child, &["fifths", "mode"])?;
                let fifths = required_text(unique_child(child, "fifths")?)?
                    .parse::<i8>()
                    .map_err(|_| node_error(child, "key fifths must be an integer"))?;
                let mode = unique_optional_child(child, "mode")?
                    .map(required_text)
                    .transpose()?
                    .unwrap_or("major");
                let parsed = key_from_fifths(fifths, mode)
                    .ok_or_else(|| node_error(child, "unsupported key signature"))?;
                set_consistent(key, parsed, child, "key signature")?;
            }
            "clef" => {
                ensure_attrs(child, &[])?;
                ensure_children(child, &["sign", "line", "clef-octave-change"])?;
                state.losses.push(NotationLoss {
                    kind: NotationLossKind::Clef,
                    canonical_path: Some(path.to_owned()),
                    detail:
                        "MusicXML clef is layout metadata and is not carried by canonical Score"
                            .to_owned(),
                });
            }
            _ => unreachable!("child vocabulary checked above"),
        }
    }
    Ok(())
}

fn parse_direction(node: Node<'_, '_>, tempo: &mut Option<u32>) -> Result<(), NotationError> {
    ensure_attrs(node, &[])?;
    ensure_children(node, &["sound"])?;
    let sound = unique_child(node, "sound")?;
    ensure_attrs(sound, &["tempo"])?;
    ensure_children(sound, &[])?;
    let value = required_attr(sound, "tempo")?
        .parse::<u32>()
        .map_err(|_| node_error(sound, "sound tempo must be a positive integer"))?;
    if value == 0 {
        return Err(node_error(sound, "sound tempo must be positive"));
    }
    set_consistent(tempo, value, sound, "tempo")
}

fn merge_globals(parts: &[PartImport]) -> Result<Globals, NotationError> {
    let first = parts
        .first()
        .expect("caller checks non-empty score parts")
        .globals
        .clone();
    for part in &parts[1..] {
        if part.globals != first {
            return Err(musicxml_error(
                format!(
                    "part {} changes global tempo, key, or time signature",
                    part.id
                ),
                None,
            ));
        }
    }
    Ok(first)
}
