use num_rational::Ratio;
use sim_kernel::Span;
use sim_lib_music_core::{
    Articulation, Channel, Chord, Melody, MelodyItem, Music, Note, Progression, Rest, Score,
};

use crate::{
    model::{NotationError, NotationReport, error_at},
    spell::{decode_lily_pitch, key_from_lily},
};

/// Parses LilyPond-subset text into a score, returning it with diagnostics.
pub fn import_lilypond_report(source: &str) -> Result<NotationReport<Score>, NotationError> {
    let tokens = lex(source)?;
    let mut parser = Parser {
        tokens: &tokens,
        index: 0,
    };
    let score = parser.parse_score()?;
    Ok(NotationReport {
        value: score,
        diagnostics: Vec::new(),
    })
}

/// Parses LilyPond-subset text into a score, discarding diagnostics.
pub fn import_lilypond(source: &str) -> Result<Score, NotationError> {
    Ok(import_lilypond_report(source)?.value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Item {
    Melody(Melody),
    Progression(Progression),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TokenKind {
    Command(String),
    Word(String),
    String(String),
    LBrace,
    RBrace,
    DoubleLt,
    DoubleGt,
    Lt,
    Gt,
    Equal,
    Tilde,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    span: Span,
}

struct Parser<'a> {
    tokens: &'a [Token],
    index: usize,
}

impl Parser<'_> {
    fn parse_score(&mut self) -> Result<Score, NotationError> {
        self.expect_command("score")?;
        self.expect(TokenKind::LBrace)?;
        let mut tempo = 120u32;
        let mut time_signature = (4u8, 4u8);
        let mut key = None;
        let mut body = None;
        while !self.check(&TokenKind::RBrace) {
            if self.match_command("tempo") {
                let beat = self.expect_word()?;
                if beat != "4" {
                    return Err(self.error_here("only quarter-note tempo is supported"));
                }
                self.expect(TokenKind::Equal)?;
                tempo = self
                    .expect_word()?
                    .parse()
                    .map_err(|_| self.error_here("invalid tempo"))?;
            } else if self.match_command("key") {
                let tonic = self.expect_word()?;
                let mode = self.expect_command_any(&["major", "minor"])?;
                key = key_from_lily(&tonic, &mode);
            } else if self.match_command("time") {
                let value = self.expect_word()?;
                let (num, den) = value
                    .split_once('/')
                    .ok_or_else(|| self.error_here("expected time signature of form 4/4"))?;
                time_signature = (
                    num.parse()
                        .map_err(|_| self.error_here("invalid numerator"))?,
                    den.parse()
                        .map_err(|_| self.error_here("invalid denominator"))?,
                );
            } else if self.check(&TokenKind::DoubleLt) {
                body = Some(Music::Counterpoint(self.parse_counterpoint()?));
            } else if self.check(&TokenKind::LBrace) {
                body = Some(match self.parse_sequence(key.clone())? {
                    Item::Melody(melody) => Music::Melody(melody),
                    Item::Progression(progression) => Music::Progression(progression),
                });
            } else {
                return Err(self.error_here("unsupported lilypond syntax"));
            }
        }
        self.expect(TokenKind::RBrace)?;
        Score::new(
            tempo,
            time_signature,
            key,
            body.ok_or_else(|| self.error_here("missing music body"))?,
        )
        .map_err(NotationError::from)
    }

    fn parse_counterpoint(&mut self) -> Result<sim_lib_music_core::Counterpoint, NotationError> {
        self.expect(TokenKind::DoubleLt)?;
        let mut voices = Vec::new();
        let mut names = Vec::new();
        while !self.check(&TokenKind::DoubleGt) {
            self.expect_command("new")?;
            if self.expect_word()? != "Voice" {
                return Err(self.error_here("only \\new Voice is supported"));
            }
            self.expect(TokenKind::Equal)?;
            let name = match self.bump()?.kind.clone() {
                TokenKind::String(value) => value,
                _ => return Err(self.error_here("expected quoted voice name")),
            };
            let melody = match self.parse_sequence(None)? {
                Item::Melody(melody) => melody,
                Item::Progression(_) => return Err(self.error_here("voice body must be melodic")),
            };
            names.push(name);
            voices.push(melody);
        }
        self.expect(TokenKind::DoubleGt)?;
        sim_lib_music_core::Counterpoint::new(voices, names).map_err(NotationError::from)
    }

    fn parse_sequence(&mut self, key: Option<String>) -> Result<Item, NotationError> {
        self.expect(TokenKind::LBrace)?;
        let mut melody_items = Vec::new();
        let mut chords = Vec::new();
        let mut saw_chords = false;
        let mut saw_melody = false;
        while !self.check(&TokenKind::RBrace) {
            if self.check(&TokenKind::Lt) {
                saw_chords = true;
                if saw_melody {
                    return Err(self.error_here("mixed chord and melodic bodies are unsupported"));
                }
                chords.push(self.parse_chord()?);
            } else {
                saw_melody = true;
                if saw_chords {
                    return Err(self.error_here("mixed chord and melodic bodies are unsupported"));
                }
                melody_items.push(self.parse_melody_item()?);
            }
        }
        self.expect(TokenKind::RBrace)?;
        if saw_chords {
            Ok(Item::Progression(
                Progression::new(key, chords).map_err(NotationError::from)?,
            ))
        } else {
            Ok(Item::Melody(
                Melody::new(merge_rests(melody_items)).map_err(NotationError::from)?,
            ))
        }
    }

    fn parse_melody_item(&mut self) -> Result<MelodyItem, NotationError> {
        let token = self.expect_word()?;
        if let Some(duration_text) = token.strip_prefix('r') {
            return Ok(MelodyItem::Rest(Rest::new(parse_duration(duration_text)?)?));
        }
        let (pitch_text, duration_text) = split_note_duration(&token)
            .ok_or_else(|| self.error_here("expected note token with duration"))?;
        let spelled = decode_lily_pitch(pitch_text)
            .ok_or_else(|| self.error_here("invalid lilypond pitch spelling"))?;
        let mut duration = parse_duration(duration_text)?;
        while self.match_token(&TokenKind::Tilde) {
            let next = self.expect_word()?;
            let (next_pitch, next_duration) = split_note_duration(&next)
                .ok_or_else(|| self.error_here("expected tied note token with duration"))?;
            if next_pitch != pitch_text {
                return Err(self.error_here("tied notes must preserve pitch"));
            }
            duration += parse_duration(next_duration)?;
        }
        Ok(MelodyItem::Note(Note::new(
            duration,
            spelled.to_pitch(),
            100,
            Channel::new(0).expect("channel 0 is valid"),
            Articulation::Normal,
        )?))
    }

    fn parse_chord(&mut self) -> Result<Chord, NotationError> {
        self.expect(TokenKind::Lt)?;
        let mut pitches = Vec::new();
        while !self.check(&TokenKind::Gt) {
            let token = self.expect_word()?;
            let spelled = decode_lily_pitch(&token)
                .ok_or_else(|| self.error_here("invalid chord pitch spelling"))?;
            pitches.push(spelled.to_pitch());
        }
        self.expect(TokenKind::Gt)?;
        let mut duration = parse_duration(&self.expect_word()?)?;
        while self.match_token(&TokenKind::Tilde) {
            self.expect(TokenKind::Lt)?;
            let mut next = Vec::new();
            while !self.check(&TokenKind::Gt) {
                let token = self.expect_word()?;
                let spelled = decode_lily_pitch(&token)
                    .ok_or_else(|| self.error_here("invalid chord pitch spelling"))?;
                next.push(spelled.to_pitch());
            }
            self.expect(TokenKind::Gt)?;
            if next != pitches {
                return Err(self.error_here("tied chords must preserve pitch content"));
            }
            duration += parse_duration(&self.expect_word()?)?;
        }
        Chord::new(
            duration,
            "ly",
            pitches,
            100,
            Channel::new(0).expect("channel 0 is valid"),
        )
        .map_err(NotationError::from)
    }

    fn expect_command(&mut self, value: &str) -> Result<(), NotationError> {
        match self.bump()?.kind.clone() {
            TokenKind::Command(found) if found == value => Ok(()),
            _ => Err(self.error_here(format!("expected \\{value}"))),
        }
    }

    fn expect_command_any(&mut self, values: &[&str]) -> Result<String, NotationError> {
        match self.bump()?.kind.clone() {
            TokenKind::Command(found) if values.iter().any(|value| *value == found) => Ok(found),
            _ => Err(self.error_here("unexpected command")),
        }
    }

    fn expect_word(&mut self) -> Result<String, NotationError> {
        match self.bump()?.kind.clone() {
            TokenKind::Word(value) => Ok(value),
            _ => Err(self.error_here("expected word token")),
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<(), NotationError> {
        if self.match_token(&kind) {
            Ok(())
        } else {
            Err(self.error_here("unexpected token"))
        }
    }

    fn match_command(&mut self, value: &str) -> bool {
        matches!(self.peek(), Some(Token { kind: TokenKind::Command(found), .. }) if found == value)
            && {
                self.index += 1;
                true
            }
    }

    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek().is_some_and(|token| &token.kind == kind)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn bump(&mut self) -> Result<&Token, NotationError> {
        let token = self
            .tokens
            .get(self.index)
            .ok_or_else(|| self.error_here("unexpected end of input"))?;
        self.index += 1;
        Ok(token)
    }

    fn error_here(&self, message: impl Into<String>) -> NotationError {
        let span = self
            .peek()
            .map(|token| token.span.clone())
            .unwrap_or(Span { start: 0, end: 0 });
        error_at(message, span)
    }
}

fn parse_duration(value: &str) -> Result<Ratio<i64>, NotationError> {
    let denom = value
        .parse::<i64>()
        .map_err(|_| error_at("invalid duration", Span { start: 0, end: 0 }))?;
    if denom <= 0 {
        return Err(error_at(
            "duration must be positive",
            Span { start: 0, end: 0 },
        ));
    }
    Ok(Ratio::new(1, denom))
}

fn split_note_duration(token: &str) -> Option<(&str, &str)> {
    let index = token.find(|ch: char| ch.is_ascii_digit())?;
    Some(token.split_at(index))
}

fn merge_rests(items: Vec<MelodyItem>) -> Vec<MelodyItem> {
    let mut merged = Vec::with_capacity(items.len());
    for item in items {
        match item {
            MelodyItem::Rest(rest) => match merged.last_mut() {
                Some(MelodyItem::Rest(existing)) => {
                    existing.duration += rest.duration;
                }
                _ => merged.push(MelodyItem::Rest(rest)),
            },
            MelodyItem::Note(note) => merged.push(MelodyItem::Note(note)),
        }
    }
    merged
}

fn lex(source: &str) -> Result<Vec<Token>, NotationError> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] as char {
            ch if ch.is_ascii_whitespace() => index += 1,
            '%' => {
                while index < bytes.len() && bytes[index] as char != '\n' {
                    index += 1;
                }
            }
            '{' => {
                tokens.push(token(TokenKind::LBrace, index, index + 1));
                index += 1;
            }
            '}' => {
                tokens.push(token(TokenKind::RBrace, index, index + 1));
                index += 1;
            }
            '=' => {
                tokens.push(token(TokenKind::Equal, index, index + 1));
                index += 1;
            }
            '~' => {
                tokens.push(token(TokenKind::Tilde, index, index + 1));
                index += 1;
            }
            '<' if bytes
                .get(index + 1)
                .is_some_and(|next| *next as char == '<') =>
            {
                tokens.push(token(TokenKind::DoubleLt, index, index + 2));
                index += 2;
            }
            '>' if bytes
                .get(index + 1)
                .is_some_and(|next| *next as char == '>') =>
            {
                tokens.push(token(TokenKind::DoubleGt, index, index + 2));
                index += 2;
            }
            '<' => {
                tokens.push(token(TokenKind::Lt, index, index + 1));
                index += 1;
            }
            '>' => {
                tokens.push(token(TokenKind::Gt, index, index + 1));
                index += 1;
            }
            '"' => {
                let start = index;
                index += 1;
                let mut value = String::new();
                while index < bytes.len() && bytes[index] as char != '"' {
                    value.push(bytes[index] as char);
                    index += 1;
                }
                if index >= bytes.len() {
                    return Err(error_at("unterminated string", Span { start, end: index }));
                }
                index += 1;
                tokens.push(token(TokenKind::String(value), start, index));
            }
            '\\' => {
                let start = index;
                index += 1;
                while index < bytes.len() && (bytes[index] as char).is_ascii_alphabetic() {
                    index += 1;
                }
                tokens.push(token(
                    TokenKind::Command(source[start + 1..index].to_owned()),
                    start,
                    index,
                ));
            }
            _ => {
                let start = index;
                while index < bytes.len()
                    && !matches!(
                        bytes[index] as char,
                        ' ' | '\n' | '\t' | '\r' | '{' | '}' | '<' | '>' | '=' | '~' | '"'
                    )
                {
                    index += 1;
                }
                tokens.push(token(
                    TokenKind::Word(source[start..index].to_owned()),
                    start,
                    index,
                ));
            }
        }
    }
    Ok(tokens)
}

fn token(kind: TokenKind, start: usize, end: usize) -> Token {
    Token {
        kind,
        span: Span { start, end },
    }
}
