//! Finite rotation and bounded nesting helpers for serial techniques.

use thiserror::Error;

/// One recursively nestable serial value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NestedSerialValue<T> {
    /// One terminal value.
    Value(T),
    /// One nested serial group.
    Group(Vec<NestedSerialValue<T>>),
}

/// Explicit safety limits for recursive serial expansion.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NestingLimits {
    /// Maximum recursive group depth, counting the outermost sequence as depth 1.
    pub max_depth: usize,
    /// Maximum number of terminal values emitted by expansion.
    pub max_output: usize,
}

/// Result of one bounded nesting expansion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NestingExpansion<T> {
    /// Maximum depth encountered while traversing the source.
    pub depth_reached: usize,
    /// Expanded terminal values in left-to-right order.
    pub values: Vec<T>,
}

/// Failure while expanding a finite nested serial pattern.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Error)]
pub enum NestingError {
    /// The requested limit admitted no nesting depth.
    #[error("nesting depth limit must be at least 1")]
    ZeroDepthLimit,
    /// The requested limit admitted no output.
    #[error("nesting output limit must be at least 1")]
    ZeroOutputLimit,
    /// Expansion encountered a deeper group than permitted.
    #[error("nesting depth {depth} exceeds limit {max_depth}")]
    DepthExceeded {
        /// Observed depth at the failing group.
        depth: usize,
        /// Maximum permitted depth.
        max_depth: usize,
    },
    /// Expansion would emit more terminal values than permitted.
    #[error("nesting output would exceed limit {max_output}")]
    OutputExceeded {
        /// Maximum permitted output cardinality.
        max_output: usize,
    },
}

/// Returns a left-rotated copy of `values`, reduced modulo `values.len()`.
pub fn rotate_sequence_left<T: Clone>(values: &[T], steps: usize) -> Vec<T> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut rotated = values.to_vec();
    let len = rotated.len();
    rotated.rotate_left(steps % len);
    rotated
}

/// Expands one finite nested serial pattern under explicit depth and output limits.
pub fn expand_nested<T: Clone>(
    source: &[NestedSerialValue<T>],
    limits: NestingLimits,
) -> Result<NestingExpansion<T>, NestingError> {
    if limits.max_depth == 0 {
        return Err(NestingError::ZeroDepthLimit);
    }
    if limits.max_output == 0 {
        return Err(NestingError::ZeroOutputLimit);
    }
    let mut values = Vec::new();
    let mut depth_reached = 1;
    expand_level(source, 1, limits, &mut depth_reached, &mut values)?;
    Ok(NestingExpansion {
        depth_reached,
        values,
    })
}

fn expand_level<T: Clone>(
    source: &[NestedSerialValue<T>],
    depth: usize,
    limits: NestingLimits,
    depth_reached: &mut usize,
    values: &mut Vec<T>,
) -> Result<(), NestingError> {
    if depth > limits.max_depth {
        return Err(NestingError::DepthExceeded {
            depth,
            max_depth: limits.max_depth,
        });
    }
    *depth_reached = (*depth_reached).max(depth);
    for item in source {
        match item {
            NestedSerialValue::Value(value) => {
                if values.len() == limits.max_output {
                    return Err(NestingError::OutputExceeded {
                        max_output: limits.max_output,
                    });
                }
                values.push(value.clone());
            }
            NestedSerialValue::Group(group) => {
                expand_level(group, depth + 1, limits, depth_reached, values)?;
            }
        }
    }
    Ok(())
}
