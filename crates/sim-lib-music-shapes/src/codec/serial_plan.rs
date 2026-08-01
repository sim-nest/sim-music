//! Text adapter for immutable serial plans.

use std::collections::BTreeMap;

use sim_codec::{DomainForm, DomainValue, parse_domain_form};
use sim_lib_music_core::ObjectId;
use sim_lib_music_serial::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId, SerialOrigin,
    SerialPlan, SerialRole, SimultaneousGroupId,
};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

use super::analysis::{ensure_form, field, field_atom, field_list};
use super::{MusicShapeError, encode_string};

/// Encodes one immutable serial plan as a canonical `#(SerialPlan ...)` form.
pub fn encode_serial_plan(plan: &SerialPlan) -> Result<String, MusicShapeError> {
    let rows = plan
        .rows()
        .iter()
        .map(|(row_id, row)| {
            format!(
                "#(SerialRow id={} family={} addend={} classes=[{}])",
                encode_string(row_id.as_str()),
                row.operation().family.as_str(),
                row.operation().addend,
                row.classes()
                    .iter()
                    .map(|class| class.value().to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let events = plan
        .events()
        .values()
        .map(encode_event)
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let precedence = plan
        .precedence()
        .edges()
        .map(|(before, after)| {
            format!(
                "#(SerialEdge before={} after={})",
                encode_string(before.as_str()),
                encode_string(after.as_str()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "#(SerialPlan rows=[{rows}] events=[{events}] precedence=[{precedence}])"
    ))
}

/// Decodes and semantically validates a `#(SerialPlan ...)` value.
pub fn decode_serial_plan(value: &str) -> Result<SerialPlan, MusicShapeError> {
    let node = parse_domain_form(value)?;
    ensure_form(&node, "SerialPlan")?;
    let rows = field_list(&node, "rows")?
        .iter()
        .map(|value| decode_row(value.as_form()?))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let events = field_list(&node, "events")?
        .iter()
        .map(|value| decode_event(value.as_form()?))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let precedence = field_list(&node, "precedence")?
        .iter()
        .map(|value| decode_edge(value.as_form()?))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SerialPlan::try_new(rows, events, precedence)?)
}

fn encode_event(event: &PlannedSerialEvent) -> Result<String, MusicShapeError> {
    let ordinals = event
        .ordinals
        .iter()
        .map(|ordinal| {
            format!(
                "#(OrdinalRef row_id={} ordinal={})",
                encode_string(ordinal.row_id.as_str()),
                ordinal.ordinal,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let parents = event
        .parents
        .iter()
        .map(|parent| encode_string(parent.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    let placement = encode_placement(&event.placement);
    let (origin_kind, origin_value) = match &event.origin {
        SerialOrigin::Structural { rationale } => ("Structural", rationale.as_str()),
        SerialOrigin::Derived { technique } => ("Derived", technique.as_str()),
        SerialOrigin::Ornamental { technique } => ("Ornamental", technique.as_str()),
        SerialOrigin::External { source } => ("External", source.as_str()),
    };
    Ok(format!(
        "#(SerialEvent id={} ordinals=[{}] role={} origin_kind={} origin_value={} voice={} placement={} parents=[{}])",
        encode_string(event.id.as_str()),
        ordinals,
        event.role.as_str(),
        origin_kind,
        encode_string(origin_value),
        encode_string(event.voice.as_str()),
        placement,
        parents,
    ))
}

fn encode_placement(placement: &EventPlacement) -> String {
    match placement.simultaneous_group() {
        Some(group) => format!(
            "#(EventPlacement kind=Simultaneous group={})",
            encode_string(group.as_str())
        ),
        None => "#(EventPlacement kind=Independent)".to_owned(),
    }
}

fn decode_row(
    node: &DomainForm,
) -> Result<(RowInstanceId, sim_lib_pitch_serial::RowForm), MusicShapeError> {
    ensure_form(node, "SerialRow")?;
    let row_id = RowInstanceId::new(node.field_atom_or_string("id")?)?;
    let family = decode_family(&field_atom(node, "family")?)?;
    let addend = field_atom(node, "addend")?
        .parse::<u8>()
        .map_err(|_| MusicShapeError::InvalidMusic)?;
    let classes = field_list(node, "classes")?
        .iter()
        .map(value_as_u8)
        .collect::<Result<Vec<_>, _>>()?;
    let classes: [sim_lib_music_core::PitchClass; 12] = classes
        .into_iter()
        .map(sim_lib_music_core::PitchClass::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MusicShapeError::InvalidMusic)?
        .try_into()
        .map_err(|_| MusicShapeError::InvalidMusic)?;
    let row = ToneRow::try_from_classes(classes)?;
    Ok((row_id, row.apply(RowOperation::new(family, addend))))
}

fn decode_event(node: &DomainForm) -> Result<(SerialEventId, PlannedSerialEvent), MusicShapeError> {
    ensure_form(node, "SerialEvent")?;
    let event_id = SerialEventId::new(node.field_atom_or_string("id")?)?;
    let ordinals = field_list(node, "ordinals")?
        .iter()
        .map(|value| decode_ordinal_ref(value.as_form()?))
        .collect::<Result<Vec<_>, _>>()?;
    let role = decode_role(&field_atom(node, "role")?)?;
    let origin = decode_origin(
        &field_atom(node, "origin_kind")?,
        node.field_atom_or_string("origin_value")?,
    )?;
    let voice = ObjectId::new(node.field_atom_or_string("voice")?)
        .map_err(|_| MusicShapeError::InvalidMusic)?;
    let placement = decode_placement(field(node, "placement")?.as_form()?)?;
    let parents = field_list(node, "parents")?
        .iter()
        .map(|value| match value {
            DomainValue::String(text) | DomainValue::Atom(text) => {
                SerialEventId::new(text.clone()).map_err(MusicShapeError::from)
            }
            _ => Err(MusicShapeError::InvalidMusic),
        })
        .collect::<Result<Vec<_>, MusicShapeError>>()?;
    let event = PlannedSerialEvent {
        id: event_id.clone(),
        ordinals,
        role,
        origin,
        voice,
        placement,
        parents,
    };
    Ok((event_id, event))
}

fn decode_ordinal_ref(node: &DomainForm) -> Result<OrdinalRef, MusicShapeError> {
    ensure_form(node, "OrdinalRef")?;
    Ok(OrdinalRef::new(
        RowInstanceId::new(node.field_atom_or_string("row_id")?)?,
        field_atom(node, "ordinal")?
            .parse::<usize>()
            .map_err(|_| MusicShapeError::InvalidMusic)?,
    ))
}

fn decode_edge(node: &DomainForm) -> Result<(SerialEventId, SerialEventId), MusicShapeError> {
    ensure_form(node, "SerialEdge")?;
    Ok((
        SerialEventId::new(node.field_atom_or_string("before")?)?,
        SerialEventId::new(node.field_atom_or_string("after")?)?,
    ))
}

fn decode_placement(node: &DomainForm) -> Result<EventPlacement, MusicShapeError> {
    ensure_form(node, "EventPlacement")?;
    match field_atom(node, "kind")?.as_str() {
        "Independent" => Ok(EventPlacement::independent()),
        "Simultaneous" => Ok(EventPlacement::simultaneous(SimultaneousGroupId::new(
            node.field_atom_or_string("group")?,
        )?)),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn decode_role(value: &str) -> Result<SerialRole, MusicShapeError> {
    match value {
        "structural" => Ok(SerialRole::Structural),
        "derived" => Ok(SerialRole::Derived),
        "ornamental" => Ok(SerialRole::Ornamental),
        "external" => Ok(SerialRole::External),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn decode_origin(kind: &str, value: &str) -> Result<SerialOrigin, MusicShapeError> {
    Ok(match kind {
        "Structural" => SerialOrigin::Structural {
            rationale: value.to_owned(),
        },
        "Derived" => SerialOrigin::Derived {
            technique: value.to_owned(),
        },
        "Ornamental" => SerialOrigin::Ornamental {
            technique: value.to_owned(),
        },
        "External" => SerialOrigin::External {
            source: value.to_owned(),
        },
        _ => return Err(MusicShapeError::InvalidMusic),
    })
}

fn decode_family(value: &str) -> Result<RowFamily, MusicShapeError> {
    match value {
        "P" => Ok(RowFamily::P),
        "I" => Ok(RowFamily::I),
        "R" => Ok(RowFamily::R),
        "RI" => Ok(RowFamily::RI),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn value_as_u8(value: &DomainValue) -> Result<u8, MusicShapeError> {
    match value {
        DomainValue::Atom(text) | DomainValue::String(text) => text
            .parse::<u8>()
            .map_err(|_| MusicShapeError::InvalidMusic),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}
