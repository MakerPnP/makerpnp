use std::fmt::Display;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub enum FileReference {
    Relative(PathBuf),
    Absolute(PathBuf),
}

impl FileReference {
    pub fn build_path(&self, root: &PathBuf) -> PathBuf {
        match self {
            FileReference::Relative(relative_path) => {
                let mut path = root.clone();
                path.push(relative_path);
                path
            }
            FileReference::Absolute(path) => path.clone(),
        }
    }

    pub fn try_build_path(&self, root: Option<&PathBuf>) -> Result<PathBuf, FileReferenceError> {
        match (self, root) {
            (FileReference::Relative(_), Some(root)) => Ok(self.build_path(root)),
            (FileReference::Relative(_), None) => Err(FileReferenceError::MissingRoot),
            (FileReference::Absolute(path), _) => Ok(path.clone()),
        }
    }
}

#[derive(Error, Debug)]
pub enum FileReferenceError {
    #[error("Missing root path for relative file reference")]
    MissingRoot,
    #[error("Invalid file reference format. Required format is 'relative=path' or 'absolute=path'.  Input: '{0}'")]
    Invalid(String),
}

impl Display for FileReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileReference::Relative(path) => write!(f, "relative='{}'", path.display()),
            FileReference::Absolute(path) => write!(f, "absolute='{}'", path.display()),
        }
    }
}

/// A document's contents, as they're saved, so a core can have them written for it, without writing them itself.
pub fn to_string<T: Serialize>(t: &T) -> Result<String, serde_json::Error> {
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(Vec::new(), formatter);
    t.serialize(&mut ser)?;

    let mut contents = ser.into_inner();
    contents.push(b'\n');

    // serde_json only writes UTF-8.
    Ok(String::from_utf8(contents).expect("UTF-8"))
}

/// A document, from its contents, as read for a core.
pub fn from_str<'de, T: Deserialize<'de>>(contents: &'de str) -> Result<T, serde_json::Error> {
    serde_json::from_str(contents)
}

impl TryFrom<&str> for FileReference {
    type Error = FileReferenceError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let parts = value.split("=").collect::<Vec<&str>>();
        if parts.len() == 2 {
            if parts[0] == "relative" {
                let path = PathBuf::from(parts[1]);
                return Ok(FileReference::Relative(path));
            } else if parts[0] == "absolute" {
                let path = PathBuf::from(value);
                return Ok(FileReference::Absolute(path));
            }
        }

        Err(FileReferenceError::Invalid(value.to_string()))
    }
}
