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

/// The variant name of a new variant builder, unless another is given.
pub const DEFAULT_VARIANT_NAME: &str = "default";

/// The contents of a new assembly rules file: the header row, and no rules (`VARB-8`).
pub const NEW_ASSEMBLY_RULES: &str = "\"RefDes\",\"Manufacturer\",\"Mpn\"\n";

/// What a variant builder's file says (`VARB-1`). A field left out of the file is read as it is when it's new, so the
/// file can be written by hand. A relative path is relative to the file's folder.
///
/// The fields are in the groups the GUI shows them in: what the variant builder is of, part matching, design specific,
/// reference designators, and output.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct VariantBuilderFile {
    /// The design's name, set when the variant builder is created, and kept as it was (`VARB-2`).
    pub design_name: String,
    /// The variant's name, set when the variant builder is created, and kept as it was (`VARB-2`).
    pub variant_name: String,
    pub eda_tool: Option<EdaTool>,
    pub placements: Option<PathBuf>,

    pub parts: Option<PathBuf>,
    pub part_mappings: Option<PathBuf>,
    /// Substitutions shared by designs, applied after the design's own.
    pub global_substitutions: Vec<PathBuf>,
    pub load_out: Option<PathBuf>,

    pub assembly_rules: Option<PathBuf>,
    /// Substitutions of this design, applied first.
    pub design_substitutions: Vec<PathBuf>,

    /// The ref-des the variant is limited to, or all if there are none.
    pub ref_des_list: Vec<String>,
    /// The ref-des left out, such as test-pads and fiducials.
    pub ref_des_exclusion_list: Vec<String>,
    /// The ref-des not placed, such as do-not-fit.
    pub ref_des_no_place_list: Vec<String>,

    pub output_placements: Option<PathBuf>,
    pub output_bom: Option<PathBuf>,
}

impl Default for VariantBuilderFile {
    fn default() -> Self {
        Self {
            design_name: String::new(),
            variant_name: DEFAULT_VARIANT_NAME.to_string(),
            eda_tool: None,
            placements: None,
            parts: None,
            part_mappings: None,
            global_substitutions: vec![],
            load_out: None,
            assembly_rules: None,
            design_substitutions: vec![],
            ref_des_list: vec![],
            ref_des_exclusion_list: vec![],
            ref_des_no_place_list: vec![],
            output_placements: None,
            output_bom: None,
        }
    }
}

/// A change to one field of a variant builder's file, setting it, or clearing it with `None` or an empty list
/// (`VARB-4`). The design name and variant name are set only as it's created (`VARB-2`).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum VariantBuilderChange {
    EdaTool(Option<EdaTool>),
    Placements(Option<PathBuf>),
    Parts(Option<PathBuf>),
    PartMappings(Option<PathBuf>),
    GlobalSubstitutions(Vec<PathBuf>),
    LoadOut(Option<PathBuf>),
    AssemblyRules(Option<PathBuf>),
    DesignSubstitutions(Vec<PathBuf>),
    RefDesList(Vec<String>),
    RefDesExclusionList(Vec<String>),
    RefDesNoPlaceList(Vec<String>),
    OutputPlacements(Option<PathBuf>),
    OutputBom(Option<PathBuf>),
}

/// What a new variant builder is created with (`VARB-2`): the design and variant names, which are kept as they are,
/// and the changes made to it before it's written.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
pub struct NewVariantBuilder {
    /// The design's name, or none for the name of the placements' file, without its extension.
    pub design_name: Option<String>,
    /// The variant's name, or none for [`DEFAULT_VARIANT_NAME`].
    pub variant_name: Option<String>,
    pub changes: Vec<VariantBuilderChange>,
}

impl NewVariantBuilder {
    /// The new variant builder's file.
    pub fn file(self) -> VariantBuilderFile {
        let mut file = VariantBuilderFile::default();
        for change in self.changes {
            file.apply(change);
        }
        file.design_name = self.design_name.unwrap_or_else(|| {
            file.placements
                .as_deref()
                .map(design_name)
                .unwrap_or_default()
        });
        if let Some(variant_name) = self.variant_name {
            file.variant_name = variant_name;
        }
        file
    }
}

impl VariantBuilderFile {
    pub fn apply(&mut self, change: VariantBuilderChange) {
        match change {
            VariantBuilderChange::EdaTool(eda_tool) => self.eda_tool = eda_tool,
            VariantBuilderChange::Placements(path) => self.placements = path,
            VariantBuilderChange::Parts(path) => self.parts = path,
            VariantBuilderChange::PartMappings(path) => self.part_mappings = path,
            VariantBuilderChange::GlobalSubstitutions(paths) => self.global_substitutions = paths,
            VariantBuilderChange::LoadOut(path) => self.load_out = path,
            VariantBuilderChange::AssemblyRules(path) => self.assembly_rules = path,
            VariantBuilderChange::DesignSubstitutions(paths) => self.design_substitutions = paths,
            VariantBuilderChange::RefDesList(list) => self.ref_des_list = list,
            VariantBuilderChange::RefDesExclusionList(list) => self.ref_des_exclusion_list = list,
            VariantBuilderChange::RefDesNoPlaceList(list) => self.ref_des_no_place_list = list,
            VariantBuilderChange::OutputPlacements(path) => self.output_placements = path,
            VariantBuilderChange::OutputBom(path) => self.output_bom = path,
        }
    }

    /// The substitutions, in the order they're applied: the design's own first, so that where a design's substitution
    /// and a global one both apply, the design's is applied, then the global ones, each in its list's order (`VARB-5`).
    pub fn substitutions(&self) -> impl Iterator<Item = &PathBuf> {
        self.design_substitutions
            .iter()
            .chain(self.global_substitutions.iter())
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
            assembly_variant: AssemblyVariant::new(self.variant_name.clone(), self.ref_des_list.clone()),
            parts: source(parts),
            part_mappings: source(part_mappings),
            substitutions: self
                .substitutions()
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
            ref_des_exclude_list: self.ref_des_exclusion_list.clone(),
            ref_des_disable_list: self.ref_des_no_place_list.clone(),
        })
    }
}

/// The name of a new variant builder's file, `<design name>_<variant name>.variant.json` (`VARB-2`).
pub fn file_name(design_name: &str, variant_name: &str) -> String {
    format!("{}_{}{}", design_name, variant_name, EXTENSION)
}

/// The design name for a placements file: its name, without its extension (`VARB-2`).
pub fn design_name(placements: &Path) -> String {
    placements
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// The extensions of the placements files the EDA tool writes, which the placements file is chosen from (`VARB-1`).
pub fn placements_extensions(eda_tool: EdaTool) -> &'static [&'static str] {
    match eda_tool {
        EdaTool::DipTrace => &["csv"],
        EdaTool::KiCad => &["csv"],
        // tab-separated, but named `.csv`.
        EdaTool::EasyEda => &["csv"],
    }
}

/// The folder a variant builder's file is in, which its relative paths are relative to.
fn folder(path: &Path) -> PathBuf {
    path.parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// The path to keep in a variant builder's file for a file the user chose: relative to the variant builder's folder,
/// if it's in it, or else as it is (`VARB-1`). Both are relative to the same folder, or absolute, such as the folder a
/// front end is run in. Only the paths are compared, no file is read.
pub fn stored_path(variant_builder: &Path, path: &Path) -> PathBuf {
    match path.strip_prefix(folder(variant_builder)) {
        Ok(relative) if !relative.as_os_str().is_empty() => relative.to_path_buf(),
        _ => path.to_path_buf(),
    }
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
    #[error("Assembly rules exist already. path: '{}'", path.display())]
    AssemblyRulesExistAlready { path: PathBuf },
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
    new: NewVariantBuilder,
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
    let file = new.file();
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

/// Writes a new assembly rules file, with no rules, unless there's one, then sets the variant builder's assembly rules
/// to it (`VARB-8`). `assembly_rules` is relative to the variant builder's folder, or absolute.
pub(crate) async fn create_assembly_rules(
    ctx: &Context,
    path: &Path,
    assembly_rules: &Path,
) -> Result<VariantBuilderView, VariantBuilderError> {
    // the variant builder is read first, so that one that can't be read isn't left with a file it doesn't use.
    open(ctx, path).await?;
    let assembly_rules_path = folder(path).join(assembly_rules);
    if read_document(ctx, &assembly_rules_path)
        .await?
        .is_some()
    {
        return Err(VariantBuilderError::AssemblyRulesExistAlready {
            path: assembly_rules_path,
        });
    }
    write_document(ctx, &assembly_rules_path, NEW_ASSEMBLY_RULES.to_string()).await?;
    configure(ctx, path, vec![VariantBuilderChange::AssemblyRules(Some(
        assembly_rules.to_path_buf(),
    ))])
    .await
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
