use std::collections::HashMap;

use sim_lib_sound_timbre::Timbre;

/// A bank mapping MIDI `(bank MSB, bank LSB, program)` selections to timbres,
/// with a fallback for unmapped selections.
///
/// # Examples
///
/// ```
/// use sim_lib_sound_bridge::TimbreBank;
/// use sim_lib_sound_timbre::{pure_sine, sawtooth};
///
/// let mut bank = TimbreBank::new(pure_sine());
/// bank.insert(0, 0, 1, sawtooth(8));
/// assert_eq!(bank.get(0, 0, 1).name, "sawtooth");
/// assert_eq!(bank.get(0, 0, 9).name, "pure_sine");
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct TimbreBank {
    entries: HashMap<(u8, u8, u8), Timbre>,
    fallback: Timbre,
}

impl TimbreBank {
    /// Builds an empty bank with the given fallback timbre.
    pub fn new(fallback: Timbre) -> Self {
        Self {
            entries: HashMap::new(),
            fallback,
        }
    }

    /// Assigns `timbre` to the given bank/program selection.
    pub fn insert(&mut self, bank_msb: u8, bank_lsb: u8, program: u8, timbre: Timbre) {
        self.entries.insert((bank_msb, bank_lsb, program), timbre);
    }

    /// Returns the timbre for the selection, or the fallback if unmapped.
    pub fn get(&self, bank_msb: u8, bank_lsb: u8, program: u8) -> &Timbre {
        self.entries
            .get(&(bank_msb, bank_lsb, program))
            .unwrap_or(&self.fallback)
    }

    /// Returns the fallback timbre.
    pub fn fallback(&self) -> &Timbre {
        &self.fallback
    }

    /// Returns the mapped entries keyed by bank/program selection.
    pub fn entries(&self) -> &HashMap<(u8, u8, u8), Timbre> {
        &self.entries
    }
}
