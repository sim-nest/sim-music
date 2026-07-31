use std::collections::{BTreeMap, BTreeSet};

use sim_lib_discrete_graph::{Directedness, Graph, connected_components};
use sim_lib_music_core::{
    AtomRef, Counterpoint, Melody, MelodyItem, Music, MusicObject, Rest, Time, TimedAtom,
};
use sim_lib_music_transform::{augment, pitch_invert, retrograde, retrograde_invert, transpose};

use crate::{
    ContrapuntalForm, NoteEvidence, OverlapEvidence, StrettoChain, StrettoCluster,
    StrettoCompatibility, StrettoCouple, StrettoEntry, StrettoError, StrettoFusion, StrettoGraph,
    StrettoPolicy, StrettoRejection, StrettoTransform, TimeSpan, Violation, analyze_counterpoint,
};

/// Builds a bounded compatibility graph, maximal cliques, and cluster chains.
pub fn stretto_graph(
    subject: &Melody,
    policy: StrettoPolicy,
) -> Result<StrettoGraph, StrettoError> {
    validate_policy(&policy)?;
    let entries = candidate_entries(subject, &policy)?;
    let mut compatibility = Graph::with_nodes(entries, Directedness::Undirected);
    let mut couples = Vec::new();
    let mut rejections = Vec::new();
    for first in 0..compatibility.node_count() {
        for second in first + 1..compatibility.node_count() {
            let pair = pairwise(
                &compatibility.nodes[first],
                &compatibility.nodes[second],
                &policy,
            )?;
            if pair.compatible {
                let weight = StrettoCompatibility {
                    overlap: pair.overlap,
                    violation_count: 0,
                };
                let edge_id = compatibility.add_edge(first, second, weight.clone())?;
                debug_assert_eq!(edge_id, couples.len());
                couples.push(StrettoCouple {
                    leader: first,
                    follower: second,
                    compatibility: weight,
                });
            } else {
                rejections.push(StrettoRejection {
                    first,
                    second,
                    overlap: pair.overlap,
                    violations: pair.violations,
                });
            }
        }
    }
    let components = connected_components(&compatibility)?;
    let clique_nodes = maximal_cliques(
        &compatibility,
        policy.minimum_cluster_voices,
        policy.max_clusters,
    )?;
    let mut clusters = Vec::with_capacity(clique_nodes.len());
    for nodes in clique_nodes {
        let edge_ids = clique_edge_ids(&compatibility, &nodes);
        let cluster_entries = nodes
            .iter()
            .map(|index| compatibility.nodes[*index].clone())
            .collect::<Vec<_>>();
        clusters.push(StrettoCluster {
            entries: nodes,
            edge_ids,
            fusion: fuse_stretto_entries(&cluster_entries)?,
        });
    }
    let chain_graph = cluster_graph(&clusters, &compatibility.nodes)?;
    let chains = longest_chains(
        &chain_graph,
        &clusters,
        policy.max_chain_length,
        policy.max_clusters,
    )?;
    Ok(StrettoGraph {
        compatibility,
        couples,
        rejections,
        components,
        clusters,
        chain_graph,
        chains,
        provenance: vec![
            "mode=derived-stretto-analysis".to_owned(),
            "generation=false".to_owned(),
            "graph=sim-lib-discrete-graph/Graph".to_owned(),
            "components=sim-lib-discrete-graph/connected_components".to_owned(),
            "transforms=sim-lib-music-transform".to_owned(),
            "pairwise-rules=analyze_counterpoint".to_owned(),
            "cliques=maximal-pairwise-compatible".to_owned(),
            "chain-overlap=largest-normalized-suffix-prefix".to_owned(),
        ],
    })
}

/// Materializes one transform request through `sim-lib-music-transform`.
pub fn materialize_transform(
    subject: &Melody,
    request: &StrettoTransform,
) -> Result<Melody, StrettoError> {
    if request.duration_factor <= Time::from_integer(0) {
        return Err(StrettoError::InvalidPolicy(
            "transform duration factor must be positive".to_owned(),
        ));
    }
    if request.form == ContrapuntalForm::Original
        && request.transposition == 0
        && request.duration_factor == Time::from_integer(1)
    {
        return Ok(subject.clone());
    }
    let source = Music::Melody(subject.clone());
    let formed = match request.form {
        ContrapuntalForm::Original => source,
        ContrapuntalForm::Retrograde => retrograde(&source)?,
        ContrapuntalForm::Inversion { axis } => pitch_invert(&source, axis)?,
        ContrapuntalForm::RetrogradeInversion { axis } => retrograde_invert(&source, axis)?,
    };
    let timed = if request.duration_factor == Time::from_integer(1) {
        formed
    } else {
        augment(&formed, request.duration_factor)?
    };
    let pitched = if request.transposition == 0 {
        timed
    } else {
        transpose(&timed, request.transposition)?
    };
    let duration = subject.total_duration() * request.duration_factor;
    melody_from_music(&pitched, duration)
}

/// Fuses entries into a counterpoint view without claiming generated material.
pub fn fuse_stretto_entries(entries: &[StrettoEntry]) -> Result<StrettoFusion, StrettoError> {
    let mut ordered = entries.to_vec();
    ordered.sort_by(|left, right| left.delay.cmp(&right.delay).then(left.id.cmp(&right.id)));
    let voices = ordered
        .iter()
        .map(|entry| delayed_melody(&entry.melody, entry.delay))
        .collect::<Result<Vec<_>, _>>()?;
    let names = ordered
        .iter()
        .map(|entry| format!("Stretto entry {}", entry.id))
        .collect::<Vec<_>>();
    Ok(StrettoFusion {
        counterpoint: Counterpoint::new(voices, names)?,
        entry_ids: ordered.iter().map(|entry| entry.id).collect(),
        mode: "derived-analysis-not-generation".to_owned(),
        provenance: ordered
            .iter()
            .map(|entry| {
                format!(
                    "entry={}; delay={}; form={:?}; transpose={}; duration-factor={}",
                    entry.id,
                    rational(entry.delay),
                    entry.transform.form,
                    entry.transform.transposition,
                    rational(entry.transform.duration_factor)
                )
            })
            .collect(),
    })
}

/// Returns the largest normalized suffix/prefix overlap between two clusters.
///
/// Returning the largest relation deliberately corrects the cataloged legacy
/// behavior, which stopped at the first (smallest) match.
pub fn cluster_overlap(
    left: &StrettoCluster,
    right: &StrettoCluster,
    entries: &[StrettoEntry],
) -> usize {
    let left = normalized_signature(&left.entries, entries);
    let right = normalized_signature(&right.entries, entries);
    let limit = left.len().min(right.len());
    (1..=limit)
        .rev()
        .find(|size| {
            normalize_slice(&left[left.len() - size..]) == normalize_slice(&right[..*size])
        })
        .unwrap_or(0)
}

struct Pairwise {
    compatible: bool,
    overlap: OverlapEvidence,
    violations: Vec<Violation>,
}

fn validate_policy(policy: &StrettoPolicy) -> Result<(), StrettoError> {
    if policy.minimum_overlap <= Time::from_integer(0) {
        return Err(StrettoError::InvalidPolicy(
            "minimum overlap must be positive".to_owned(),
        ));
    }
    if policy
        .delays
        .iter()
        .any(|delay| *delay < Time::from_integer(0))
    {
        return Err(StrettoError::InvalidPolicy(
            "entry delays cannot be negative".to_owned(),
        ));
    }
    if policy.transforms.is_empty() {
        return Err(StrettoError::InvalidPolicy(
            "at least one transform is required".to_owned(),
        ));
    }
    if policy
        .transforms
        .iter()
        .any(|transform| transform.duration_factor <= Time::from_integer(0))
    {
        return Err(StrettoError::InvalidPolicy(
            "transform duration factors must be positive".to_owned(),
        ));
    }
    if policy.max_entries == 0
        || policy.minimum_cluster_voices < 2
        || policy.max_clusters == 0
        || policy.max_chain_length < 2
    {
        return Err(StrettoError::InvalidPolicy(
            "entry/cluster/chain bounds must be non-zero and structurally usable".to_owned(),
        ));
    }
    policy
        .compatibility_rules
        .validate()
        .map_err(|error| StrettoError::InvalidPolicy(error.to_string()))
}

fn candidate_entries(
    subject: &Melody,
    policy: &StrettoPolicy,
) -> Result<Vec<StrettoEntry>, StrettoError> {
    let mut entries = vec![StrettoEntry {
        id: 0,
        delay: Time::from_integer(0),
        transform: StrettoTransform::original(0),
        melody: subject.clone(),
    }];
    let mut seen = BTreeSet::from([(
        Time::from_integer(0),
        transform_key(&StrettoTransform::original(0)),
    )]);
    'delays: for delay in &policy.delays {
        for transform in &policy.transforms {
            if entries.len() >= policy.max_entries {
                break 'delays;
            }
            let key = (*delay, transform_key(transform));
            if !seen.insert(key) {
                continue;
            }
            let melody = materialize_transform(subject, transform)?;
            let overlap = subject
                .total_duration()
                .min(*delay + melody.total_duration())
                - *delay;
            if overlap < policy.minimum_overlap {
                continue;
            }
            entries.push(StrettoEntry {
                id: entries.len(),
                delay: *delay,
                transform: transform.clone(),
                melody,
            });
        }
    }
    Ok(entries)
}

fn pairwise(
    first: &StrettoEntry,
    second: &StrettoEntry,
    policy: &StrettoPolicy,
) -> Result<Pairwise, StrettoError> {
    let start = first.delay.max(second.delay);
    let end = (first.delay + first.melody.total_duration())
        .min(second.delay + second.melody.total_duration());
    let admitted_end = end.max(start);
    let minimum_met = end - start >= policy.minimum_overlap;
    let base = first.delay.min(second.delay);
    let cp = Counterpoint::new(
        vec![
            delayed_melody(&first.melody, first.delay - base)?,
            delayed_melody(&second.melody, second.delay - base)?,
        ],
        vec![
            format!("Entry {}", first.id),
            format!("Entry {}", second.id),
        ],
    )?;
    let report = analyze_counterpoint(&cp, &policy.compatibility_rules);
    let relative_overlap = TimeSpan::new(start - base, admitted_end - base);
    let violations = report
        .violations
        .into_iter()
        .filter(|violation| intersects(&violation.span, &relative_overlap))
        .map(|violation| shift_violation(violation, base))
        .collect::<Vec<_>>();
    let simultaneous = report
        .alignment
        .iter()
        .filter(|window| window.notes.len() >= 2 && intersects(&window.span, &relative_overlap))
        .collect::<Vec<_>>();
    let mut histogram = BTreeMap::<u8, usize>::new();
    for window in &simultaneous {
        let first = window.notes[0].pitch.semitone();
        let second = window.notes[1].pitch.semitone();
        let distance = (second - first).unsigned_abs() as u8 % 12;
        let class = distance.min(12 - distance);
        *histogram.entry(class).or_default() += 1;
    }
    let overlap = OverlapEvidence {
        span: TimeSpan::new(start, admitted_end),
        simultaneous_windows: simultaneous.len(),
        interval_classes: histogram.into_iter().collect(),
        facts: vec![
            format!("entries={},{}", first.id, second.id),
            format!("minimum-overlap={}", rational(policy.minimum_overlap)),
            format!("minimum-overlap-met={minimum_met}"),
            format!("rule-set={}", policy.compatibility_rules.id),
            "alignment=exact-rational-half-open".to_owned(),
        ],
    };
    Ok(Pairwise {
        compatible: minimum_met && violations.is_empty(),
        overlap,
        violations,
    })
}

fn melody_from_music(music: &Music, duration: Time) -> Result<Melody, StrettoError> {
    let mut atoms = Vec::<TimedAtom<'_>>::new();
    music.voices(Time::from_integer(0), &mut atoms);
    let mut notes = atoms
        .into_iter()
        .filter_map(|atom| match atom.atom {
            AtomRef::Note(note) => Some((atom.onset, note)),
            AtomRef::Rest(_) | AtomRef::Phantom(_) => None,
        })
        .collect::<Vec<_>>();
    notes.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.pitch.cmp(&right.1.pitch))
    });
    let mut cursor = Time::from_integer(0);
    let mut items = Vec::new();
    for (onset, note) in notes {
        if onset < cursor {
            return Err(StrettoError::NonMonophonic(format!(
                "note at {} overlaps prior release {}",
                rational(onset),
                rational(cursor)
            )));
        }
        if onset > cursor {
            items.push(MelodyItem::Rest(Rest::new(onset - cursor)?));
        }
        cursor = onset + note.duration;
        items.push(MelodyItem::Note(note));
    }
    if duration > cursor {
        items.push(MelodyItem::Rest(Rest::new(duration - cursor)?));
    }
    Ok(Melody::new(items)?)
}

fn delayed_melody(melody: &Melody, delay: Time) -> Result<Melody, StrettoError> {
    let mut items =
        Vec::with_capacity(melody.items.len() + usize::from(delay > Time::from_integer(0)));
    if delay > Time::from_integer(0) {
        items.push(MelodyItem::Rest(Rest::new(delay)?));
    }
    items.extend(melody.items.clone());
    Ok(Melody::new(items)?)
}

fn maximal_cliques<N, W>(
    graph: &Graph<N, W>,
    minimum: usize,
    limit: usize,
) -> Result<Vec<Vec<usize>>, StrettoError> {
    let adjacency = (0..graph.node_count())
        .map(|node| {
            Ok(graph
                .neighbors(node)?
                .into_iter()
                .map(|neighbor| neighbor.node)
                .collect::<BTreeSet<_>>())
        })
        .collect::<Result<Vec<_>, sim_lib_discrete_graph::GraphError>>()?;
    let mut output = Vec::new();
    bron_kerbosch(
        Vec::new(),
        (0..graph.node_count()).collect(),
        BTreeSet::new(),
        &adjacency,
        minimum,
        limit,
        &mut output,
    );
    output.sort();
    Ok(output)
}

fn bron_kerbosch(
    selected: Vec<usize>,
    mut candidates: BTreeSet<usize>,
    mut excluded: BTreeSet<usize>,
    adjacency: &[BTreeSet<usize>],
    minimum: usize,
    limit: usize,
    output: &mut Vec<Vec<usize>>,
) {
    if output.len() >= limit {
        return;
    }
    if candidates.is_empty() && excluded.is_empty() {
        if selected.len() >= minimum {
            output.push(selected);
        }
        return;
    }
    let ordered = candidates.iter().copied().collect::<Vec<_>>();
    for node in ordered {
        if output.len() >= limit {
            break;
        }
        let mut next = selected.clone();
        next.push(node);
        bron_kerbosch(
            next,
            candidates.intersection(&adjacency[node]).copied().collect(),
            excluded.intersection(&adjacency[node]).copied().collect(),
            adjacency,
            minimum,
            limit,
            output,
        );
        candidates.remove(&node);
        excluded.insert(node);
    }
}

fn clique_edge_ids<N, W>(graph: &Graph<N, W>, nodes: &[usize]) -> Vec<usize> {
    let members = nodes.iter().copied().collect::<BTreeSet<_>>();
    graph
        .edges
        .iter()
        .filter(|edge| members.contains(&edge.source) && members.contains(&edge.target))
        .map(|edge| edge.id)
        .collect()
}

fn cluster_graph(
    clusters: &[StrettoCluster],
    entries: &[StrettoEntry],
) -> Result<Graph<usize, usize>, StrettoError> {
    let mut graph = Graph::with_nodes((0..clusters.len()).collect(), Directedness::Directed);
    for first in 0..clusters.len() {
        for second in 0..clusters.len() {
            if first == second {
                continue;
            }
            let overlap = cluster_overlap(&clusters[first], &clusters[second], entries);
            if overlap > 0 {
                graph.add_edge(first, second, overlap)?;
            }
        }
    }
    Ok(graph)
}

fn longest_chains(
    graph: &Graph<usize, usize>,
    clusters: &[StrettoCluster],
    max_length: usize,
    max_results: usize,
) -> Result<Vec<StrettoChain>, StrettoError> {
    let mut paths = Vec::new();
    for start in 0..graph.node_count() {
        extend_chain(
            graph,
            vec![start],
            Vec::new(),
            max_length,
            max_results,
            &mut paths,
        )?;
        if paths.len() >= max_results {
            break;
        }
    }
    paths.retain(|(path, _)| path.len() >= 2);
    let longest = paths.iter().map(|(path, _)| path.len()).max().unwrap_or(0);
    paths.retain(|(path, _)| path.len() == longest);
    paths.sort();
    paths.dedup();
    Ok(paths
        .into_iter()
        .map(|(path, overlaps)| {
            let mut fused = clusters[path[0]].entries.clone();
            for (position, cluster_index) in path.iter().enumerate().skip(1) {
                let overlap = overlaps[position - 1].min(fused.len());
                fused.truncate(fused.len() - overlap);
                fused.extend(clusters[*cluster_index].entries.iter().copied());
            }
            StrettoChain {
                clusters: path,
                overlaps,
                fused_entries: fused,
            }
        })
        .collect())
}

fn extend_chain(
    graph: &Graph<usize, usize>,
    path: Vec<usize>,
    overlaps: Vec<usize>,
    max_length: usize,
    max_results: usize,
    output: &mut Vec<(Vec<usize>, Vec<usize>)>,
) -> Result<(), StrettoError> {
    if output.len() >= max_results {
        return Ok(());
    }
    let current = *path.last().expect("chain paths are non-empty");
    let extensions = graph
        .neighbors(current)?
        .into_iter()
        .filter(|neighbor| !path.contains(&neighbor.node))
        .map(|neighbor| (neighbor.node, *neighbor.weight))
        .collect::<Vec<_>>();
    if extensions.is_empty() || path.len() >= max_length {
        output.push((path, overlaps));
        return Ok(());
    }
    for (next, overlap) in extensions {
        let mut next_path = path.clone();
        next_path.push(next);
        let mut next_overlaps = overlaps.clone();
        next_overlaps.push(overlap);
        extend_chain(
            graph,
            next_path,
            next_overlaps,
            max_length,
            max_results,
            output,
        )?;
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EntrySignature {
    delay: Time,
    transposition: i32,
    form: String,
    duration_factor: Time,
}

fn normalized_signature(indices: &[usize], entries: &[StrettoEntry]) -> Vec<EntrySignature> {
    let mut signature = indices
        .iter()
        .map(|index| {
            let entry = &entries[*index];
            EntrySignature {
                delay: entry.delay,
                transposition: entry.transform.transposition,
                form: form_key(entry.transform.form),
                duration_factor: entry.transform.duration_factor,
            }
        })
        .collect::<Vec<_>>();
    signature.sort();
    normalize_slice(&signature)
}

fn normalize_slice(signature: &[EntrySignature]) -> Vec<EntrySignature> {
    let Some(anchor) = signature.first() else {
        return Vec::new();
    };
    signature
        .iter()
        .map(|entry| EntrySignature {
            delay: entry.delay - anchor.delay,
            transposition: (entry.transposition - anchor.transposition).rem_euclid(12),
            form: entry.form.clone(),
            duration_factor: entry.duration_factor,
        })
        .collect()
}

fn transform_key(transform: &StrettoTransform) -> (String, i32, Time) {
    (
        form_key(transform.form),
        transform.transposition,
        transform.duration_factor,
    )
}

fn form_key(form: ContrapuntalForm) -> String {
    match form {
        ContrapuntalForm::Original => "original".to_owned(),
        ContrapuntalForm::Retrograde => "retrograde".to_owned(),
        ContrapuntalForm::Inversion { axis } => {
            format!("inversion@{}", axis.semitone())
        }
        ContrapuntalForm::RetrogradeInversion { axis } => {
            format!("retrograde-inversion@{}", axis.semitone())
        }
    }
}

fn shift_violation(mut violation: Violation, offset: Time) -> Violation {
    violation.span.start += offset;
    violation.span.end += offset;
    for note in &mut violation.notes {
        shift_note(note, offset);
    }
    violation
}

fn shift_note(note: &mut NoteEvidence, offset: Time) {
    note.span.start += offset;
    note.span.end += offset;
}

fn intersects(left: &TimeSpan, right: &TimeSpan) -> bool {
    left.start < right.end && right.start < left.end
}

fn rational(value: Time) -> String {
    format!("{}/{}", value.numer(), value.denom())
}
