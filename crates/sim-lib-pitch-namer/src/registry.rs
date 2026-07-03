use sim_lib_pitch_namer_forte::lookup_forte_label;
use sim_lib_pitch_namer_jazz::match_jazz_symbol;
use sim_lib_pitch_namer_riemann::label_riemann;
use sim_lib_pitch_namer_roman::label_roman;
use sim_lib_pitch_set::PitchClassMask;

use crate::set_theory::label_mask as label_set_theory_mask;
use crate::{ClusterLabel, ClusterMeta, ClusterNamer, LabelContext, NamingSchool, PitchNamerError};

/// A registry of [`ClusterNamer`]s that can label a set in every registered school
/// and translate labels between schools.
#[derive(Default)]
pub struct NamerRegistry {
    namers: Vec<Box<dyn ClusterNamer>>,
}

impl NamerRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry populated with all five built-in naming schools.
    pub fn new_with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(BuiltinNamer::Forte);
        registry.register(BuiltinNamer::FunctionalRoman);
        registry.register(BuiltinNamer::SetTheory);
        registry.register(BuiltinNamer::Riemannian);
        registry.register(BuiltinNamer::Jazz);
        registry
    }

    /// Registers an additional namer.
    pub fn register<N>(&mut self, namer: N)
    where
        N: ClusterNamer + 'static,
    {
        self.namers.push(Box::new(namer));
    }

    /// Labels `mask` with every registered namer, returning one label per school.
    pub fn label_all(&self, mask: PitchClassMask, context: &LabelContext) -> Vec<ClusterLabel> {
        self.namers
            .iter()
            .map(|namer| namer.label(mask, context))
            .collect()
    }

    /// Re-labels `label`'s underlying set in the target `school`.
    ///
    /// Returns [`PitchNamerError::UnknownSchool`] if no namer for `school` is
    /// registered.
    pub fn translate(
        &self,
        label: &ClusterLabel,
        school: NamingSchool,
    ) -> Result<ClusterLabel, PitchNamerError> {
        let namer = self
            .namers
            .iter()
            .find(|namer| namer.school() == school)
            .ok_or_else(|| PitchNamerError::UnknownSchool(school.to_string()))?;
        Ok(namer.label(
            label.meta.canonical_mask,
            &LabelContext {
                root: label.meta.root,
                key: label.meta.key,
            },
        ))
    }
}

#[derive(Copy, Clone)]
enum BuiltinNamer {
    Forte,
    FunctionalRoman,
    SetTheory,
    Riemannian,
    Jazz,
}

impl ClusterNamer for BuiltinNamer {
    fn school(&self) -> NamingSchool {
        match self {
            Self::Forte => NamingSchool::Forte,
            Self::FunctionalRoman => NamingSchool::FunctionalRoman,
            Self::SetTheory => NamingSchool::SetTheory,
            Self::Riemannian => NamingSchool::Riemannian,
            Self::Jazz => NamingSchool::Jazz,
        }
    }

    fn label(&self, mask: PitchClassMask, context: &LabelContext) -> ClusterLabel {
        let canonical_mask = mask.normalize();
        let text = match self {
            Self::Forte => lookup_forte_label(canonical_mask)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("prime:{}", label_set_theory_mask(canonical_mask))),
            Self::FunctionalRoman => label_roman(mask, context.key, context.root)
                .unwrap_or_else(|diagnostic| format!("roman:? [{diagnostic}]")),
            Self::SetTheory => label_set_theory_mask(canonical_mask),
            Self::Riemannian => {
                label_riemann(mask, context.root).unwrap_or_else(|| "riemann:?".to_owned())
            }
            Self::Jazz => match_jazz_symbol(mask, context.root)
                .map(|symbol| symbol.to_string())
                .unwrap_or_else(|| "jazz:?".to_owned()),
        };
        let diagnostic = match self {
            Self::FunctionalRoman if context.key.is_none() => {
                Some("key context required".to_owned())
            }
            _ => None,
        };
        ClusterLabel {
            school: self.school(),
            text,
            meta: ClusterMeta {
                canonical_mask,
                root: context.root,
                key: context.key,
                diagnostic,
            },
        }
    }
}
