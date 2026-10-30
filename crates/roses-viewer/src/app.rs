use roses_viewer::{from_saved_capture, from_snapshot_json, read_bounded, BoardSnapshot};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum Input {
    Demo,
    Capture(PathBuf),
    Snapshot(PathBuf),
}

pub struct Options {
    pub input: Input,
    pub library: Option<PathBuf>,
    pub check: bool,
    pub help: bool,
}
impl Options {
    pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut input = None;
        let mut library = None;
        let mut check = false;
        let mut help = false;
        let mut arguments = arguments.into_iter();
        while let Some(option) = arguments.next() {
            match option.to_str() {
                Some("--demo") if input.is_none() => input = Some(Input::Demo),
                Some("--capture" | "--snapshot") if input.is_none() => {
                    let path = next_path(&mut arguments, &option)?;
                    input = Some(if option == "--capture" {
                        Input::Capture(path)
                    } else {
                        Input::Snapshot(path)
                    });
                }
                Some("--library") if library.is_none() => {
                    library = Some(next_path(&mut arguments, &option)?)
                }
                Some("--check") if !check => check = true,
                Some("--help") if !help => help = true,
                _ => {
                    return Err(format!(
                        "Unknown or duplicate option: {}. Use --help for usage.",
                        option.to_string_lossy()
                    ))
                }
            }
        }
        Ok(Self {
            input: input.unwrap_or(Input::Demo),
            library,
            check,
            help,
        })
    }
}
fn next_path(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &OsString,
) -> Result<PathBuf, String> {
    let value = arguments
        .next()
        .filter(|argument| !argument.is_empty() && !argument.to_string_lossy().starts_with("--"))
        .ok_or_else(|| format!("Missing path after {}.", option.to_string_lossy()))?;
    Ok(PathBuf::from(value))
}

pub struct Loaded {
    pub snapshot: BoardSnapshot,
    pub source_label: String,
    pub library: Option<roses_assets::library::Library>,
    pub warnings: Vec<String>,
}
impl Loaded {
    pub fn open(input: Input, library_path: Option<&Path>) -> Result<Self, String> {
        let (snapshot, source_label) = load_input(input)?;
        let mut loaded = Self {
            snapshot,
            source_label,
            library: None,
            warnings: Vec::new(),
        };
        if let Some(path) = library_path {
            loaded.load_library(path);
        }
        Ok(loaded)
    }
    pub fn replace_input(&mut self, input: Input) -> Result<(), String> {
        let (snapshot, source_label) = load_input(input)?;
        let snapshot = if let Some(library) = &self.library {
            if snapshot.is_synthetic() {
                snapshot
            } else {
                snapshot
                    .with_library(library)
                    .map_err(|error| error.to_string())?
            }
        } else {
            snapshot
        };
        self.snapshot = snapshot;
        self.source_label = source_label;
        Ok(())
    }
    pub fn load_library(&mut self, path: &Path) {
        match roses_assets::library::load_library(path) {
            Ok(library) => {
                if !self.snapshot.is_synthetic() {
                    match self.snapshot.clone().with_library(&library) {
                        Ok(snapshot) => self.snapshot = snapshot,
                        Err(error) => { self.warnings = vec![format!("Library metadata unavailable: {error}")]; return; }
                    }
                }
                self.library = Some(library);
                self.warnings.clear();
            }
            Err(error) => self.warnings = vec![format!("Optional library could not be loaded; existing labels or raw card IDs remain available. {error}")],
        }
    }
}
fn load_input(input: Input) -> Result<(BoardSnapshot, String), String> {
    match input {
        Input::Demo => Ok((
            roses_viewer::demo(),
            "Built-in synthetic fixture — no original assets".into(),
        )),
        Input::Capture(path) => {
            from_saved_capture(&read_bounded(&path).map_err(|error| error.to_string())?)
                .map(|snapshot| (snapshot, format!("Saved capture: {}", path.display())))
                .map_err(|error| error.to_string())
        }
        Input::Snapshot(path) => {
            from_snapshot_json(&read_bounded(&path).map_err(|error| error.to_string())?)
                .map(|snapshot| (snapshot, format!("Explicit snapshot: {}", path.display())))
                .map_err(|error| error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_ambiguous_input_and_missing_paths() {
        for arguments in [
            vec!["--capture"],
            vec!["--library", "--check"],
            vec!["--demo", "--capture", "x.json"],
            vec!["--check", "--check"],
        ] {
            assert!(Options::parse(arguments.into_iter().map(OsString::from)).is_err());
        }
    }
    #[test]
    fn missing_optional_library_does_not_destroy_demo() {
        let path =
            std::env::temp_dir().join(format!("absent-roses-library-{}", std::process::id()));
        let loaded = Loaded::open(Input::Demo, Some(&path)).unwrap();
        assert_eq!(loaded.snapshot.document().units.len(), 3);
        assert!(!loaded.warnings.is_empty());
        assert!(loaded.library.is_none());
    }
}
