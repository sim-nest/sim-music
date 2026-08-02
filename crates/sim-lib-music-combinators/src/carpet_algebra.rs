use std::collections::BTreeMap;

use sim_lib_music_core::{Music, Par};
use sim_lib_music_transform::TransformChain;

use crate::{
    CarpetAxis, CarpetError, CarpetTransformDiagnostic, CarpetTransformReport, MusicCarpet,
    OverlayPolicy, SlicePolicy,
};

impl MusicCarpet {
    /// Overlays an identically shaped carpet under an explicit collision policy.
    pub fn overlay(&self, overlay: &Self, collision: OverlayPolicy) -> Result<Self, CarpetError> {
        if self.axes != overlay.axes {
            return Err(CarpetError::AxisMismatch);
        }
        let mut cells = self.cells.clone();
        for (index, top) in &overlay.cells {
            match cells.remove(index) {
                None => {
                    cells.insert(index.clone(), top.clone());
                }
                Some(base) => {
                    let music = match collision {
                        OverlayPolicy::Reject => {
                            return Err(CarpetError::OverlayCollision {
                                index: index.clone(),
                            });
                        }
                        OverlayPolicy::KeepBase => base,
                        OverlayPolicy::KeepOverlay => top.clone(),
                        OverlayPolicy::Parallel => Music::Par(Par {
                            children: vec![Box::new(base), Box::new(top.clone())],
                        }),
                    };
                    cells.insert(index.clone(), music);
                }
            }
        }
        Self::new(self.axes.clone(), cells, self.policy)
    }

    /// Reflects a carpet along one axis. Applying the same reflection twice is identity.
    pub fn reflect(&self, axis: usize) -> Result<Self, CarpetError> {
        let length = self
            .axes
            .get(axis)
            .ok_or(CarpetError::InvalidAxisSelection)?
            .len();
        let mut axes = self.axes.clone();
        axes[axis].labels.reverse();
        let cells = self
            .cells
            .iter()
            .map(|(index, music)| {
                let mut reflected = index.clone();
                reflected.coordinates[axis] = length - 1 - reflected.coordinates[axis];
                (reflected, music.clone())
            })
            .collect();
        Self::new(axes, cells, self.policy)
    }

    /// Rotates a two-axis plane clockwise by `quarter_turns`.
    pub fn rotate(
        &self,
        first_axis: usize,
        second_axis: usize,
        quarter_turns: u8,
    ) -> Result<Self, CarpetError> {
        if first_axis == second_axis
            || first_axis >= self.axes.len()
            || second_axis >= self.axes.len()
        {
            return Err(CarpetError::InvalidAxisSelection);
        }
        let mut output = self.clone();
        for _ in 0..quarter_turns % 4 {
            output = output.rotate_once(first_axis, second_axis)?;
        }
        Ok(output)
    }

    /// Takes a finite window from one axis under an explicit boundary policy.
    pub fn slice(
        &self,
        axis: usize,
        start: usize,
        length: usize,
        boundary: SlicePolicy,
    ) -> Result<Self, CarpetError> {
        let source_axis = self
            .axes
            .get(axis)
            .ok_or(CarpetError::InvalidAxisSelection)?;
        let selection = slice_selection(source_axis, start, length, boundary)?;
        let mut axes = self.axes.clone();
        axes[axis] = CarpetAxis::new(
            source_axis.name.clone(),
            selection
                .iter()
                .map(|coordinate| source_axis.labels[*coordinate].clone())
                .collect(),
            false,
        );
        let mut cells = BTreeMap::new();
        for (new_coordinate, old_coordinate) in selection.into_iter().enumerate() {
            for (index, music) in &self.cells {
                if index.coordinates[axis] == old_coordinate {
                    let mut sliced = index.clone();
                    sliced.coordinates[axis] = new_coordinate;
                    cells.insert(sliced, music.clone());
                }
            }
        }
        Self::new(axes, cells, self.policy)
    }

    /// Applies an existing music-transform chain to every occupied cell.
    pub fn apply_transform(
        &self,
        chain: &TransformChain,
    ) -> Result<CarpetTransformReport, CarpetError> {
        let mut cells = BTreeMap::new();
        let mut diagnostics = Vec::new();
        for (index, music) in &self.cells {
            let report = chain
                .apply_report(music)
                .map_err(|error| CarpetError::Transform {
                    index: index.clone(),
                    detail: error.to_string(),
                })?;
            diagnostics.extend(report.diagnostics.into_iter().map(|diagnostic| {
                CarpetTransformDiagnostic {
                    index: index.clone(),
                    diagnostic,
                }
            }));
            cells.insert(index.clone(), report.music);
        }
        Ok(CarpetTransformReport {
            carpet: Self::new(self.axes.clone(), cells, self.policy)?,
            diagnostics,
        })
    }

    fn rotate_once(&self, first_axis: usize, second_axis: usize) -> Result<Self, CarpetError> {
        let first_len = self.axes[first_axis].len();
        let mut axes = self.axes.clone();
        axes[first_axis] = self.axes[second_axis].clone();
        axes[second_axis] = self.axes[first_axis].clone();
        axes[second_axis].labels.reverse();
        let cells = self
            .cells
            .iter()
            .map(|(index, music)| {
                let mut rotated = index.clone();
                rotated.coordinates[first_axis] = index.coordinates[second_axis];
                rotated.coordinates[second_axis] = first_len - 1 - index.coordinates[first_axis];
                (rotated, music.clone())
            })
            .collect();
        Self::new(axes, cells, self.policy)
    }
}

fn slice_selection(
    axis: &CarpetAxis,
    start: usize,
    length: usize,
    policy: SlicePolicy,
) -> Result<Vec<usize>, CarpetError> {
    match policy {
        SlicePolicy::Strict => {
            let end = start.checked_add(length).ok_or(CarpetError::InvalidSlice {
                start,
                length,
                axis_length: axis.len(),
            })?;
            if end > axis.len() {
                return Err(CarpetError::InvalidSlice {
                    start,
                    length,
                    axis_length: axis.len(),
                });
            }
            Ok((start..end).collect())
        }
        SlicePolicy::Clamp => {
            let start = start.min(axis.len());
            let end = start.saturating_add(length).min(axis.len());
            Ok((start..end).collect())
        }
        SlicePolicy::WrapCyclic if axis.cyclic && !axis.is_empty() => {
            let first = start % axis.len();
            Ok((0..length)
                .map(|offset| (first + offset % axis.len()) % axis.len())
                .collect())
        }
        SlicePolicy::WrapCyclic => Err(CarpetError::InvalidSlice {
            start,
            length,
            axis_length: axis.len(),
        }),
    }
}
