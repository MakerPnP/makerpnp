use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use thiserror::Error;

// FUTURE maybe this should be a url?
#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
pub enum Source {
    File(PathBuf),
    Url(String),
}

impl FromStr for Source {
    type Err = SourceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Source::File(PathBuf::from(s)))
    }
}

impl Source {
    pub fn try_from_relative_path(path: PathBuf) -> Result<Source, SourceError> {
        if !path.is_relative() {
            panic!()
        }
        Self::try_from_path_inner(path)
    }

    pub fn try_from_path_inner(path: PathBuf) -> Result<Source, SourceError> {
        if !path.exists() {
            return Err(SourceError::PathDoesNotExist(path));
        }
        if !path.is_file() {
            return Err(SourceError::PathIsNotAFile(path));
        }
        Ok(Source::File(path))
    }

    pub fn try_from_absolute_path(path: PathBuf) -> Result<Source, SourceError> {
        if !path.is_absolute() {
            panic!()
        }
        Self::try_from_path_inner(path)
    }

    pub fn try_from_path(project_path: &PathBuf, path: PathBuf) -> Result<Source, SourceError> {
        match path.is_absolute() {
            true => Self::from_absolute_path(path.clone()),
            false => {
                let full_path = project_path.clone().join(path);
                Self::try_from_path_inner(full_path)
            }
        }
    }

    /// The file at the path, relative to the directory unless it's absolute, without checking that it exists, so it can
    /// be used by a core, which does no IO.
    pub fn from_path(directory: &Path, path: &Path) -> Source {
        Source::File(directory.join(path))
    }

    pub fn from_absolute_path(path: PathBuf) -> Result<Source, SourceError> {
        assert!(path.is_absolute());
        Ok(Source::File(path))
    }

    pub fn path(&self) -> Result<PathBuf, SourceError> {
        match self {
            Source::File(path) => Ok(path.clone()),
            Source::Url(_) => Err(SourceError::NotAPath),
        }
    }
}

impl Display for Source {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::File(path) => f.write_str(path.display().to_string().as_str()),
            Source::Url(_) => {
                unimplemented!()
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("Path does not exist. path: {0}")]
    PathDoesNotExist(PathBuf),
    #[error("Path is not a file. path: {0}")]
    PathIsNotAFile(PathBuf),
    #[error("Source is not a path.")]
    NotAPath,
}
