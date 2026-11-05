use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use cli::args::EdaToolArg;
use variantbuilder_app::variant_builders::stored_path;
use variantbuilder_app::{Event, NewVariantBuilder, VariantBuilderChange};

#[derive(Parser)]
#[command(name = "variantbuilder_cli")]
#[command(bin_name = "variantbuilder_cli")]
#[command(version, about, long_about = None)]
pub struct Opts {
    #[command(subcommand)]
    pub command: Command,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log")]
    pub trace: Option<PathBuf>,

    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
pub enum Command {
    /// Create a variant builder's file, setting the options given
    Create {
        /// Variant builder file, <design name>_<variant name>.variant.json
        #[arg(long, value_name = "FILE")]
        file: PathBuf,

        /// Replace the file if there is one
        #[arg(long)]
        overwrite: bool,

        /// Name of the design, kept as it is [default: the placements file's name, without its extension]
        #[arg(long)]
        design_name: Option<String>,

        /// Name of the assembly variant, kept as it is [default: default]
        #[arg(long)]
        variant_name: Option<String>,

        #[command(flatten)]
        args: ConfigureArgs,
    },
    /// Create an assembly rules file, with no rules, and set the variant builder's assembly rules to it
    CreateAssemblyRules {
        /// Variant builder file, <design name>_<variant name>.variant.json
        #[arg(long, value_name = "FILE")]
        file: PathBuf,

        /// Assembly rules file to create
        #[arg(long, value_name = "FILE")]
        assembly_rules: PathBuf,
    },
    /// Configure a variant builder, setting only the options given
    Configure {
        /// Variant builder file, <design name>_<variant name>.variant.json
        #[arg(long, value_name = "FILE")]
        file: PathBuf,

        #[command(flatten)]
        args: ConfigureArgs,
    },
    /// Build a variant builder's variant
    Build {
        /// Variant builder file, <design name>_<variant name>.variant.json
        #[arg(long, value_name = "FILE")]
        file: PathBuf,
    },
}

/// What `configure` sets, each only if it's given (`VARBCLI-2`).
#[derive(Args, Clone)]
pub struct ConfigureArgs {
    /// EDA tool
    #[arg(long)]
    eda: Option<EdaToolArg>,

    /// Load-out source
    #[arg(long, value_name = "SOURCE")]
    load_out: Option<PathBuf>,

    /// Placements source
    #[arg(long, value_name = "SOURCE")]
    placements: Option<PathBuf>,

    /// Parts source
    #[arg(long, value_name = "SOURCE")]
    parts: Option<PathBuf>,

    /// Part-mappings source
    #[arg(long, value_name = "SOURCE")]
    part_mappings: Option<PathBuf>,

    /// Global substitution sources, shared by designs, none to clear them
    #[arg(long, value_delimiter = ',', num_args = 0.., value_name = "SOURCE")]
    global_substitutions: Option<Vec<PathBuf>>,

    /// Design substitution sources, applied before the global ones, none to clear them
    #[arg(long, value_delimiter = ',', num_args = 0.., value_name = "SOURCE")]
    design_substitutions: Option<Vec<PathBuf>>,

    /// List of reference designators to exclude (test-pads, fiducials, etc), none to clear it
    #[arg(long, num_args = 0.., value_delimiter = ',', value_name = "REF_DES")]
    ref_des_exclusion_list: Option<Vec<String>>,

    /// List of reference designators not to place (do-not-fit, etc), none to clear it
    #[arg(long, num_args = 0.., value_delimiter = ',', value_name = "REF_DES")]
    ref_des_no_place_list: Option<Vec<String>>,

    /// Assembly rules source
    #[arg(long, value_name = "SOURCE")]
    assembly_rules: Option<PathBuf>,

    /// Output CSV file [default: <name>_placements.csv]
    #[arg(long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Output BOM CSV file (Currently JLCPCB format only) [default: <name>_bom.csv]
    #[arg(long, value_name = "FILE")]
    output_bom: Option<PathBuf>,

    /// List of reference designators for assembly variant (all if there are none), none to clear it
    #[arg(long, num_args = 0.., value_delimiter = ',', value_name = "REF_DES")]
    ref_des_list: Option<Vec<String>>,

    /// Options to clear
    #[arg(long, num_args = 1.., value_delimiter = ',', value_name = "OPTION")]
    clear: Vec<ClearArg>,
}

/// An option of `configure` that can be cleared.
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ClearArg {
    Eda,
    LoadOut,
    Placements,
    Parts,
    PartMappings,
    AssemblyRules,
    Output,
    OutputBom,
}

impl Opts {
    /// The event the command asks the core for. A relative path given to `configure` is relative to the folder the CLI
    /// is run in, so it's made relative to the variant builder's folder, or absolute if it's outside it (`VARBCLI-2`).
    pub fn event(self, current_dir: &Path) -> Event {
        let stored = |file: &Path, path: PathBuf| stored_path(&current_dir.join(file), &current_dir.join(path));
        match self.command {
            Command::Create {
                file,
                overwrite,
                design_name,
                variant_name,
                args,
            } => {
                let changes = args.changes(|path| stored(&file, path));
                Event::Create {
                    path: file,
                    overwrite,
                    new: NewVariantBuilder {
                        design_name,
                        variant_name,
                        changes,
                    },
                }
            }
            Command::CreateAssemblyRules {
                file,
                assembly_rules,
            } => Event::CreateAssemblyRules {
                assembly_rules: stored(&file, assembly_rules),
                path: file,
            },
            Command::Configure {
                file,
                args,
            } => {
                let changes = args.changes(|path| stored(&file, path));
                Event::Configure {
                    path: file,
                    changes,
                }
            }
            Command::Build {
                file,
            } => Event::Build {
                path: file,
            },
        }
    }
}

impl ConfigureArgs {
    /// The changes, in the order of the variant builder's fields, then those cleared.
    fn changes(self, path: impl Fn(PathBuf) -> PathBuf) -> Vec<VariantBuilderChange> {
        let ConfigureArgs {
            eda,
            load_out,
            placements,
            parts,
            part_mappings,
            global_substitutions,
            design_substitutions,
            ref_des_exclusion_list,
            ref_des_no_place_list,
            assembly_rules,
            output,
            output_bom,
            ref_des_list,
            clear,
        } = self;

        let path = &path;
        let some_path = |value: Option<PathBuf>| value.map(|value| Some(path(value)));
        let paths = |values: Option<Vec<PathBuf>>| values.map(|values| values.into_iter().map(path).collect());
        let changes = [
            eda.map(|eda| VariantBuilderChange::EdaTool(Some(eda.build()))),
            some_path(placements).map(VariantBuilderChange::Placements),
            some_path(parts).map(VariantBuilderChange::Parts),
            some_path(part_mappings).map(VariantBuilderChange::PartMappings),
            paths(global_substitutions).map(VariantBuilderChange::GlobalSubstitutions),
            some_path(load_out).map(VariantBuilderChange::LoadOut),
            some_path(assembly_rules).map(VariantBuilderChange::AssemblyRules),
            paths(design_substitutions).map(VariantBuilderChange::DesignSubstitutions),
            ref_des_list.map(VariantBuilderChange::RefDesList),
            ref_des_exclusion_list.map(VariantBuilderChange::RefDesExclusionList),
            ref_des_no_place_list.map(VariantBuilderChange::RefDesNoPlaceList),
            some_path(output).map(VariantBuilderChange::OutputPlacements),
            some_path(output_bom).map(VariantBuilderChange::OutputBom),
        ];

        changes
            .into_iter()
            .flatten()
            .chain(
                clear
                    .into_iter()
                    .map(|option| match option {
                        ClearArg::Eda => VariantBuilderChange::EdaTool(None),
                        ClearArg::LoadOut => VariantBuilderChange::LoadOut(None),
                        ClearArg::Placements => VariantBuilderChange::Placements(None),
                        ClearArg::Parts => VariantBuilderChange::Parts(None),
                        ClearArg::PartMappings => VariantBuilderChange::PartMappings(None),
                        ClearArg::AssemblyRules => VariantBuilderChange::AssemblyRules(None),
                        ClearArg::Output => VariantBuilderChange::OutputPlacements(None),
                        ClearArg::OutputBom => VariantBuilderChange::OutputBom(None),
                    }),
            )
            .collect()
    }
}
