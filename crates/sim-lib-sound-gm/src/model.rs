use std::collections::BTreeMap;

use sim_lib_sound_bridge::TimbreBank;
use sim_lib_sound_timbre::{
    bell_inharmonic, fm_pair, karplus_strong, organ_pipe, pure_sine, sawtooth, square, triangle,
};

/// A single percussion sound in a drum kit, addressed by its MIDI note key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrumSound {
    /// MIDI note number that triggers the sound.
    pub key: u8,
    /// Canonical display name.
    pub name: String,
    /// Alternative labels that resolve to this sound.
    pub aliases: Vec<String>,
}

impl DrumSound {
    /// Builds a drum sound from a MIDI key, display name, and alias list.
    pub fn new<I, S>(key: u8, name: impl Into<String>, aliases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            key,
            name: name.into(),
            aliases: aliases.into_iter().map(Into::into).collect(),
        }
    }

    fn alias_keys(&self) -> impl Iterator<Item = String> + '_ {
        std::iter::once(normalize_label(&self.name))
            .chain(self.aliases.iter().map(|alias| normalize_label(alias)))
    }
}

/// A mapping from MIDI note keys and text labels to percussion sounds for a
/// named drum kit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrumKeyMap {
    /// Identifier of the drum kit.
    pub kit_name: String,
    sounds: BTreeMap<u8, DrumSound>,
    aliases: BTreeMap<String, u8>,
}

impl DrumKeyMap {
    /// Returns the General MIDI standard drum kit.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_sound_gm::DrumKeyMap;
    ///
    /// let kit = DrumKeyMap::gm();
    /// assert_eq!(kit.resolve("kick"), Some(36));
    /// ```
    pub fn gm() -> Self {
        Self::named_kit("gm-standard").expect("GM standard kit exists")
    }

    /// Returns the built-in kit with the given name, if recognized.
    pub fn named_kit(name: &str) -> Option<Self> {
        match normalize_label(name).as_str() {
            "gm" | "gmstandard" | "standard" => Some(Self::from_sounds(
                "gm-standard",
                GM_DRUM_SOUNDS.iter().map(|(key, name, aliases)| {
                    DrumSound::new(*key, *name, aliases.iter().copied())
                }),
            )),
            "fouronfloor" | "fourfloor" => Some(Self::from_sounds(
                "four-on-floor",
                [
                    DrumSound::new(36, "Kick", ["kick", "bd"]),
                    DrumSound::new(38, "Snare", ["snare", "sd"]),
                    DrumSound::new(42, "Closed Hat", ["closed-hat", "chh"]),
                    DrumSound::new(46, "Open Hat", ["open-hat", "ohh"]),
                    DrumSound::new(49, "Crash", ["crash"]),
                ],
            )),
            _ => None,
        }
    }

    /// Builds a custom kit named `kit_name` from the given sounds.
    pub fn custom<I>(kit_name: impl Into<String>, sounds: I) -> Self
    where
        I: IntoIterator<Item = DrumSound>,
    {
        Self::from_sounds(kit_name, sounds)
    }

    /// Inserts `sound`, registering its name, aliases, MIDI key, and
    /// `midi<key>` label as lookup keys.
    pub fn insert(&mut self, sound: DrumSound) {
        for alias in sound.alias_keys() {
            self.aliases.insert(alias, sound.key);
        }
        self.aliases.insert(sound.key.to_string(), sound.key);
        self.aliases.insert(format!("midi{}", sound.key), sound.key);
        self.sounds.insert(sound.key, sound);
    }

    /// Resolves a text `label` to a MIDI key, ignoring case and punctuation.
    pub fn resolve(&self, label: &str) -> Option<u8> {
        self.aliases.get(&normalize_label(label)).copied()
    }

    /// Resolves `label` to a MIDI key, returning `fallback` when unrecognized.
    pub fn remap(&self, label: &str, fallback: u8) -> u8 {
        self.resolve(label).unwrap_or(fallback)
    }

    /// Returns the sound mapped to `key`, if any.
    pub fn sound(&self, key: u8) -> Option<&DrumSound> {
        self.sounds.get(&key)
    }

    /// Returns the aliases registered for the sound at `key`.
    pub fn aliases_for(&self, key: u8) -> Vec<&str> {
        self.sound(key)
            .map(|sound| sound.aliases.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Returns an iterator over the kit's sounds in ascending key order.
    pub fn sounds(&self) -> impl Iterator<Item = &DrumSound> {
        self.sounds.values()
    }

    fn from_sounds<I>(kit_name: impl Into<String>, sounds: I) -> Self
    where
        I: IntoIterator<Item = DrumSound>,
    {
        let mut map = Self {
            kit_name: kit_name.into(),
            sounds: BTreeMap::new(),
            aliases: BTreeMap::new(),
        };
        for sound in sounds {
            map.insert(sound);
        }
        map
    }
}

/// Returns a [`TimbreBank`] assigning a timbre to each of the 128 General MIDI
/// melodic programs.
pub fn general_midi_bank() -> TimbreBank {
    let mut bank = TimbreBank::new(pure_sine());
    for program in 0..128 {
        let timbre = match program {
            0..=7 => pure_sine(),
            8..=15 => fm_pair(2.0, 0.8),
            16..=23 => organ_pipe(&[1.0, 2.0, 4.0]),
            24..=31 => karplus_strong(0.7),
            32..=39 => square(8),
            40..=47 => triangle(8),
            48..=55 => sawtooth(10),
            56..=63 => bell_inharmonic(&[1.0, 2.7, 5.8]),
            64..=71 => fm_pair(3.0, 1.2),
            72..=79 => organ_pipe(&[1.0, 3.0, 5.0]),
            80..=87 => karplus_strong(0.85),
            88..=95 => bell_inharmonic(&[1.0, 1.5, 2.3, 3.9]),
            96..=103 => square(5),
            104..=111 => triangle(5),
            112..=119 => sawtooth(6),
            _ => fm_pair(1.5, 2.0),
        };
        bank.insert(0, 0, program, timbre);
    }
    bank
}

fn normalize_label(label: &str) -> String {
    label
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

const GM_DRUM_SOUNDS: &[(u8, &str, &[&str])] = &[
    (35, "Acoustic Bass Drum", &["kick-low", "acoustic-kick"]),
    (36, "Bass Drum 1", &["kick", "bass-drum", "bd"]),
    (37, "Side Stick", &["sidestick", "rim"]),
    (38, "Acoustic Snare", &["snare", "snare-acoustic", "sd"]),
    (39, "Hand Clap", &["clap"]),
    (40, "Electric Snare", &["snare-electric", "esnare"]),
    (41, "Low Floor Tom", &["floor-tom-low"]),
    (42, "Closed Hi-Hat", &["closed-hat", "chh", "hat-closed"]),
    (43, "High Floor Tom", &["floor-tom-high"]),
    (44, "Pedal Hi-Hat", &["pedal-hat", "phh"]),
    (45, "Low Tom", &["tom-low"]),
    (46, "Open Hi-Hat", &["open-hat", "ohh", "hat-open"]),
    (47, "Low-Mid Tom", &["tom-low-mid"]),
    (48, "High-Mid Tom", &["tom-high-mid"]),
    (49, "Crash Cymbal 1", &["crash", "crash-1"]),
    (50, "High Tom", &["tom-high"]),
    (51, "Ride Cymbal 1", &["ride", "ride-1"]),
    (52, "Chinese Cymbal", &["china"]),
    (53, "Ride Bell", &["bell", "ride-bell"]),
    (54, "Tambourine", &["tamb"]),
    (55, "Splash Cymbal", &["splash"]),
    (56, "Cowbell", &["cowbell"]),
    (57, "Crash Cymbal 2", &["crash-2"]),
    (58, "Vibraslap", &["vibraslap"]),
    (59, "Ride Cymbal 2", &["ride-2"]),
    (60, "High Bongo", &["bongo-high"]),
    (61, "Low Bongo", &["bongo-low"]),
    (62, "Mute High Conga", &["conga-mute-high"]),
    (63, "Open High Conga", &["conga-open-high"]),
    (64, "Low Conga", &["conga-low"]),
    (65, "High Timbale", &["timbale-high"]),
    (66, "Low Timbale", &["timbale-low"]),
    (67, "High Agogo", &["agogo-high"]),
    (68, "Low Agogo", &["agogo-low"]),
    (69, "Cabasa", &["cabasa"]),
    (70, "Maracas", &["maracas"]),
    (71, "Short Whistle", &["whistle-short"]),
    (72, "Long Whistle", &["whistle-long"]),
    (73, "Short Guiro", &["guiro-short"]),
    (74, "Long Guiro", &["guiro-long"]),
    (75, "Claves", &["claves"]),
    (76, "High Wood Block", &["woodblock-high"]),
    (77, "Low Wood Block", &["woodblock-low"]),
    (78, "Mute Cuica", &["cuica-mute"]),
    (79, "Open Cuica", &["cuica-open"]),
    (80, "Mute Triangle", &["triangle-mute"]),
    (81, "Open Triangle", &["triangle-open"]),
];
