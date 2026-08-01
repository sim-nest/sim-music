//! Cyclic ordering and rotation for serial parameter tracks.

/// Track categories that can reuse cyclic order/rotation projections.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ParameterTrackKind {
    /// Pitch-class or register parameters.
    Pitch,
    /// Rhythmic values such as durations or attack groups.
    Rhythm,
    /// Dynamic values such as accents or layers.
    Dynamics,
    /// Timbral values such as mute or synthesis patches.
    Timbre,
    /// Orchestration values such as instrument assignments.
    Orchestration,
    /// Harmonic values such as chord colors or voicing states.
    Harmonic,
}

/// One named cyclic source order over a parameter track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CyclicOrder<T> {
    /// The track the cycle controls.
    pub track: ParameterTrackKind,
    /// Source values in declared cyclic order.
    pub values: Vec<T>,
}

/// Projection settings for one cyclic order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CyclicProjectionSpec {
    /// Source-order indices visited before rotation.
    pub order: Vec<usize>,
    /// Left rotation applied to the ordered projection.
    pub rotation: usize,
}

/// Materialized cyclic projection with retained provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CyclicProjection<T> {
    /// The originating track.
    pub track: ParameterTrackKind,
    /// The requested source-order indices.
    pub order: Vec<usize>,
    /// The applied rotation amount modulo the projection length.
    pub rotation: usize,
    /// The projected values after rotation.
    pub values: Vec<T>,
}

/// Projects one cyclic order through an explicit index order and rotation.
pub fn project_cyclic_order<T: Clone>(
    cycle: &CyclicOrder<T>,
    spec: &CyclicProjectionSpec,
) -> Result<CyclicProjection<T>, String> {
    if cycle.values.is_empty() {
        return Err("cyclic source cannot be empty".to_owned());
    }
    if spec.order.is_empty() {
        return Err("cyclic order cannot be empty".to_owned());
    }
    let mut values = Vec::with_capacity(spec.order.len());
    for &index in &spec.order {
        let Some(value) = cycle.values.get(index).cloned() else {
            return Err(format!(
                "cyclic order index {index} is outside source length {}",
                cycle.values.len()
            ));
        };
        values.push(value);
    }
    let rotation = spec.rotation % values.len();
    values.rotate_left(rotation);
    Ok(CyclicProjection {
        track: cycle.track,
        order: spec.order.clone(),
        rotation,
        values,
    })
}
