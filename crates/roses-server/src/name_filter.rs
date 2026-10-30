//! Names refused for new accounts beyond the protocol's own rules
//! (`roses_net::lobby::check_new_username`: length, letters and digits,
//! the reserved staff words): offensive words and names that pretend to be
//! the project or its staff.
//!
//! The built-in list is `name-filter.txt` beside this crate's Cargo.toml;
//! a server operator adds their own with `roses-server hub --name-filter
//! <file>` (same format). One entry per line, `#` starts a comment:
//!
//! - `word`: refused anywhere in a name (`xWORDx`, `Word2`).
//! - `=word`: refused only as the whole name, ignoring trailing digits
//!   (for short words that are parts of ordinary names, e.g. `=ass` keeps
//!   `Cassandra` usable).
//!
//! Matching ignores case and undoes common digit look-alikes (`0` as `o`,
//! `1` as `i`, `3` as `e`, `4` as `a`, `5` as `s`, `7` as `t`, `8` as `b`,
//! `9` as `g`), so `F4ke` matches the entry `fake`. Only new names are
//! checked: existing accounts keep theirs (an operator renames or disables
//! them by hand).

use std::path::Path;

/// The built-in list (compiled in).
pub const BUILTIN: &str = include_str!("../name-filter.txt");

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameFilter {
    /// Refused anywhere in the name.
    contains: Vec<String>,
    /// Refused as the whole name (trailing digits ignored).
    exact: Vec<String>,
}

/// The lower-case name and its digit look-alikes undone.
fn forms(name: &str) -> [String; 2] {
    let lower = name.to_ascii_lowercase();
    let plain = lower
        .chars()
        .map(|c| match c {
            '0' => 'o',
            '1' => 'i',
            '3' => 'e',
            '4' => 'a',
            '5' => 's',
            '7' => 't',
            '8' => 'b',
            '9' => 'g',
            c => c,
        })
        .collect();
    [lower, plain]
}

impl NameFilter {
    /// The built-in list only.
    pub fn builtin() -> Self {
        Self::parse(BUILTIN).expect("the built-in name filter parses")
    }

    /// Entries in the file format above.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut filter = Self::default();
        filter.add(text)?;
        Ok(filter)
    }

    /// Add the entries of `text` (file format above).
    pub fn add(&mut self, text: &str) -> Result<(), String> {
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (exact, word) = match line.strip_prefix('=') {
                Some(w) => (true, w.trim()),
                None => (false, line),
            };
            if word.is_empty() || !word.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err(format!("name filter line {}: {line:?} is not letters and digits", n + 1));
            }
            let word = word.to_ascii_lowercase();
            let list = if exact { &mut self.exact } else { &mut self.contains };
            if !list.contains(&word) {
                list.push(word);
            }
        }
        Ok(())
    }

    /// The built-in list plus the entries of the file at `path`.
    pub fn builtin_and_file(path: &Path) -> Result<Self, String> {
        let mut filter = Self::builtin();
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        filter.add(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(filter)
    }

    /// Entries in the filter.
    pub fn len(&self) -> usize {
        self.contains.len() + self.exact.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether a new account may use `name`.
    pub fn allows(&self, name: &str) -> bool {
        forms(name).iter().all(|form| {
            let stem = form.trim_end_matches(|c: char| c.is_ascii_digit());
            !self.contains.iter().any(|w| form.contains(w.as_str()))
                && !self.exact.iter().any(|w| w == form || w == stem)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_match_anywhere_or_whole_and_ignore_look_alikes() {
        let f = NameFilter::parse("# comment\nbadword\n=ass  # whole name only\n\n").unwrap();
        assert_eq!(f.len(), 2);
        for refused in ["badword", "xBADWORDx", "B4dw0rd", "Badword7", "ass", "ASS", "Ass12"] {
            assert!(!f.allows(refused), "{refused}");
        }
        for allowed in ["Cassandra", "Bass", "Yugi", "Joey", "Badwor", "Assassin"] {
            assert!(f.allows(allowed), "{allowed}");
        }
        assert!(NameFilter::parse("two words").is_err());
        assert!(NameFilter::parse("=").is_err());
    }

    #[test]
    fn the_builtin_list_parses_and_keeps_ordinary_names() {
        let f = NameFilter::builtin();
        assert!(f.len() > 20);
        for allowed in [
            "Yugi", "Joey", "Tea", "Tristan", "Bakura", "Marik", "Mai", "Weevil", "Pegasus", "Cassandra", "Essex",
            "Sussex", "Hancock", "Peacock", "Dickens", "Spice", "Bass", "Analyst", "Grape", "Therapist", "Cockburn",
            "Hello", "Classic", "Passion", "Kaiba", "Ishizu",
        ] {
            assert!(f.allows(allowed), "{allowed}");
        }
        assert!(!f.allows("KonamiStaff"));
        assert!(!f.allows("RosesDev"));
        assert!(!f.allows("Hitler1"));
        assert!(!f.allows("N4zi"));
        assert!(!f.allows("xFuckx"));
        assert!(!f.allows("Sh1t"));
    }
}
