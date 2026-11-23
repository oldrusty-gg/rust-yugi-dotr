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

/// What a name looks like, for telling lookalikes apart: lower case, with
/// `rn` read as `m`, `vv` as `w`, `I` / `i` / `1` as `l`, `q` as `g` and
/// the digits that pass for letters as those letters (`0` o, `2` z, `3` e,
/// `4` a, `5` s, `6` and `8` b, `7` t, `9` g). Two names with one skeleton
/// read the same at a glance (`Yugi`, `Yuqi`, `YUG1`; `Il1ya`, `llla`).
pub fn skeleton(name: &str) -> String {
    let lower = name.to_ascii_lowercase().replace("rn", "m").replace("vv", "w");
    lower
        .chars()
        .map(|c| match c {
            '0' => 'o',
            '1' | 'i' => 'l',
            '2' => 'z',
            '3' => 'e',
            '4' => 'a',
            '5' => 's',
            '6' | '8' => 'b',
            '7' => 't',
            '9' | 'q' => 'g',
            c => c,
        })
        .collect()
}

/// Whether a new name looks like one of the protocol's reserved names
/// (`roses_net::lobby::RESERVED_NAMES` and the staff words, which
/// `check_new_username` compares letter for letter): `C0mputer`, `Adm1n`,
/// `M0derator7`. The name itself has passed `check_new_username`.
pub fn looks_reserved(name: &str) -> bool {
    const STAFF: [&str; 12] =
        ["admin", "administrator", "moderator", "mod", "server", "system", "root", "owner", "staff", "support", "official", "computer"];
    let whole = skeleton(name);
    let stem = skeleton(name.trim_end_matches(|c: char| c.is_ascii_digit()));
    roses_net::lobby::RESERVED_NAMES.iter().any(|r| skeleton(r) == whole)
        || STAFF.iter().any(|s| skeleton(s) == stem)
        || ["admin", "moderator"].iter().any(|p| whole.starts_with(&skeleton(p)))
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
    fn lookalikes_share_a_skeleton_and_reserved_lookalikes_are_caught() {
        for (a, b) in [("Yugi", "Yuqi"), ("Yugi", "YUG1"), ("Kaiba", "Ka1ba"), ("Bakura", "8akura"), ("Mai", "Mal"), ("Marik", "Mar1k"), ("Weevil", "VVeevil"), ("Mirror", "Mlrror"), ("Arnold", "Amold")] {
            assert_eq!(skeleton(a), skeleton(b), "{a} / {b}");
        }
        for (a, b) in [("Yugi", "Yugi2"), ("Joey", "Joel"), ("Tea", "Ted"), ("Clark", "Dark"), ("Mai", "May")] {
            assert_ne!(skeleton(a), skeleton(b), "{a} / {b}");
        }
        for reserved in ["C0mputer", "Adm1n", "ADMlN", "Adm1nistrator", "M0derator7", "M0d", "R00t", "0fficial", "K0nami", "Rose5", "Sy5tem12", "Guest", "Gue5t"] {
            assert!(looks_reserved(reserved), "{reserved}");
        }
        for ordinary in ["Yugi", "Admiral", "Modest", "Rooted", "Kaiba", "Joey", "Serverus", "Hostess", "Players"] {
            assert!(!looks_reserved(ordinary), "{ordinary}");
        }
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
