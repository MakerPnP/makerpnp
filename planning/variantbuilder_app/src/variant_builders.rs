//! A variant builder's file, `<name>.variant.json`, which defines a variant, and the data sources it's built from, so
//! the user defines it once, and builds it again and again (`VARB`).

use std::path::{Path, PathBuf};

use eda::EdaTool;
use stores_protocol::{Revision, StoresOperation, StoresOutput};
use thiserror::Error;
use tracing::info;

use crate::{AssemblyVariant, BuildArgs, Context, Source};

/// The extension of a variant builder's file, after its name.
pub const EXTENSION: &str = ".variant.json";

/// What a variant builder's file says (`VARB-1`). A field left out of the file is read as it is when it's new, so the
/// file can be written by hand. A relative path is relative to the file's folder.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct VariantBuilderFile {
    pub variant: Variant,
    pub eda_tool: Option<EdaTool>,
    pub placements: Option<PathBuf>,
    pub parts: Option<PathBuf>,
    pub part_mappings: Option<PathBuf>,
    pub substitutions: Vec<PathBuf>,
    pub load_out: Option<PathBuf>,
    pub assembly_rules: Option<PathBuf>,
    pub ref_des_exclude_list: Vec<String>,
    pub ref_des_disable_list: Vec<String>,
    pub output_placements: Option<PathBuf>,
    pub output_bom: Option<PathBuf>,
}

/// The variant to build: its name, and the ref-des it's limited to, or all if there are none.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Variant {
    pub name: String,
    pub ref_des_list: Vec<String>,
}

impl Default for Variant {
    fn default() -> Self {
        let AssemblyVariant {
            name,
            ref_des_list,
        } = AssemblyVariant::default();
        Self {
            name,
            ref_des_list,
        }
    }
}

/// A change to one field of a variant builder's file, setting it, or clearing it with `None` or an empty list
/// (`VARB-4`).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum VariantBuilderChange {
    VariantName(String),
    RefDesList(Vec<String>),
    EdaTool(Option<EdaTool>),
    Placements(Option<PathBuf>),
    Parts(Option<PathBuf>),
    PartMappings(Option<PathBuf>),
    Substitutions(Vec<PathBuf>),
    LoadOut(Option<PathBuf>),
    AssemblyRules(Option<PathBuf>),
    RefDesExcludeList(Vec<String>),
    RefDesDisableList(Vec<String>),
    OutputPlacements(Option<PathBuf>),
    OutputBom(Option<PathBuf>),
}

impl VariantBuilderFile {
    pub fn apply(&mut self, change: VariantBuilderChange) {
        match change {
            VariantBuilderChange::VariantName(name) => self.variant.name = name,
            VariantBuilderChange::RefDesList(list) => self.variant.ref_des_list = list,
            VariantBuilderChange::EdaTool(eda_tool) => self.eda_tool = eda_tool,
            VariantBuilderChange::Placements(path) => self.placements = path,
            VariantBuilderChange::Parts(path) => self.parts = path,
            VariantBuilderChange::PartMappings(path) => self.part_mappings = path,
            VariantBuilderChange::Substitutions(paths) => self.substitutions = paths,
            VariantBuilderChange::LoadOut(path) => self.load_out = path,
            VariantBuilderChange::AssemblyRules(path) => self.assembly_rules = path,
            VariantBuilderChange::RefDesExcludeList(list) => self.ref_des_exclude_list = list,
            VariantBuilderChange::RefDesDisableList(list) => self.ref_des_disable_list = list,
            VariantBuilderChange::OutputPlacements(path) => self.output_placements = path,
            VariantBuilderChange::OutputBom(path) => self.output_bom = path,
        }
    }

    /// The file's contents, as JSON, indented as the planner's documents are.
    pub fn contents(&self) -> Result<String, VariantBuilderError> {
        let mut buffer = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, formatter);
        serde::Serialize::serialize(self, &mut serializer).map_err(|error| VariantBuilderError::Store {
            cause: error.to_string(),
        })?;
        String::from_utf8(buffer).map_err(|error| VariantBuilderError::Store {
            cause: error.to_string(),
        })
    }

    fn from_contents(path: &Path, contents: &str) -> Result<Self, VariantBuilderError> {
        serde_json::from_str(contents).map_err(|error| VariantBuilderError::Invalid {
            path: path.to_path_buf(),
            cause: error.to_string(),
        })
    }

    /// What's needed to build the variant, with each path relative to the file's folder, or the fields that aren't set
    /// (`VARB-5`).
    pub(crate) fn build_args(&self, path: &Path) -> Result<BuildArgs, VariantBuilderError> {
        let mut missing = vec![];
        if self.eda_tool.is_none() {
            missing.push("eda_tool".to_string());
        }
        if self.placements.is_none() {
            missing.push("placements".to_string());
        }
        if self.parts.is_none() {
            missing.push("parts".to_string());
        }
        if self.part_mappings.is_none() {
            missing.push("part_mappings".to_string());
        }
        let (Some(eda_tool), Some(placements), Some(parts), Some(part_mappings)) =
            (self.eda_tool, &self.placements, &self.parts, &self.part_mappings)
        else {
            return Err(VariantBuilderError::Incomplete {
                path: path.to_path_buf(),
                missing,
            });
        };

        let folder = folder(path);
        let source = |file: &PathBuf| Source::from_path(&folder, file);
        let name = name(path);
        let output_placements = self
            .output_placements
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("{}_placements.csv", name)));
        let output_bom = self
            .output_bom
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("{}_bom.csv", name)));

        Ok(BuildArgs {
            eda_tool,
            placements: source(placements),
            assembly_variant: AssemblyVariant::new(self.variant.name.clone(), self.variant.ref_des_list.clone()),
            parts: source(parts),
            part_mappings: source(part_mappings),
            substitutions: self
                .substitutions
                .iter()
                .map(source)
                .collect(),
            load_out: self.load_out.as_ref().map(source),
            assembly_rules: self.assembly_rules.as_ref().map(source),
            output: folder
                .join(output_placements)
                .display()
                .to_string(),
            output_bom: Some(
                folder
                    .join(output_bom)
                    .display()
                    .to_string(),
            ),
            ref_des_exclude_list: self.ref_des_exclude_list.clone(),
            ref_des_disable_list: self.ref_des_disable_list.clone(),
        })
    }
}

/// The folder a variant builder's file is in, which its relative paths are relative to.
fn folder(path: &Path) -> PathBuf {
    path.parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// A variant builder's name: its file's name, without [`EXTENSION`].
pub fn name(path: &Path) -> String {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    match file_name.strip_suffix(EXTENSION) {
        Some(name) => name.to_string(),
        None => path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default(),
    }
}

/// An open variant builder, as its file says, at the file's revision (`VARB-3`).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct VariantBuilderView {
    pub file: VariantBuilderFile,
    pub revision: Revision,
}

/// Why a variant builder couldn't be created, opened, configured or built.
#[derive(Error, serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum VariantBuilderError {
    #[error("Variant builder exists already. path: '{}'", path.display())]
    ExistsAlready { path: PathBuf },
    #[error("Variant builder not found. path: '{}'", path.display())]
    NotFound { path: PathBuf },
    #[error("Invalid variant builder. path: '{}', cause: '{cause}'", path.display())]
    Invalid { path: PathBuf, cause: String },
    #[error("Variant builder incomplete. path: '{}', missing: {missing:?}", path.display())]
    Incomplete { path: PathBuf, missing: Vec<String> },
    /// The stores service couldn't read or write a file, or the build failed, saying why.
    #[error("{cause}")]
    Store { cause: String },
}

impl From<String> for VariantBuilderError {
    fn from(cause: String) -> Self {
        VariantBuilderError::Store {
            cause,
        }
    }
}

/// Writes a new variant builder's file, unless there's one and it's not to be overwritten (`VARB-2`).
pub(crate) async fn create(
    ctx: &Context,
    path: &Path,
    overwrite: bool,
) -> Result<VariantBuilderView, VariantBuilderError> {
    if !overwrite
        && read_document(ctx, path)
            .await?
            .is_some()
    {
        return Err(VariantBuilderError::ExistsAlready {
            path: path.to_path_buf(),
        });
    }
    let file = VariantBuilderFile::default();
    let revision = write_document(ctx, path, file.contents()?).await?;
    Ok(VariantBuilderView {
        file,
        revision,
    })
}

/// Reads a variant builder's file (`VARB-3`).
pub(crate) async fn open(ctx: &Context, path: &Path) -> Result<VariantBuilderView, VariantBuilderError> {
    let document = read_document(ctx, path)
        .await?
        .ok_or_else(|| VariantBuilderError::NotFound {
            path: path.to_path_buf(),
        })?;
    Ok(VariantBuilderView {
        file: VariantBuilderFile::from_contents(path, &document.contents)?,
        revision: document.revision,
    })
}

/// Reads a variant builder's file, makes the changes, and writes it (`VARB-4`).
pub(crate) async fn configure(
    ctx: &Context,
    path: &Path,
    changes: Vec<VariantBuilderChange>,
) -> Result<VariantBuilderView, VariantBuilderError> {
    let VariantBuilderView {
        mut file, ..
    } = open(ctx, path).await?;
    for change in changes {
        file.apply(change);
    }
    let revision = write_document(ctx, path, file.contents()?).await?;
    Ok(VariantBuilderView {
        file,
        revision,
    })
}

/// Reads a variant builder's file, and builds its variant (`VARB-5`).
pub(crate) async fn build(ctx: &Context, path: &Path) -> Result<(), VariantBuilderError> {
    let VariantBuilderView {
        file, ..
    } = open(ctx, path).await?;
    let args = file.build_args(path)?;
    info!("Building variant. path: '{}'", path.display());
    crate::build(ctx, args).await?;
    Ok(())
}

async fn read_document(ctx: &Context, path: &Path) -> Result<Option<stores_protocol::Document>, VariantBuilderError> {
    match ctx
        .request_from_shell(StoresOperation::ReadDocument {
            path: path.to_path_buf(),
        })
        .await
    {
        StoresOutput::DocumentRead(result) => Ok(result?),
        output => Err(format!("Unexpected output. output: {:?}", output).into()),
    }
}

async fn write_document(ctx: &Context, path: &Path, contents: String) -> Result<Revision, VariantBuilderError> {
    Ok(crate::write_document(ctx, &path.to_path_buf(), contents).await?)
}
