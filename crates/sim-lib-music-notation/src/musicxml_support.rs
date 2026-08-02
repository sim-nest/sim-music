//! Shared bounded-profile validation and exact-time helpers.

use std::collections::{BTreeMap, BTreeSet};

use roxmltree::{Document, Node};
use sim_kernel::Span;
use sim_lib_music_core::{MelodyItem, Time};

use crate::model::{
    MusicXmlLimits, NotationError, NotationIdentity, NotationIdentityKind, musicxml_error,
};

pub(crate) const MAX_DIVISIONS: i64 = 16_384;
const MUSICXML_NAMESPACE: &str = "http://www.musicxml.org/ns/musicxml";

pub(crate) fn item_duration(item: &MelodyItem) -> Time {
    match item {
        MelodyItem::Note(note) => note.duration,
        MelodyItem::Rest(rest) => rest.duration,
    }
}

pub(crate) fn key_from_fifths(fifths: i8, mode: &str) -> Option<String> {
    let tonic = match (mode, fifths) {
        ("major", -7) => "Cb",
        ("major", -6) => "Gb",
        ("major", -5) => "Db",
        ("major", -4) => "Ab",
        ("major", -3) => "Eb",
        ("major", -2) => "Bb",
        ("major", -1) => "F",
        ("major", 0) => "C",
        ("major", 1) => "G",
        ("major", 2) => "D",
        ("major", 3) => "A",
        ("major", 4) => "E",
        ("major", 5) => "B",
        ("major", 6) => "F#",
        ("major", 7) => "C#",
        ("minor", -7) => "Ab",
        ("minor", -6) => "Eb",
        ("minor", -5) => "Bb",
        ("minor", -4) => "F",
        ("minor", -3) => "C",
        ("minor", -2) => "G",
        ("minor", -1) => "D",
        ("minor", 0) => "A",
        ("minor", 1) => "E",
        ("minor", 2) => "B",
        ("minor", 3) => "F#",
        ("minor", 4) => "C#",
        ("minor", 5) => "G#",
        ("minor", 6) => "D#",
        ("minor", 7) => "A#",
        _ => return None,
    };
    Some(format!("{tonic} {mode}"))
}

pub(crate) fn fifths_from_key(key: &str) -> Option<(i8, &'static str)> {
    let (tonic, mode) = if let Some(tonic) = key.strip_suffix(" major") {
        (tonic, "major")
    } else if let Some(tonic) = key.strip_suffix(" minor") {
        (tonic, "minor")
    } else if let Some(tonic) = key.strip_suffix('m') {
        (tonic, "minor")
    } else {
        (key, "major")
    };
    let expected = format!("{} {mode}", tonic.trim());
    (-7..=7)
        .find(|fifths| key_from_fifths(*fifths, mode).as_deref() == Some(expected.as_str()))
        .map(|fifths| (fifths, mode))
}

pub(crate) fn check_tree_limits(
    document: &Document<'_>,
    limits: MusicXmlLimits,
) -> Result<(), NotationError> {
    let mut nodes = 0usize;
    let mut text = 0usize;
    let mut depth = 0usize;
    for node in document.descendants() {
        nodes = nodes.saturating_add(1);
        depth = depth.max(node.ancestors().filter(Node::is_element).count());
        if let Some(value) = node.text().filter(|_| node.is_text()) {
            text = text.saturating_add(value.len());
        }
    }
    check_limit("nodes", nodes, limits.nodes)?;
    check_limit("depth", depth, limits.depth)?;
    check_limit("text", text, limits.text)
}

pub(crate) fn check_limit(
    limit: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), NotationError> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(NotationError::MusicXmlLimit {
            limit,
            actual,
            maximum,
        })
    }
}

pub(crate) fn ensure_namespace(node: Node<'_, '_>) -> Result<(), NotationError> {
    match node.tag_name().namespace() {
        None | Some("") | Some(MUSICXML_NAMESPACE) => Ok(()),
        Some(namespace) => Err(node_error(
            node,
            format!("unsupported MusicXML namespace {namespace}"),
        )),
    }
}

pub(crate) fn ensure_named(node: Node<'_, '_>, expected: &str) -> Result<(), NotationError> {
    if node.tag_name().name() == expected {
        Ok(())
    } else {
        Err(node_error(
            node,
            format!("expected <{expected}>, found <{}>", node.tag_name().name()),
        ))
    }
}

pub(crate) fn ensure_attrs(node: Node<'_, '_>, allowed: &[&str]) -> Result<(), NotationError> {
    for attribute in node.attributes() {
        if attribute.namespace().is_some() || !allowed.contains(&attribute.name()) {
            return Err(node_error(
                node,
                format!(
                    "attribute {} is outside the bounded MusicXML profile",
                    attribute.name()
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn ensure_children(node: Node<'_, '_>, allowed: &[&str]) -> Result<(), NotationError> {
    for child in node.children() {
        if child.is_text() {
            if child.text().is_some_and(|text| !text.trim().is_empty()) {
                return Err(node_error(
                    child,
                    "mixed text is outside the bounded MusicXML profile",
                ));
            }
        } else if child.is_pi() {
            return Err(node_error(
                child,
                "processing instructions are outside the bounded MusicXML profile",
            ));
        } else if child.is_element() {
            ensure_namespace(child)?;
            if !allowed.contains(&child.tag_name().name()) {
                return Err(node_error(
                    child,
                    format!(
                        "element <{}> is outside the bounded MusicXML profile",
                        child.tag_name().name()
                    ),
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn children_named<'a, 'input>(
    node: Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    node.children()
        .filter(move |child| child.is_element() && child.tag_name().name() == name)
}

pub(crate) fn unique_child<'a, 'input>(
    node: Node<'a, 'input>,
    name: &str,
) -> Result<Node<'a, 'input>, NotationError> {
    let mut matches = node
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == name);
    let child = matches
        .next()
        .ok_or_else(|| node_error(node, format!("missing required <{name}>")))?;
    if matches.next().is_some() {
        return Err(node_error(node, format!("expected exactly one <{name}>")));
    }
    Ok(child)
}

pub(crate) fn optional_child<'a, 'input>(
    node: Node<'a, 'input>,
    name: &str,
) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
}

pub(crate) fn unique_optional_child<'a, 'input>(
    node: Node<'a, 'input>,
    name: &str,
) -> Result<Option<Node<'a, 'input>>, NotationError> {
    let mut matches = node
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == name);
    let child = matches.next();
    if matches.next().is_some() {
        return Err(node_error(node, format!("expected at most one <{name}>")));
    }
    Ok(child)
}

pub(crate) fn required_attr<'a>(node: Node<'a, '_>, name: &str) -> Result<&'a str, NotationError> {
    node.attribute(name)
        .ok_or_else(|| node_error(node, format!("missing required attribute {name}")))
}

pub(crate) fn required_text<'a>(node: Node<'a, '_>) -> Result<&'a str, NotationError> {
    let value = node
        .text()
        .ok_or_else(|| node_error(node, "element requires text"))?
        .trim();
    if value.is_empty() {
        Err(node_error(node, "element text cannot be empty"))
    } else {
        Ok(value)
    }
}

pub(crate) fn parse_positive_i64(
    value: &str,
    node: Node<'_, '_>,
    label: &str,
) -> Result<i64, NotationError> {
    let parsed = value
        .parse::<i64>()
        .map_err(|_| node_error(node, format!("{label} must be an integer")))?;
    if parsed > 0 {
        Ok(parsed)
    } else {
        Err(node_error(node, format!("{label} must be positive")))
    }
}

pub(crate) fn parse_u8(value: &str, node: Node<'_, '_>, label: &str) -> Result<u8, NotationError> {
    value
        .parse()
        .map_err(|_| node_error(node, format!("{label} must fit in u8")))
}

pub(crate) fn set_consistent<T: Clone + PartialEq>(
    slot: &mut Option<T>,
    value: T,
    node: Node<'_, '_>,
    label: &str,
) -> Result<(), NotationError> {
    if slot.as_ref().is_some_and(|existing| existing != &value) {
        Err(node_error(
            node,
            format!("{label} changes are outside the bounded profile"),
        ))
    } else {
        *slot = Some(value);
        Ok(())
    }
}

pub(crate) fn identity_map(
    identities: &[NotationIdentity],
) -> Result<BTreeMap<&str, &str>, NotationError> {
    let mut map = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for identity in identities {
        validate_xml_id(&identity.xml_id)?;
        let expected_kind = identity_path_kind(&identity.canonical_path).ok_or_else(|| {
            musicxml_error(
                format!(
                    "invalid canonical identity path {}",
                    identity.canonical_path
                ),
                None,
            )
        })?;
        if identity.kind != expected_kind {
            return Err(musicxml_error(
                format!(
                    "identity kind does not match canonical path {}",
                    identity.canonical_path
                ),
                None,
            ));
        }
        if !ids.insert(identity.xml_id.as_str()) {
            return Err(musicxml_error(
                format!("duplicate identity id {}", identity.xml_id),
                None,
            ));
        }
        if map
            .insert(identity.canonical_path.as_str(), identity.xml_id.as_str())
            .is_some()
        {
            return Err(musicxml_error(
                format!("duplicate identity path {}", identity.canonical_path),
                None,
            ));
        }
    }
    Ok(map)
}

fn identity_path_kind(path: &str) -> Option<NotationIdentityKind> {
    let pieces = path.split('/').collect::<Vec<_>>();
    match pieces.as_slice() {
        ["part", part] if canonical_index(part) => Some(NotationIdentityKind::Part),
        ["part", part, "event", event] if canonical_index(part) && canonical_index(event) => {
            Some(NotationIdentityKind::Event)
        }
        _ => None,
    }
}

fn canonical_index(value: &str) -> bool {
    value
        .parse::<usize>()
        .is_ok_and(|parsed| parsed.to_string() == value)
}

pub(crate) fn ensure_unique_identity_ids(
    identities: &[NotationIdentity],
) -> Result<(), NotationError> {
    let mut ids = BTreeSet::new();
    for identity in identities {
        if !ids.insert(identity.xml_id.as_str()) {
            return Err(musicxml_error(
                format!("duplicate exported identity id {}", identity.xml_id),
                None,
            ));
        }
    }
    Ok(())
}

pub(crate) fn retained_id(identities: &BTreeMap<&str, &str>, path: &str) -> Option<String> {
    identities.get(path).map(|value| (*value).to_owned())
}

pub(crate) fn validate_xml_id(value: &str) -> Result<(), NotationError> {
    if is_xml_id(value) {
        Ok(())
    } else {
        Err(musicxml_error(
            format!("invalid bounded-profile XML id {value:?}"),
            None,
        ))
    }
}

pub(crate) fn validate_xml_id_at(value: &str, node: Node<'_, '_>) -> Result<(), NotationError> {
    if is_xml_id(value) {
        Ok(())
    } else {
        Err(node_error(
            node,
            format!("invalid bounded-profile XML id {value:?}"),
        ))
    }
}

fn is_xml_id(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

pub(crate) fn checked_lcm(left: i64, right: i64) -> Option<i64> {
    let gcd = gcd(left, right);
    left.checked_div(gcd)?.checked_mul(right)
}

fn gcd(mut left: i64, mut right: i64) -> i64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.abs()
}

pub(crate) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(crate) fn node_error(node: Node<'_, '_>, message: impl Into<String>) -> NotationError {
    let range = node.range();
    musicxml_error(
        message,
        Some(Span {
            start: range.start,
            end: range.end,
        }),
    )
}
