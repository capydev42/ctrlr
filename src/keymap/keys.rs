//! Parsing and printing key strings — the vocabulary of `config.toml` and the
//! text the help popup shows.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// One key press. Stored normalized: see [`KeyChord::matches`] for why SHIFT is
/// treated differently for characters than for named keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

/// One or two chords. Depth is capped at two because that is what the pending
/// key buffer can represent, and `gg` is the only sequence anyone has asked for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Binding {
    Single(KeyChord),
    Chord(KeyChord, KeyChord),
}

impl KeyChord {
    pub fn new(code: KeyCode, mods: KeyModifiers) -> Self {
        Self {
            code,
            mods: normalize(code, mods),
        }
    }

    /// CONTROL and ALT must match exactly. SHIFT is ignored for `Char` codes —
    /// the character already carries the case, and terminals disagree about
    /// whether to report the modifier alongside it — but compared for named
    /// keys, where `Shift+Tab` is a different key from `Tab`.
    pub fn matches(&self, key: &KeyEvent) -> bool {
        self.code == key.code && self.mods == normalize(key.code, key.modifiers)
    }
}

fn normalize(code: KeyCode, mods: KeyModifiers) -> KeyModifiers {
    if matches!(code, KeyCode::Char(_)) {
        mods & !KeyModifiers::SHIFT
    } else {
        mods
    }
}

#[derive(Debug, PartialEq)]
pub struct KeyParseError(pub String);

impl std::fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whitespace-separated chords, e.g. `"g g"`.
pub fn parse_binding(s: &str) -> Result<Binding, KeyParseError> {
    let mut chords = s.split_whitespace();
    let Some(first) = chords.next() else {
        return Err(KeyParseError("empty key".into()));
    };
    let first = parse_chord(first)?;
    let Some(second) = chords.next() else {
        return Ok(Binding::Single(first));
    };
    let second = parse_chord(second)?;
    if chords.next().is_some() {
        return Err(KeyParseError(format!(
            "`{}` is longer than two keys; ctrlr only tracks one pending key",
            s
        )));
    }
    Ok(Binding::Chord(first, second))
}

pub fn parse_chord(s: &str) -> Result<KeyChord, KeyParseError> {
    if s.is_empty() {
        return Err(KeyParseError("empty key".into()));
    }
    // Split on `+`, but a trailing empty segment is the literal `+` itself, as
    // in "ctrl++".
    let mut parts: Vec<&str> = s.split('+').collect();
    if parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
        if let Some(last) = parts.last_mut() {
            if last.is_empty() {
                *last = "+";
            } else {
                parts.push("+");
            }
        }
    }

    let (name, mod_parts) = parts.split_last().expect("split always yields one part");
    let mut mods = KeyModifiers::NONE;
    for part in mod_parts {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= KeyModifiers::CONTROL,
            "alt" | "meta" => mods |= KeyModifiers::ALT,
            "shift" => mods |= KeyModifiers::SHIFT,
            other => {
                return Err(KeyParseError(format!(
                    "unknown modifier `{}` in `{}`; ctrlr understands ctrl, alt and shift",
                    other, s
                )));
            }
        }
    }

    let code = parse_code(name, s)?;
    Ok(KeyChord::new(code, mods))
}

fn parse_code(name: &str, whole: &str) -> Result<KeyCode, KeyParseError> {
    // A single character is always itself, case-sensitively: `G` is not `g`.
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Ok(KeyCode::Char(c));
    }
    let code = match name.to_ascii_lowercase().as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "space" => KeyCode::Char(' '),
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        other => {
            if let Some(n) = other.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
                && (1..=12).contains(&n)
            {
                return Ok(KeyCode::F(n));
            }
            return Err(KeyParseError(format!(
                "unknown key `{}` in `{}`",
                other, whole
            )));
        }
    };
    Ok(code)
}

pub fn format_binding(binding: &Binding) -> String {
    match binding {
        Binding::Single(c) => format_chord(c),
        Binding::Chord(a, b) => format!("{} {}", format_chord(a), format_chord(b)),
    }
}

pub fn format_chord(chord: &KeyChord) -> String {
    let mut out = String::new();
    if chord.mods.contains(KeyModifiers::CONTROL) {
        out.push_str("Ctrl+");
    }
    if chord.mods.contains(KeyModifiers::ALT) {
        out.push_str("Alt+");
    }
    if chord.mods.contains(KeyModifiers::SHIFT) {
        out.push_str("Shift+");
    }
    out.push_str(&format_code(chord.code));
    out
}

fn format_code(code: KeyCode) -> String {
    match code {
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::BackTab => "BackTab".into(),
        KeyCode::Backspace => "Backspace".into(),
        KeyCode::Delete => "Delete".into(),
        KeyCode::Insert => "Insert".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
        KeyCode::PageUp => "PageUp".into(),
        KeyCode::PageDown => "PageDown".into(),
        KeyCode::Up => "Up".into(),
        KeyCode::Down => "Down".into(),
        KeyCode::Left => "Left".into(),
        KeyCode::Right => "Right".into(),
        KeyCode::F(n) => format!("F{}", n),
        other => format!("{:?}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(s: &str) -> KeyChord {
        parse_chord(s).unwrap()
    }

    #[test]
    fn test_parse_chord_plain_characters() {
        assert_eq!(
            chord("j"),
            KeyChord::new(KeyCode::Char('j'), KeyModifiers::NONE)
        );
        assert_eq!(
            chord("?"),
            KeyChord::new(KeyCode::Char('?'), KeyModifiers::NONE)
        );
        assert_eq!(
            chord("<"),
            KeyChord::new(KeyCode::Char('<'), KeyModifiers::NONE)
        );
        assert_eq!(
            chord("."),
            KeyChord::new(KeyCode::Char('.'), KeyModifiers::NONE)
        );
        assert_eq!(
            chord("1"),
            KeyChord::new(KeyCode::Char('1'), KeyModifiers::NONE)
        );
    }

    /// Characters are case-sensitive, so `G` and `g` can mean different things.
    #[test]
    fn test_parse_chord_is_case_sensitive_for_characters() {
        assert_ne!(chord("g"), chord("G"));
    }

    #[test]
    fn test_parse_chord_modifiers() {
        assert_eq!(
            chord("ctrl+d"),
            KeyChord::new(KeyCode::Char('d'), KeyModifiers::CONTROL)
        );
        assert_eq!(
            chord("Alt+<"),
            KeyChord::new(KeyCode::Char('<'), KeyModifiers::ALT)
        );
        assert_eq!(
            chord("CTRL+ALT+x"),
            KeyChord::new(
                KeyCode::Char('x'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )
        );
    }

    #[test]
    fn test_parse_chord_named_keys() {
        assert_eq!(chord("pagedown").code, KeyCode::PageDown);
        assert_eq!(chord("PgUp").code, KeyCode::PageUp);
        assert_eq!(chord("f1").code, KeyCode::F(1));
        assert_eq!(chord("F12").code, KeyCode::F(12));
        assert_eq!(chord("esc").code, KeyCode::Esc);
        assert_eq!(chord("space").code, KeyCode::Char(' '));
    }

    /// `+` is both the separator and a legitimate key.
    #[test]
    fn test_parse_chord_handles_a_literal_plus() {
        assert_eq!(chord("+").code, KeyCode::Char('+'));
        assert_eq!(
            chord("ctrl++"),
            KeyChord::new(KeyCode::Char('+'), KeyModifiers::CONTROL)
        );
    }

    #[test]
    fn test_parse_chord_rejects_nonsense() {
        assert!(parse_chord("").is_err());
        assert!(parse_chord("hyper+x").is_err());
        assert!(parse_chord("f13").is_err());
        assert!(parse_chord("pagesideways").is_err());
    }

    #[test]
    fn test_parse_binding_sequences() {
        assert_eq!(
            parse_binding("g g").unwrap(),
            Binding::Chord(chord("g"), chord("g"))
        );
        assert_eq!(parse_binding("G").unwrap(), Binding::Single(chord("G")));
        assert!(parse_binding("a b c").is_err());
        assert!(parse_binding("   ").is_err());
    }

    #[test]
    fn test_format_round_trips_through_parse() {
        for s in [
            "j", "G", "?", "<", "ctrl+d", "alt+<", "f1", "pagedown", "esc", "g g", "ctrl++",
        ] {
            let parsed = parse_binding(s).unwrap();
            let printed = format_binding(&parsed);
            assert_eq!(
                parse_binding(&printed).unwrap(),
                parsed,
                "{} printed as {} and did not survive",
                s,
                printed
            );
        }
    }

    /// Terminals disagree about reporting SHIFT alongside an already-uppercase
    /// character, so a binding must match either way. Named keys keep it:
    /// Shift+Tab is its own key.
    #[test]
    fn test_shift_is_ignored_for_characters_but_kept_for_named_keys() {
        let g = chord("G");
        assert!(g.matches(&KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE)));
        assert!(g.matches(&KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT)));
        assert!(!g.matches(&KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)));

        let tab = chord("shift+tab");
        assert!(tab.matches(&KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT)));
        assert!(!tab.matches(&KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)));
    }

    #[test]
    fn test_modifiers_must_match_exactly() {
        let c = chord("ctrl+d");
        assert!(c.matches(&KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)));
        assert!(!c.matches(&KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)));
        assert!(!c.matches(&KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL | KeyModifiers::ALT
        )));
    }
}
