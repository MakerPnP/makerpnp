#[macro_use]
extern crate util;

use std::collections::BTreeMap;

use nalgebra::Vector2;
use planning::design::DesignIndex;
use pnp::panel::Dimensions;
use pnp::pcb::PcbUnitIndex;

use crate::common::project_builder::TestDesignSizing;

pub mod common;

mod operation_sequence_1 {
    use std::collections::BTreeMap;
    use std::fs::{read_to_string, File};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    use assert_cmd::Command;
    use indoc::indoc;
    use nalgebra::Vector2;
    use planning::design::DesignVariant;
    use planning::file::FileReference;
    use planning::pcb::PcbAssemblyOrientation;
    use planning::placement::{PlacementOperation, PlacementStatus, ProjectPlacementStatus};
    use planning::process::{TaskKind, TaskStatus};
    use pnp::object_path::ObjectPath;
    use pnp::panel::Dimensions;
    use pnp::pcb::PcbSide;
    use rust_decimal_macros::dec;
    use stores::test::load_out_builder::{LoadOutCSVBuilder, TestLoadOutRecord};
    use tempfile::tempdir;
    use util::test::{build_temp_file, prepare_args, print};

    use crate::common::operation_history::{
        TestOperationHistoryItem, TestOperationHistoryKind, TestPlacementOperationHistoryKind,
        TestTaskOperationHistoryKind,
    };
    use crate::common::package_mappings_builder::{PackageMappingsCSVBuilder, TestPackageMappingRecord};
    use crate::common::packages_builder::{PackagesCSVBuilder, TestPackageRecord};
    use crate::common::phase_placement_builder::{PhasePlacementsCSVBuilder, TestPhasePlacementRecord};
    use crate::common::project_builder as project;
    use crate::common::project_builder::{
        TestDesignSizing, TestGenericTaskState, TestOperationState, TestPartState, TestPcbUnitPositioning, TestPhase,
        TestPlacement, TestPlacementState, TestPlacementTaskState, TestProcessDefinition, TestProcessOperationStatus,
        TestProject, TestSerializableTaskState, TestUnitPosition,
    };
    use crate::common::project_report_builder as report;
    use crate::common::project_report_builder::{
        ProjectReportBuilder, TestGenericTaskOverview, TestGenericTaskSpecification, TestIssue, TestIssueKind,
        TestIssueSeverity, TestLoadPcbsTaskOverview, TestLoadPcbsTaskSpecification, TestPart, TestPcbUnitAssignment,
        TestPhaseLoadOutAssignmentItem, TestPhaseOperation, TestPhaseOperationOverview, TestPhaseOverview,
        TestPhaseSpecification, TestPlaceComponentsTaskOverview, TestPlaceComponentsTaskSpecification,
        TestTaskOverview, TestTaskSpecification,
    };
    use crate::{calculate_offset, calculate_size};

    /// A context, which will be dropped when the tests are completed.
    mod context {
        use std::fs;
        use std::path::Path;
        use std::sync::{Mutex, MutexGuard};
        use std::thread::sleep;
        use std::time::{Duration, Instant};

        use util::source::Source;

        use super::*;
        use crate::common::project_builder::TestLibraryConfig;

        #[derive(Debug)]
        pub struct Context {
            pub temp_dir: tempfile::TempDir,

            pub trace_log_arg: String,
            pub path_arg: String,
            pub project_arg: String,
            pub test_trace_log_path: PathBuf,
            pub test_project_path: PathBuf,
            pub test_pcb_1_path: PathBuf,
            pub test_pcb_1_arg: String,
            pub pnp_process_path: PathBuf,
            pub pnp_process_arg: String,
            pub manual_process_path: PathBuf,
            pub manual_process_arg: String,
            pub phase_1_load_out_path: PathBuf,
            pub phase_2_load_out_path: PathBuf,
            pub phase_1_log_path: PathBuf,
            pub packages_path: PathBuf,
            pub package_mappings_path: PathBuf,
            pub library_config: TestLibraryConfig,
        }

        impl Context {
            pub fn new() -> Self {
                let temp_dir = tempdir().unwrap();

                let path_arg = format!("--path {}", temp_dir.path().to_str().unwrap());

                let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
                let trace_log_arg = format!(
                    "--trace {}",
                    test_trace_log_file_name
                        .to_str()
                        .unwrap()
                );

                let (test_project_path, _test_project_file_name) =
                    build_temp_file(&temp_dir, "project-job1", "mpnp.json");

                let (packages_path, _packages_file_name) = build_temp_file(&temp_dir, "packages", "csv");
                let (package_mappings_path, _package_mappings_file_name) =
                    build_temp_file(&temp_dir, "package_mappings", "csv");

                let library_config = TestLibraryConfig {
                    package_source: Source::from_absolute_path(packages_path.clone()).ok(),
                    package_mappings_source: Source::from_absolute_path(package_mappings_path.clone()).ok(),
                };

                let (test_pcb_1_path, _test_pcb_1_file_name) = build_temp_file(&temp_dir, "panel_a", "pcb.json");
                let test_pcb_1_arg = format!("--pcb-file {}", test_pcb_1_path.to_str().unwrap());

                let (pnp_process_path, _) = build_temp_file(&temp_dir, "pnp", "process.json");
                let pnp_process_arg = format!("--process-file {}", pnp_process_path.to_str().unwrap());
                let (manual_process_path, _) = build_temp_file(&temp_dir, "manual", "process.json");
                let manual_process_arg = format!("--process-file {}", manual_process_path.to_str().unwrap());

                let project_arg = "--project job1".to_string();

                let mut phase_1_load_out_path = PathBuf::from(temp_dir.path());
                phase_1_load_out_path.push("phase_1_top_1_load_out.csv");

                let mut phase_1_log_path = PathBuf::from(temp_dir.path());
                phase_1_log_path.push("top_1_log.json");

                let mut phase_2_load_out_path = PathBuf::from(temp_dir.path());
                phase_2_load_out_path.push("phase_2_bottom_1_load_out.csv");

                Context {
                    temp_dir,
                    path_arg,
                    project_arg,
                    trace_log_arg,
                    test_trace_log_path,
                    test_project_path,
                    test_pcb_1_path,
                    test_pcb_1_arg,
                    pnp_process_path,
                    pnp_process_arg,
                    manual_process_path,
                    manual_process_arg,
                    phase_1_load_out_path,
                    phase_1_log_path,
                    phase_2_load_out_path,
                    packages_path,
                    package_mappings_path,
                    library_config,
                }
            }

            pub fn delete_trace_log(&self) {
                if Path::new(&self.test_trace_log_path).exists() {
                    println!(
                        "deleting trace log: {}",
                        self.test_trace_log_path
                            .to_str()
                            .unwrap()
                    );
                    fs::remove_file(&self.test_trace_log_path).unwrap();
                }
            }
        }

        impl Drop for Context {
            fn drop(&mut self) {
                println!(
                    "destroying context. temp_dir: {}",
                    self.temp_dir.path().to_str().unwrap()
                );
            }
        }

        /// IMPORTANT: lock content must be dropped manually, as static items are never dropped.
        static LOCK: Mutex<(usize, Option<Context>)> = Mutex::new((0, None));

        /// How long a step waits for the sequence to move on before failing, as the step before it isn't running.
        const STALLED: Duration = Duration::from_secs(30);

        /// Use a mutex to prevent multiple test threads interacting with the same static state.
        /// This can happen when tests use the same mock context.  Without this mechanism tests will
        /// interact with each other causing unexpected results and test failures.
        ///
        /// Each step runs after the one before it, so a step run on its own, filtered by name, fails once the sequence
        /// has stalled, rather than waiting forever.
        pub fn acquire(sequence: usize) -> MutexGuard<'static, (usize, Option<Context>)> {
            let mut last_seen = (usize::MAX, Instant::now());
            let mut lock = loop {
                let mut lock = LOCK
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if lock.0 == sequence - 1 {
                    lock.0 += 1;
                    break lock;
                }
                let reached = lock.0;
                drop(lock);

                if reached != last_seen.0 {
                    last_seen = (reached, Instant::now());
                } else if last_seen.1.elapsed() > STALLED {
                    panic!(
                        "operation_sequence_1 step {} waited {:?} for step {}, which isn't running. The steps depend \
                         on each other: run the whole module, `cargo test -p planner_cli --test planner \
                         operation_sequence_1`.",
                        sequence,
                        STALLED,
                        sequence - 1
                    );
                }

                sleep(Duration::from_millis(100));
            };

            if lock.1.is_none() {
                lock.1.replace(Context::new());
            }

            lock
        }
    }

    pub fn read_and_show_file(path: &Path) -> std::io::Result<String> {
        println!("File: {:?}", path);
        let content: String = read_to_string(path)?;
        println!("{}", content);

        Ok(content)
    }

    #[test]
    fn sequence_01_create_job() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(1);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .content();

        // and
        let packages_arg = format!("--packages {}", ctx.packages_path.to_str().unwrap());
        let package_mappings_arg = format!(
            "--package-mappings {}",
            ctx.package_mappings_path
                .to_str()
                .unwrap()
        );

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "-vvv",
            "create",
            packages_arg.as_str(),
            package_mappings_arg.as_str(),
        ]);
        println!("args: {:?}", args);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        // `DOC-1`, `STORE-10`, the project is created once the stores service has written its file.
        assert_contains_inorder!(trace_content, [
            "planner_app: Received event. event: CreateProject {",
            "Creating project. path: ",
            "stores_service: Performing stores operation. operation: WriteDocument { path: ",
            "planner_app: Received event. event: ProjectWritten {",
            "output: DocumentWritten(Ok(",
            "Created project successfully",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_02_create_pcb() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(2);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_pcb_1_content = project::TestPcb {
            name: "panel_a".to_string(),
            units: 4,
            design_names: vec!["design_a".into(), "design_b".into()],
            unit_map: BTreeMap::from_iter([(0, 0), (1, 1), (2, 0), (3, 1)]),
            gerber_offset: Default::default(),
            panel_sizing: project::TestPanelSizing {
                units: Default::default(),
                size: Default::default(),
                edge_rails: Default::default(),
                fiducials: vec![],
                design_sizings: vec![
                    TestDesignSizing {
                        origin: Default::default(),
                        gerber_offset: Default::default(),
                        placement_offset: Default::default(),
                        size: Default::default(),
                    },
                    TestDesignSizing {
                        origin: Default::default(),
                        gerber_offset: Default::default(),
                        placement_offset: Default::default(),
                        size: Default::default(),
                    },
                ],
                pcb_unit_positionings: vec![
                    TestPcbUnitPositioning {
                        offset: Default::default(),
                        rotation: dec!(0),
                    },
                    TestPcbUnitPositioning {
                        offset: Default::default(),
                        rotation: dec!(0),
                    },
                    TestPcbUnitPositioning {
                        offset: Default::default(),
                        rotation: dec!(0),
                    },
                    TestPcbUnitPositioning {
                        offset: Default::default(),
                        rotation: dec!(0),
                    },
                ],
            },
            orientation: PcbAssemblyOrientation::default(),
        }
        .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "pcb",
            "-vvv",
            ctx.test_pcb_1_arg.as_str(),
            "create",
            "--name panel_a",
            "--units 4",
            "--design 1=design_a,2=design_b,3=design_a,4=design_b",
        ]);
        println!("args: {:?}", args);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and

        let saving_pcb_message = &format!("Saving PCB. path: {:?}\n", ctx.test_pcb_1_path);

        assert_contains_inorder!(trace_content, [
            "Creating PCB. name: 'panel_a'\n",
            "Added designs to PCB. design: [design_a, design_b]\n",
            saving_pcb_message,
        ]);

        // and
        let pcb_1_content: String = read_and_show_file(&ctx.test_pcb_1_path)?;

        assert_eq!(pcb_1_content, expected_pcb_1_content);

        Ok(())
    }

    #[test]
    fn sequence_03_configure_panel_sizing() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(3);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        let edge_rails = Dimensions {
            left: 10.0,
            right: 10.0,
            top: 5.0,
            bottom: 5.0,
        };
        let routing_gap = 2.0;

        let design_names = vec!["design_a".into(), "design_b".into()];
        let design_a_sizing = TestDesignSizing {
            size: Vector2::new(60.0, 50.0),
            origin: Vector2::new(30.0, 25.0),
            gerber_offset: Vector2::new(-20.0, -20.0),
            placement_offset: Vector2::new(-10.0, -10.0),
        };
        let design_b_sizing = TestDesignSizing {
            size: Vector2::new(40.0, 50.0),
            origin: Vector2::new(20.0, 25.0),
            gerber_offset: Vector2::new(-20.0, -20.0),
            placement_offset: Vector2::new(-10.0, -10.0),
        };
        let design_sizings = vec![design_a_sizing, design_b_sizing];

        let unit_map = BTreeMap::from_iter([(0, 0), (1, 1), (2, 0), (3, 1)]);

        let size = calculate_size(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2);

        let pcb_unit_positions = vec![
            TestPcbUnitPositioning {
                offset: calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2, 0, 0),
                rotation: dec!(0),
            },
            TestPcbUnitPositioning {
                offset: calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2, 1, 0),
                rotation: dec!(0),
            },
            TestPcbUnitPositioning {
                offset: calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2, 0, 1),
                rotation: dec!(0),
            },
            TestPcbUnitPositioning {
                offset: calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 2, 2, 1, 1),
                rotation: dec!(0),
            },
        ];

        // and
        let expected_pcb_1_content = project::TestPcb {
            name: "panel_a".to_string(),
            units: 4,
            design_names: design_names.clone(),
            unit_map: unit_map.clone(),
            gerber_offset: Default::default(),
            panel_sizing: project::TestPanelSizing {
                units: Default::default(),
                size,
                edge_rails: edge_rails.clone(),
                fiducials: vec![],
                design_sizings: design_sizings.clone(),
                pcb_unit_positionings: pcb_unit_positions.clone(),
            },
            orientation: PcbAssemblyOrientation::default(),
        }
        .content();

        // and
        let edge_rails_arg = format!(
            "--edge-rails left={},right={},top={},bottom={}",
            edge_rails.left, edge_rails.right, edge_rails.top, edge_rails.bottom
        );
        let size_arg = format!("--size x={},y={}", size.x, size.y);

        let design_sizings_args = design_sizings
            .iter()
            .zip(design_names)
            .map(|(design_sizing, design_name)| {
                format!(
                    "--design-sizing {}:origin=x={},y={}:g_offset=x={},y={}:p_offset=x={},y={}:size=x={},y={}",
                    design_name,
                    design_sizing.origin.x,
                    design_sizing.origin.y,
                    design_sizing.gerber_offset.x,
                    design_sizing.gerber_offset.y,
                    design_sizing.placement_offset.x,
                    design_sizing.placement_offset.y,
                    design_sizing.size.x,
                    design_sizing.size.y,
                )
            })
            .collect::<Vec<_>>();

        let pcb_unit_position_args = pcb_unit_positions
            .iter()
            .enumerate()
            .map(|(index, pcb_unit_position)| {
                format!(
                    "--pcb-unit-position {}:offset=x={},y={}:rotation={}",
                    index + 1,
                    pcb_unit_position.offset.x,
                    pcb_unit_position.offset.y,
                    pcb_unit_position.rotation,
                )
            })
            .collect::<Vec<_>>();

        let mut unprepared_args = vec![
            ctx.trace_log_arg.as_str(),
            "pcb",
            ctx.test_pcb_1_arg.as_str(),
            "-vvv",
            "configure-panel-sizing",
            edge_rails_arg.as_str(),
            size_arg.as_str(),
        ];
        unprepared_args.extend(
            design_sizings_args
                .iter()
                .map(|arg| arg.as_str()),
        );
        unprepared_args.extend(
            pcb_unit_position_args
                .iter()
                .map(|arg| arg.as_str()),
        );

        let args = prepare_args(unprepared_args);

        println!("args: {:?}", args);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and

        let saving_pcb_message = &format!("Saved PCB. path: {:?}\n", ctx.test_pcb_1_path);

        assert_contains_inorder!(trace_content, [saving_pcb_message,]);

        // and
        let pcb_1_content: String = read_and_show_file(&ctx.test_pcb_1_path)?;

        assert_eq!(pcb_1_content, expected_pcb_1_content);

        Ok(())
    }

    #[test]
    fn sequence_04_add_pcb() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(4);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::default(),
            }])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "-vvv",
            "add-pcb",
            "--file relative=panel_a.pcb.json",
        ]);
        println!("args: {:?}", args);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "Added PCB to project. pcb_file: relative='panel_a.pcb.json'\n",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_05_assign_variant_to_unit() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(5);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R3","RES_MFR1","RES1","true","Top","10","15","90"
            "C1","CAP_MFR1","CAP1","true","Bottom","40","45","180"
            "J1","CONN_MFR1","CONN1","true","Bottom","30","35","-90"
            "R1","RES_MFR1","RES1","true","Top","20","25","0"
        "#};
        // two refdes on the same side should use the same part (R1, R3)
        // all placements should be within the configured design size (design_a = 60x50) + export offset (10,10)

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        println!("creating placements, path: {:?}", &placements_path);

        let mut placements_file = File::create(placements_path)?;
        let _written = placements_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placements_file.flush()?;

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (project::TestPart::new("CAP_MFR1", "CAP1"), TestPartState::default()),
                (project::TestPart::new("CONN_MFR1", "CONN1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(30),
                            dec!(35),
                            dec!(-90),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(84), dec!(-90)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(25),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(22), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(10),
                            dec!(15),
                            dec!(90),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(12), dec!(90)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-variant-to-unit",
            "--variant variant_a",
            "--unit pcb=1::unit=1",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            // `STORE-10`, the project, then its PCB, read by the stores service.
            "planner_app: Load project. path: ",
            "planner_app: Loading PCB. path: ",
            "panel_a.pcb.json",
            "Unit assignment added. unit: 'pcb=1::unit=1', variant_name: variant_a\n",
            "New part. part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }\n",
            "New part. part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }\n",
            "New part. part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }\n",
            "New placement. placement: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 10, y: 15, rotation: 90 }",
            "New placement. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom, x: 40, y: 45, rotation: 180 }",
            "New placement. placement: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Bottom, x: 30, y: 35, rotation: -90 }",
            "New placement. placement: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 20, y: 25, rotation: 0 }",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_06_refresh_from_design_variants() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(6);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R3","RES_MFR1","RES1","true","Top","40","40","135"
            "R2","RES_MFR2","RES2","true","Top","30","30","45"
            "J1","CONN_MFR1","CONN1","true","Bottom","10","10","0"
            "R1","RES_MFR1","RES1","true","Top","20","20","-45"
        "#};
        // R2 added (should be before R1). C1 deleted, all coordinates and rotations are changed.

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        let mut placments_file = File::create(placements_path)?;
        let _written = placments_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placments_file.flush()?;

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (project::TestPart::new("CONN_MFR1", "CONN1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::default()),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "refresh-from-design-variants",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "New part. part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }\n",
            "Removing unused part. part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }\n",
            "Updating placement. old: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 10, y: 15, rotation: 90 }, new: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 40, y: 40, rotation: 135 }",
            "New placement. placement: Placement { ref_des: \"R2\", part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }, place: true, pcb_side: Top, x: 30, y: 30, rotation: 45 }",
            "Updating placement. old: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Bottom, x: 30, y: 35, rotation: -90 }, new: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Bottom, x: 10, y: 10, rotation: 0 }",
            "Updating placement. old: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 20, y: 25, rotation: 0 }, new: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 20, y: 20, rotation: -45 }",
            "Marking placement as unused. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom, x: 40, y: 45, rotation: 180 }",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    /// Runs the CLI with the given arguments, and returns its trace.  Fails if the CLI fails.
    fn run_cli(ctx: &context::Context, args: Vec<&str>) -> Result<String, anyhow::Error> {
        let (trace_content, cmd_assert) = run_cli_inner(ctx, args)?;
        cmd_assert.success();
        Ok(trace_content)
    }

    fn run_cli_inner(
        ctx: &context::Context,
        args: Vec<&str>,
    ) -> Result<(String, assert_cmd::assert::Assert), anyhow::Error> {
        ctx.delete_trace_log();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));
        let args = prepare_args(args);
        println!("args: {:?}", args);

        let cmd_assert = cmd
            .args(args)
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        Ok((trace_content, cmd_assert))
    }

    /// `PROCDEF-1`, `PROCDEF-4`
    #[test]
    fn sequence_07_create_process_from_preset() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(7);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let expected_process_content = TestProcessDefinition::pnp().content();

        // when
        let trace_content = run_cli(ctx, vec![
            ctx.trace_log_arg.as_str(),
            "process",
            ctx.pnp_process_arg.as_str(),
            "-vvv",
            "create",
            "--reference pnp",
            "--preset pnp",
        ])?;

        // then
        assert_contains_inorder!(trace_content, ["Create process.", "Saving process.",]);

        // and
        let process_content: String = read_and_show_file(&ctx.pnp_process_path)?;
        assert_eq!(process_content, expected_process_content);

        Ok(())
    }

    /// `PROCDEF-1`, `PROCDEF-2`, `PROCDEF-3`, `LOG-1`, `LOG-2`
    #[test]
    fn sequence_08_define_process() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(8);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and the same process as the 'manual' preset, defined one change at a time.
        let expected_process_content = TestProcessDefinition::manual().content();

        let commands = [
            "create --reference hand",
            "set-reference --reference manual",
            "add-operation --operation load_pcbs",
            "add-task --operation load_pcbs --task load_pcbs --kind load_pcbs",
            "add-operation --operation manually_solder_components",
            "add-task --operation manually_solder_components --task place_components --kind place_components",
            "add-task --operation manually_solder_components --task manual_soldering --kind generic",
        ];

        // when
        for command in commands {
            run_cli(ctx, vec![
                ctx.trace_log_arg.as_str(),
                "process",
                ctx.manual_process_arg.as_str(),
                "-vvv",
                command,
            ])?;
        }

        // then
        let process_content: String = read_and_show_file(&ctx.manual_process_path)?;
        assert_eq!(process_content, expected_process_content);

        // and when a second task that places components is added
        let (trace_content, cmd_assert) = run_cli_inner(ctx, vec![
            ctx.trace_log_arg.as_str(),
            "process",
            ctx.manual_process_arg.as_str(),
            "-vvv",
            "add-task --operation load_pcbs --task place_again --kind place_components",
        ])?;

        // then it fails, and the process is unchanged, and the logs show the event and why it failed, in order (`LOG-1`, `LOG-2`)
        cmd_assert.failure();
        assert_contains_inorder!(trace_content, [
            "planner_app: Received event. event: EditProcess {",
            "edit: AddTask { operation: Reference(\"load_pcbs\"), task: Reference(\"place_again\"), kind: PlaceComponents }",
            "planner_app: Edit process.",
            "planner_app: Event failed. error: ProcessError(InvalidProcess(MoreThanOnePlaceComponentsTask { process: Reference(\"manual\") }))",
        ]);
        let process_content: String = read_and_show_file(&ctx.manual_process_path)?;
        assert_eq!(process_content, expected_process_content);

        Ok(())
    }

    /// `PROCDEF-5`, `PROCDEF-6`, `LOG-1`, `LOG-2`
    #[test]
    fn sequence_09_add_processes() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(9);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (project::TestPart::new("CONN_MFR1", "CONN1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::default()),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // when
        for file_arg in [
            "--file relative=pnp.process.json",
            "--file relative=manual.process.json",
        ] {
            let trace_content = run_cli(ctx, vec![
                ctx.trace_log_arg.as_str(),
                "project",
                ctx.path_arg.as_str(),
                ctx.project_arg.as_str(),
                "-vvv",
                "add-process",
                file_arg,
            ])?;

            // then
            assert_contains_inorder!(trace_content, ["Added process.",]);
        }

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;
        assert_eq!(project_content, expected_project_content);

        // and when the same process is added again
        let (trace_content, cmd_assert) = run_cli_inner(ctx, vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "-vvv",
            "add-process",
            "--file relative=pnp.process.json",
        ])?;

        // then it fails, and the project is unchanged
        cmd_assert.failure();
        // and the logs show the event and why it failed, in order (`LOG-1`, `LOG-2`)
        assert_contains_inorder!(trace_content, [
            "planner_app: Received event. event: AddProcess { process_file: Relative(\"pnp.process.json\") }",
            "planner_app: Event failed. error: ProcessError(DuplicateProcessFile { process_file: Relative(\"pnp.process.json\") })",
        ]);
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;
        assert_eq!(project_content, expected_project_content);

        // and when the processes are refreshed, nothing has changed
        let trace_content = run_cli(ctx, vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "-vvv",
            "refresh-processes",
        ])?;
        assert_contains_inorder!(trace_content, ["Processes unchanged.",]);
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;
        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_10_assign_process_to_parts() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(10);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::default()),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-process-to-parts",
            "--process manual",
            "--operation add",
            "--manufacturer CONN_MFR.*",
            "--mpn .*",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "Added process. part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, applicable_processes: [\"manual\"]",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_11_create_phase_top() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(11);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::default()),
            ])
            .with_phases(vec![TestPhase::new(
                "top_1",
                "pnp",
                ctx.phase_1_load_out_path
                    .to_str()
                    .unwrap(),
                PcbSide::Top,
                &[],
            )])
            .with_phase_orderings(&["top_1"])
            .with_phase_states(vec![("top_1", vec![
                TestOperationState::new("load_pcbs", vec![(
                    "load_pcbs",
                    Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                )]),
                TestOperationState::new("automated_pnp", vec![(
                    "place_components",
                    Box::new(TestPlacementTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                )]),
                TestOperationState::new("reflow_oven_soldering", vec![(
                    "automated_soldering",
                    Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                )]),
            ])])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let phase_1_load_out_arg = format!(
            "--load-out {}",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "create-phase",
            "--reference top_1",
            "--process pnp",
            &phase_1_load_out_arg,
            "--pcb-side top",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let load_out_creation_message = format!(
            "Created load-out. source: '{}'",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );

        assert_contains_inorder!(trace_content, [
            &load_out_creation_message,
            "Created phase. reference: 'top_1', process: pnp",
            "Phase ordering: ['top_1']\n",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_12_create_phase_bottom() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(12);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::default()),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::default()),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "manual_soldering",
                            Box::new(TestGenericTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "automated_soldering",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
            ])
            .content();

        // and
        let phase_2_load_out_arg = format!(
            "--load-out {}",
            ctx.phase_2_load_out_path
                .to_str()
                .unwrap()
        );

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "create-phase",
            "--reference bottom_1",
            "--process manual",
            &phase_2_load_out_arg,
            "--pcb-side bottom",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let load_out_creation_message = format!(
            "Created load-out. source: '{}'",
            ctx.phase_2_load_out_path
                .to_str()
                .unwrap()
        );

        assert_contains_inorder!(trace_content, [
            &load_out_creation_message,
            "Created phase. reference: 'bottom_1', process: manual",
            "Phase ordering: ['top_1', 'bottom_1']\n",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_13_assign_placements_to_phase() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(13);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "manual_soldering",
                            Box::new(TestGenericTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Pending).with_total(3))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "automated_soldering",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "-vv",
            "assign-placements-to-phase",
            "--phase top_1",
            "--operation set",
            // By placement path pattern
            //"--placements pcb=1::unit=1::ref_des=R1"
            "--placements pcb=1::unit=1::ref_des=R.*",
            //"--placements pcb=1::unit=1::ref_des=J1",
            //"--placements pcb=panel::instance=.*::unit=.*::ref_des=R1"
            //"--placements pcb=1::unit=.*::ref_des=.*"
            //"--placements .*::ref_des=R.*"
            //"--placements .*",

            // FUTURE By manufacturer and mpn
            // "--manufacturer RES_MFR.*",
            // "--mpn .*"
        ]);

        // and
        let expected_phase_1_load_out_content = LoadOutCSVBuilder::new()
            .with_items(&[
                TestLoadOutRecord {
                    reference: "".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                },
                TestLoadOutRecord {
                    reference: "".to_string(),
                    manufacturer: "RES_MFR2".to_string(),
                    mpn: "RES2".to_string(),
                },
            ])
            .as_string();

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let loading_load_out_message = format!(
            "Loading load-out. source: '{}'",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );
        let storing_load_out_message = format!(
            "Storing load-out. source: '{}'",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );

        assert_contains_inorder!(trace_content, [
            // the load-out should be read first, before the project is changed (`STORE-11`)
            &loading_load_out_message,
            // assignments should be made
            "Assigning placement to phase. phase: top_1, placement_path: pcb=1::unit=1::ref_des=R1",
            "Assigning placement to phase. phase: top_1, placement_path: pcb=1::unit=1::ref_des=R2",
            "Assigning placement to phase. phase: top_1, placement_path: pcb=1::unit=1::ref_des=R3",
            // all phase status should be updated
            "Refreshed placement task state. phase: bottom_1, operation: manually_solder_components, task: place_components, status: Pending, updated: false\n",
            "Refreshed placement task state. phase: top_1, operation: automated_pnp, task: place_components, status: Pending, updated: true\n",
            // part process should be updated
            "Added process. part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, applicable_processes: [\"pnp\"]",
            "Added process. part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }, applicable_processes: [\"pnp\"]",
            // load-out should be checked
            r#"Checking for part in load_out. part: Part { manufacturer: "RES_MFR1", mpn: "RES1" }"#,
            r#"Checking for part in load_out. part: Part { manufacturer: "RES_MFR2", mpn: "RES2" }"#,
            &storing_load_out_message,
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        // and
        let phase_1_load_out_content: String = read_to_string(ctx.phase_1_load_out_path.clone())?;
        println!("actual:\n{}", phase_1_load_out_content);
        println!("expected:\n{}", expected_phase_1_load_out_content);

        assert_eq!(phase_1_load_out_content, expected_phase_1_load_out_content);

        Ok(())
    }

    //noinspection MissingFeatures
    #[test]
    fn sequence_14_assign_feeder_to_load_out_item() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(14);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-feeder-to-load-out-item",
            "--phase top_1",
            "--feeder-reference FEEDER_1",
            "--manufacturer .*",
            "--mpn RES1",
        ]);

        let expected_phase_1_load_out_content = LoadOutCSVBuilder::new()
            .with_items(&[
                TestLoadOutRecord {
                    reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                },
                TestLoadOutRecord {
                    reference: "".to_string(),
                    manufacturer: "RES_MFR2".to_string(),
                    mpn: "RES2".to_string(),
                },
            ])
            .as_string();

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            r#"Assigned feeder to load-out item. feeder: FEEDER_1, part: Part { manufacturer: "RES_MFR1", mpn: "RES1" }"#,
        ]);

        // and
        let phase_1_load_out_content: String = read_to_string(ctx.phase_1_load_out_path.clone())?;
        println!("actual:\n{}", phase_1_load_out_content);
        println!("expected:\n{}", expected_phase_1_load_out_content);

        assert_eq!(phase_1_load_out_content, expected_phase_1_load_out_content);

        Ok(())
    }

    #[test]
    fn sequence_15_set_placement_ordering() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(15);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[
                        ("Pcb", "Asc"),
                        ("PcbUnit", "Asc"),
                        ("Area", "Asc"),
                        ("Height", "Asc"),
                        ("FeederReference", "Asc"),
                        ("RefDes", "Desc"),
                    ],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "manual_soldering",
                            Box::new(TestGenericTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Pending).with_total(3))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "automated_soldering",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "set-placement-ordering",
            "--phase top_1",
            "--placement-orderings PCB:ASC,PCB_UNIT:ASC,AREA:ASC,HEIGHT:ASC,FEEDER_REFERENCE:ASC,REF_DES:DESC",
            // example for PnP machine placement
            //"--orderings PCB_UNIT:ASC,COST:ASC,AREA:ASC,HEIGHT;ASC,FEEDER_REFERENCE:ASC",
            // example for manual placement
            //"--orderings COST:ASC,AREA:ASC,HEIGHT;ASC,PART:ASC,PCB_UNIT:ASC",
        ]);

        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "Phase placement orderings set. phase: 'top_1', orderings: [PCB:ASC, PCB_UNIT:ASC, AREA:ASC, HEIGHT:ASC, FEEDER_REFERENCE:ASC, REF_DES:DESC]",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_16_generate_artifacts() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(16);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        let packages_content = PackagesCSVBuilder::new()
            .with_items(&[
                TestPackageRecord {
                    name: "RES_0603_PKG1".to_string(),
                    size_x: dec!(1.6),
                    size_y: dec!(0.85),
                    size_z: dec!(0.45),
                },
                TestPackageRecord {
                    name: "RES_0805_PKG1".to_string(),
                    size_x: dec!(2.0),
                    size_y: dec!(1.25),
                    size_z: dec!(0.5),
                },
            ])
            .as_string();

        let mut packages_file = File::create(ctx.packages_path.clone())?;
        let _written = packages_file.write(packages_content.as_bytes())?;
        packages_file.flush()?;

        let package_mappings_content = PackageMappingsCSVBuilder::new()
            .with_items(&[
                TestPackageMappingRecord {
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                    name: "RES_0603_PKG1".to_string(),
                },
                TestPackageMappingRecord {
                    manufacturer: "RES_MFR2".to_string(),
                    mpn: "RES2".to_string(),
                    name: "RES_0805_PKG1".to_string(),
                },
            ])
            .as_string();

        let mut package_mappings_file = File::create(ctx.package_mappings_path.clone())?;
        let _written = package_mappings_file.write(package_mappings_content.as_bytes())?;
        package_mappings_file.flush()?;

        // and
        let expected_phase_1_placements_content = PhasePlacementsCSVBuilder::new()
            .with_items(&[
                // these 2 should map to the smallest and shortest, package
                TestPhasePlacementRecord {
                    object_path: "pcb=1::unit=1::ref_des=R3".to_string(),
                    feeder_reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                    x: dec!(42),
                    y: dec!(37),
                    rotation: dec!(135),
                },
                TestPhasePlacementRecord {
                    object_path: "pcb=1::unit=1::ref_des=R1".to_string(),
                    feeder_reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                    x: dec!(22),
                    y: dec!(17),
                    rotation: dec!(-45),
                },
                // should map to a package with a larger area, that is also taller
                TestPhasePlacementRecord {
                    object_path: "pcb=1::unit=1::ref_des=R2".to_string(),
                    feeder_reference: "".to_string(),
                    manufacturer: "RES_MFR2".to_string(),
                    mpn: "RES2".to_string(),
                    x: dec!(32),
                    y: dec!(27),
                    rotation: dec!(45),
                },
            ])
            .as_string();

        let expected_project_report_content = ProjectReportBuilder::default()
            .with_name("job1")
            .with_status("Incomplete")
            .with_phases_overview(&[
                TestPhaseOverview {
                    phase: "top_1".to_string(),
                    status: "Pending".to_string(),
                    process: "pnp".to_string(),
                    operations_overview: vec![
                        TestPhaseOperationOverview {
                            operation: "load_pcbs".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![(
                                "load_pcbs".to_string(),
                                Box::new(TestLoadPcbsTaskOverview {}) as Box<dyn TestTaskOverview>,
                            )],
                        },
                        TestPhaseOperationOverview {
                            operation: "automated_pnp".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![(
                                "place_components".to_string(),
                                Box::new(TestPlaceComponentsTaskOverview {
                                    placed: 0,
                                    skipped: 0,
                                    total: 3,
                                }) as Box<dyn TestTaskOverview>,
                            )],
                        },
                        TestPhaseOperationOverview {
                            operation: "reflow_oven_soldering".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![(
                                "automated_soldering".to_string(),
                                Box::new(TestGenericTaskOverview {}) as Box<dyn TestTaskOverview>,
                            )],
                        },
                    ],
                },
                TestPhaseOverview {
                    phase: "bottom_1".to_string(),
                    status: "Pending".to_string(),
                    process: "manual".to_string(),
                    operations_overview: vec![
                        TestPhaseOperationOverview {
                            operation: "load_pcbs".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![(
                                "load_pcbs".to_string(),
                                Box::new(TestLoadPcbsTaskOverview {}) as Box<dyn TestTaskOverview>,
                            )],
                        },
                        TestPhaseOperationOverview {
                            operation: "manually_solder_components".to_string(),
                            status: TestProcessOperationStatus::Pending,
                            tasks: vec![
                                (
                                    "place_components".to_string(),
                                    Box::new(TestPlaceComponentsTaskOverview {
                                        placed: 0,
                                        skipped: 0,
                                        total: 0,
                                    }) as Box<dyn TestTaskOverview>,
                                ),
                                (
                                    "manual_soldering".to_string(),
                                    Box::new(TestGenericTaskOverview {}) as Box<dyn TestTaskOverview>,
                                ),
                            ],
                        },
                    ],
                },
            ])
            .with_phase_specification(&[
                TestPhaseSpecification {
                    phase: "top_1".to_string(),
                    operations: vec![
                        TestPhaseOperation {
                            operation: "load_pcbs".to_string(),
                            task_specifications: vec![(
                                "load_pcbs".to_string(),
                                Box::new(TestLoadPcbsTaskSpecification {
                                    pcbs: vec![report::TestPcb {
                                        name: "panel_a".to_string(),
                                        unit_assignments: vec![TestPcbUnitAssignment {
                                            unit_path: "pcb=1::unit=1".to_string(),
                                            design_name: "design_a".to_string(),
                                            variant_name: "variant_a".to_string(),
                                        }],
                                    }],
                                }) as Box<dyn TestTaskSpecification>,
                            )],
                        },
                        TestPhaseOperation {
                            operation: "automated_pnp".to_string(),
                            task_specifications: vec![(
                                "place_components".to_string(),
                                Box::new(TestPlaceComponentsTaskSpecification {}) as Box<dyn TestTaskSpecification>,
                            )],
                        },
                        TestPhaseOperation {
                            operation: "reflow_oven_soldering".to_string(),
                            task_specifications: vec![(
                                "automated_soldering".to_string(),
                                Box::new(TestGenericTaskSpecification {}) as Box<dyn TestTaskSpecification>,
                            )],
                        },
                    ],
                    load_out_assignments: vec![
                        TestPhaseLoadOutAssignmentItem {
                            feeder_reference: Some("FEEDER_1".to_string()),
                            manufacturer: "RES_MFR1".to_string(),
                            mpn: "RES1".to_string(),
                            quantity: 2, // R1 and R3
                        },
                        TestPhaseLoadOutAssignmentItem {
                            feeder_reference: None,
                            manufacturer: "RES_MFR2".to_string(),
                            mpn: "RES2".to_string(),
                            quantity: 1,
                        },
                    ],
                },
                TestPhaseSpecification {
                    phase: "bottom_1".to_string(),
                    operations: vec![
                        TestPhaseOperation {
                            operation: "load_pcbs".to_string(),
                            task_specifications: vec![(
                                "load_pcbs".to_string(),
                                Box::new(TestLoadPcbsTaskSpecification {
                                    pcbs: vec![report::TestPcb {
                                        name: "panel_a".to_string(),
                                        unit_assignments: vec![TestPcbUnitAssignment {
                                            unit_path: "pcb=1::unit=1".to_string(),
                                            design_name: "design_a".to_string(),
                                            variant_name: "variant_a".to_string(),
                                        }],
                                    }],
                                }) as Box<dyn TestTaskSpecification>,
                            )],
                        },
                        TestPhaseOperation {
                            operation: "manually_solder_components".to_string(),
                            task_specifications: vec![
                                (
                                    "place_components".to_string(),
                                    Box::new(TestPlaceComponentsTaskSpecification {}) as Box<dyn TestTaskSpecification>,
                                ),
                                (
                                    "manual_soldering".to_string(),
                                    Box::new(TestGenericTaskSpecification {}) as Box<dyn TestTaskSpecification>,
                                ),
                            ],
                        },
                    ],
                    load_out_assignments: vec![],
                },
            ])
            .with_issues(&[
                TestIssue {
                    message: "Phase with no placements.".to_string(),
                    severity: TestIssueSeverity::Warning,
                    kind: TestIssueKind::PhaseWithNoPlacements {
                        phase: "bottom_1".to_string(),
                    },
                },
                TestIssue {
                    message: "A placement has not been assigned to a phase".to_string(),
                    severity: TestIssueSeverity::Warning,
                    kind: TestIssueKind::UnassignedPlacement {
                        object_path: "pcb=1::unit=1::ref_des=J1".to_string(),
                    },
                },
                TestIssue {
                    message: "A part has not been assigned to a feeder".to_string(),
                    severity: TestIssueSeverity::Warning,
                    kind: TestIssueKind::UnassignedPartFeeder {
                        phase: "top_1".to_string(),
                        part: TestPart {
                            manufacturer: "RES_MFR2".to_string(),
                            mpn: "RES2".to_string(),
                        },
                    },
                },
            ])
            .as_string();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "generate-artifacts",
            "-vv",
        ]);
        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let load_out_message_1 = format!(
            "Loading load-out. source: '{}'",
            ctx.phase_1_load_out_path
                .to_str()
                .unwrap()
        );

        assert_eq!(
            trace_content
                .clone()
                .split('\n')
                .collect::<Vec<&str>>()
                .iter()
                .fold(0, |mut count, &line| {
                    if line.contains(&load_out_message_1) {
                        count += 1;
                    }
                    count
                }),
            1
        );

        // and
        let mut phase_1_placements_file_path = PathBuf::from(ctx.temp_dir.path());
        phase_1_placements_file_path.push("top_1_placements.csv");
        let phase_1_message = format!(
            "Generated phase placements. phase: 'top_1', path: {:?}\n",
            phase_1_placements_file_path
        );

        let mut phase_2_placements_file_path = PathBuf::from(ctx.temp_dir.path());
        phase_2_placements_file_path.push("bottom_1_placements.csv");
        let phase_2_message = format!(
            "Generated phase placements. phase: 'bottom_1', path: {:?}\n",
            phase_2_placements_file_path
        );

        let mut json_project_report_file_path = PathBuf::from(ctx.temp_dir.path());
        json_project_report_file_path.push("job1_report.json");
        let json_report_message = format!("Generated JSON report. path: {:?}\n", json_project_report_file_path);

        let mut markdown_project_report_file_path = PathBuf::from(ctx.temp_dir.path());
        markdown_project_report_file_path.push("job1_report.md");
        let markdown_report_message = format!(
            "Generated Markdown report. path: {:?}\n",
            markdown_project_report_file_path
        );

        assert_contains_inorder!(trace_content, [
            &phase_1_message,
            &phase_2_message,
            &json_report_message,
            &markdown_report_message,
            "Generated artifacts.\n",
        ]);

        // and
        let phase_1_placements_content: String = read_to_string(phase_1_placements_file_path)?;
        println!("{}", phase_1_placements_content);

        assert_eq!(phase_1_placements_content, expected_phase_1_placements_content);

        // and
        println!("expected:\n{}", expected_project_report_content);

        let project_report_content: String = read_to_string(json_project_report_file_path)?;
        println!("actual:\n{}", project_report_content);

        // content of markdown report file is not asserted because:
        // a) it's generated by an external crate from the json version.
        // b) it's currently not known which is the best external crate.
        // c) it's not known if it's a long term feature, hence the feature gate.

        assert_eq!(project_report_content, expected_project_report_content);

        Ok(())
    }

    #[test]
    fn sequence_17_record_phase_operations() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(17);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd_1 = Command::new(env!("CARGO_BIN_EXE_planner_cli"));
        let mut cmd_2 = Command::new(env!("CARGO_BIN_EXE_planner_cli"));
        let mut cmd_3 = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[
                        ("Pcb", "Asc"),
                        ("PcbUnit", "Asc"),
                        ("Area", "Asc"),
                        ("Height", "Asc"),
                        ("FeederReference", "Asc"),
                        ("RefDes", "Desc"),
                    ],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "manual_soldering",
                            Box::new(TestGenericTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Complete)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Started).with_total(3))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "automated_soldering",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and
        let operation_expectations = vec![
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "load_pcbs".to_string(),
                    Box::new(TestTaskOperationHistoryKind {
                        kind: TaskKind::LoadPcbs,
                        status: TaskStatus::Started,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "load_pcbs".to_string(),
                    Box::new(TestTaskOperationHistoryKind {
                        kind: TaskKind::LoadPcbs,
                        status: TaskStatus::Complete,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "automated_pnp".to_string(),
                    Box::new(TestTaskOperationHistoryKind {
                        kind: TaskKind::PlaceComponents,
                        status: TaskStatus::Started,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            ("eof", None),
        ];

        // and
        let args_1 = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "record-phase-operation",
            "--phase top_1",
            "--operation load_pcbs",
            "--task load_pcbs",
            "--action start",
        ]);
        let message_1 = "Marking task as started. phase: top_1, operation: load_pcbs, task: load_pcbs";

        let args_2 = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "record-phase-operation",
            "--phase top_1",
            "--operation load_pcbs",
            "--task load_pcbs",
            "--action complete",
        ]);
        let message_2 = "Marking task as completed. phase: top_1, operation: load_pcbs, task: load_pcbs";

        let args_3 = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "record-phase-operation",
            "--phase top_1",
            "--operation automated_pnp",
            "--task place_components",
            "--action start",
        ]);
        let message_3 = "Marking task as started. phase: top_1, operation: automated_pnp, task: place_components";

        // when
        cmd_1
            .args(args_1)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and when
        cmd_2
            .args(args_2)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));
        // and when
        cmd_3
            .args(args_3)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("trace:\n{}", trace_content);

        let initial_log_file_message = format!("Created operation history file. path: {:?}\n", ctx.phase_1_log_path);
        let log_file_message = format!("Updated operation history file. path: {:?}\n", ctx.phase_1_log_path);

        assert_contains_inorder!(trace_content, [
            message_1,
            &initial_log_file_message,
            message_2,
            &log_file_message,
            message_3,
            &log_file_message,
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        // and
        let operation_history_file = File::open(ctx.phase_1_log_path.clone())?;
        let operation_history: Vec<TestOperationHistoryItem> = serde_json::from_reader(operation_history_file)?;

        assert_operation_history(operation_history, operation_expectations);

        Ok(())
    }

    #[test]
    fn sequence_18_record_placements_operation() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(18);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[
                        ("Pcb", "Asc"),
                        ("PcbUnit", "Asc"),
                        ("Area", "Asc"),
                        ("Height", "Asc"),
                        ("FeederReference", "Asc"),
                        ("RefDes", "Desc"),
                    ],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "manual_soldering",
                            Box::new(TestGenericTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Complete)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "place_components",
                        Box::new(
                            TestPlacementTaskState::new(TaskStatus::Complete)
                                .with_placed(3)
                                .with_total(3),
                        ) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "automated_soldering",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Placed,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Placed,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Placed,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and

        let operation_expectations = vec![
            ("ignore", None),
            ("ignore", None),
            ("ignore", None),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "automated_pnp".to_string(),
                    Box::new(TestPlacementOperationHistoryKind {
                        object_path: ObjectPath::from_str("pcb=1::unit=1::ref_des=R1").unwrap(),
                        operation: PlacementOperation::Place,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "automated_pnp".to_string(),
                    Box::new(TestPlacementOperationHistoryKind {
                        object_path: ObjectPath::from_str("pcb=1::unit=1::ref_des=R2").unwrap(),
                        operation: PlacementOperation::Place,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            (
                "require",
                Some((
                    "top_1".to_string(),
                    "automated_pnp".to_string(),
                    Box::new(TestPlacementOperationHistoryKind {
                        object_path: ObjectPath::from_str("pcb=1::unit=1::ref_des=R3").unwrap(),
                        operation: PlacementOperation::Place,
                    }) as Box<dyn TestOperationHistoryKind>,
                )),
            ),
            ("eof", None),
        ];

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "record-placements-operation",
            "--object-path-patterns pcb=1::unit=1::ref_des=R([1-3])?,pcb=1::unit=2::ref_des=.*",
            "--operation placed",
        ]);
        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        let log_file_message = format!("Updated operation history file. path: {:?}\n", ctx.phase_1_log_path);

        assert_contains_inorder!(trace_content, [
            "Placement marked as placed. object_path: pcb=1::unit=1::ref_des=R1\n",
            "Placement marked as placed. object_path: pcb=1::unit=1::ref_des=R2\n",
            "Placement marked as placed. object_path: pcb=1::unit=1::ref_des=R3\n",
            "Unmatched object path pattern. object_path_pattern: pcb=1::unit=2::ref_des=.*\n",
            "Refreshed placement task state. phase: bottom_1, operation: manually_solder_components, task: place_components, status: Pending, updated: false\n",
            "Refreshed placement task state. phase: top_1, operation: automated_pnp, task: place_components, status: Complete, updated: true\n",
            &log_file_message,
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        // and
        let operation_history_file = File::open(ctx.phase_1_log_path.clone())?;
        let operation_history: Vec<TestOperationHistoryItem> = serde_json::from_reader(operation_history_file)?;
        println!("{:?}", operation_history);

        assert_operation_history(operation_history, operation_expectations);

        Ok(())
    }

    #[test]
    fn sequence_19_reset_operations() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::acquire(19);
        let ctx = ctx_guard.1.as_mut().unwrap();
        ctx.delete_trace_log();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_project_content = TestProject::new()
            .with_name("job1")
            .with_library_config(&ctx.library_config)
            .with_processes()
            .with_pcbs(vec![project::TestProjectPcb {
                pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
                unit_assignments: BTreeMap::from_iter([(0, DesignVariant {
                    design_name: "design_a".into(),
                    variant_name: "variant_a".into(),
                })]),
            }])
            .with_part_states(vec![
                (
                    project::TestPart::new("CONN_MFR1", "CONN1"),
                    TestPartState::new(&["manual"]),
                ),
                (project::TestPart::new("RES_MFR1", "RES1"), TestPartState::new(&["pnp"])),
                (project::TestPart::new("RES_MFR2", "RES2"), TestPartState::new(&["pnp"])),
            ])
            .with_phases(vec![
                TestPhase::new(
                    "bottom_1",
                    "manual",
                    ctx.phase_2_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Bottom,
                    &[],
                ),
                TestPhase::new(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path
                        .to_str()
                        .unwrap(),
                    PcbSide::Top,
                    &[
                        ("Pcb", "Asc"),
                        ("PcbUnit", "Asc"),
                        ("Area", "Asc"),
                        ("Height", "Asc"),
                        ("FeederReference", "Asc"),
                        ("RefDes", "Desc"),
                    ],
                ),
            ])
            .with_phase_orderings(&["top_1", "bottom_1"])
            .with_phase_states(vec![
                ("bottom_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("manually_solder_components", vec![
                        (
                            "place_components",
                            Box::new(TestPlacementTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                        (
                            "manual_soldering",
                            Box::new(TestGenericTaskState::new(TaskStatus::Pending))
                                as Box<dyn TestSerializableTaskState>,
                        ),
                    ]),
                ]),
                ("top_1", vec![
                    TestOperationState::new("load_pcbs", vec![(
                        "load_pcbs",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("automated_pnp", vec![(
                        "place_components",
                        Box::new(TestPlacementTaskState::new(TaskStatus::Pending).with_total(3))
                            as Box<dyn TestSerializableTaskState>,
                    )]),
                    TestOperationState::new("reflow_oven_soldering", vec![(
                        "automated_soldering",
                        Box::new(TestGenericTaskState::new(TaskStatus::Pending)) as Box<dyn TestSerializableTaskState>,
                    )]),
                ]),
            ])
            .with_placements(vec![
                (
                    "pcb=1::unit=1::ref_des=C1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "C1",
                            "CAP_MFR1",
                            "CAP1",
                            true,
                            PcbSide::Bottom,
                            dec!(40),
                            dec!(45),
                            dec!(180),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(74), dec!(0)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Unused,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=J1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "J1",
                            "CONN_MFR1",
                            "CONN1",
                            true,
                            PcbSide::Bottom,
                            dec!(10),
                            dec!(10),
                            dec!(0),
                        ),
                        TestUnitPosition::new(dec!(12), dec!(109), dec!(180)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        None,
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R1",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R1",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(20),
                            dec!(20),
                            dec!(-45),
                        ),
                        TestUnitPosition::new(dec!(22), dec!(17), dec!(-45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R2",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R2",
                            "RES_MFR2",
                            "RES2",
                            true,
                            PcbSide::Top,
                            dec!(30),
                            dec!(30),
                            dec!(45),
                        ),
                        TestUnitPosition::new(dec!(32), dec!(27), dec!(45)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
                (
                    "pcb=1::unit=1::ref_des=R3",
                    TestPlacementState::new(
                        "pcb=1::unit=1",
                        TestPlacement::new(
                            "R3",
                            "RES_MFR1",
                            "RES1",
                            true,
                            PcbSide::Top,
                            dec!(40),
                            dec!(40),
                            dec!(135),
                        ),
                        TestUnitPosition::new(dec!(42), dec!(37), dec!(135)),
                        PlacementStatus::Pending,
                        ProjectPlacementStatus::Used,
                        Some("top_1"),
                    ),
                ),
            ])
            .content();

        // and
        let args = prepare_args(vec![
            ctx.trace_log_arg.as_str(),
            "project",
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "reset-operations",
        ]);
        // when
        let cmd_assert = cmd
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_and_show_file(&ctx.test_trace_log_path)?;

        // and assert the command *after* the trace output has been displayed.
        cmd_assert.success();

        // and
        assert_contains_inorder!(trace_content, [
            "Placement operations reset.\n",
            "Phase operations reset. phase: bottom_1\n",
            "Phase operations reset. phase: top_1\n",
        ]);

        // and
        let project_content: String = read_and_show_file(&ctx.test_project_path)?;

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_20_cleanup() {
        let mut ctx_guard = context::acquire(20);
        let ctx = ctx_guard.1.take().unwrap();
        drop(ctx);
    }

    fn assert_operation_history(
        mut operation_history: Vec<TestOperationHistoryItem>,
        operation_expectations: Vec<(&str, Option<(String, String, Box<dyn TestOperationHistoryKind>)>)>,
    ) {
        for (index, (&ref expectation_operation, expectation)) in operation_expectations
            .iter()
            .enumerate()
        {
            match expectation_operation {
                "eof" => {
                    assert!(operation_history.is_empty());
                    break;
                }
                _ => {}
            }

            let (item, remaining_operation_history) = operation_history.split_first().unwrap();
            println!(
                "index: {}, expectation: {}, item: {:?}",
                index, expectation_operation, item
            );

            match expectation_operation {
                "ignore" => {}
                "require" => {
                    assert_eq!(
                        &(
                            item.phase.clone(),
                            item.operation_reference.to_string(),
                            dyn_clone::clone_box(&*item.task_history)
                        ),
                        expectation.as_ref().unwrap()
                    );
                }
                _ => unreachable!(),
            }

            operation_history = Vec::from(remaining_operation_history);
        }
    }
}

mod help {
    use assert_cmd::Command;
    use indoc::indoc;
    use predicates::prelude::{predicate, PredicateBooleanExt};
    use util::test::print;

    #[test]
    fn no_args() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

        // and
        let expected_output = indoc! {"
            Usage: planner_cli [OPTIONS] <COMMAND>

            Commands:
              project  Project mode
              pcb      PCB mode
              process  Process mode, to define a process's operations and tasks
              parts    Parts mode, to use a parts data source
              table    Table mode, to show a data source's records as a table, and change part mappings
              help     Print this message or the help of the given subcommand(s)

            Options:
                  --trace [<TRACE>]  Trace log file
              -v, --verbose...       Increase logging verbosity
              -q, --quiet...         Decrease logging verbosity
              -h, --help             Print help
              -V, --version          Print version
        "};
        // when
        cmd
            // then
            .assert()
            .failure()
            .stderr(print("stderr").and(predicate::str::diff(expected_output)))
            .stdout(print("stdout"));
    }

    mod pcb_help {
        use assert_cmd::Command;
        use indoc::indoc;
        use predicates::prelude::{predicate, PredicateBooleanExt};
        use util::test::print;

        #[test]
        fn no_args() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                PCB mode

                Usage: planner_cli pcb [OPTIONS] --pcb-file <PCB_FILE> <COMMAND>

                Commands:
                  create                  Create a PCB file
                  configure-panel-sizing  Configure a PCB
                  help                    Print this message or the help of the given subcommand(s)

                Options:
                      --pcb-file <PCB_FILE>  Specify a PCB context
                  -v, --verbose...           Increase logging verbosity
                  -q, --quiet...             Decrease logging verbosity
                  -h, --help                 Print help
            "};

            // when
            cmd.args(&["pcb"])
                // then
                .assert()
                .failure()
                .stderr(print("stderr").and(predicate::str::diff(expected_output)))
                .stdout(print("stdout"));
        }

        #[test]
        fn help_for_create() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Create a PCB file

                Usage: planner_cli pcb --pcb-file <PCB_FILE> create [OPTIONS] --name <NAME> --units <UNITS> --design <DESIGN>...

                Options:
                      --name <NAME>         Name of the PCB, e.g. 'panel_1'
                      --units <UNITS>       The number of individual PCB units. 1 = single, >1 = panel
                      --design <DESIGN>...  The mapping of designs to units e.g. '1=design_a,2=design_b,3=design_a,4=design_b'. unit is 1-based
                  -v, --verbose...          Increase logging verbosity
                  -q, --quiet...            Decrease logging verbosity
                  -h, --help                Print help
            "};

            // when
            cmd.args(["pcb", "create", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_configure_panel_sizing() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Configure a PCB
                
                Usage: planner_cli pcb --pcb-file <PCB_FILE> configure-panel-sizing [OPTIONS]
                
                Options:
                      --edge-rails <DIMENSIONS>
                          Edge rails (left,right,top,bottom) e.g. 'left=5,right=5,top=10,bottom=10'
                      --size <VECTOR2>
                          PCB size (x,y) e.g. 'x=100,y=100'
                      --design-sizing <DESIGN_SIZING>
                          Design sizing (e.g. 'design_a:origin=x=15.25,y=15.25:offset=x=-10.0,y=-10.0:size=x=30.5,y=30.5')
                      --pcb-unit-position <PCB_UNIT_POSITIONING>
                          PCB unit positioning (e.g. '1:offset=x=10,y=10:rotation=90')
                  -v, --verbose...
                          Increase logging verbosity
                  -q, --quiet...
                          Decrease logging verbosity
                  -h, --help
                          Print help
            "};

            // when
            cmd.args(["pcb", "configure-panel-sizing", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }
    }

    mod process_help {
        use assert_cmd::Command;
        use indoc::indoc;
        use predicates::prelude::{predicate, PredicateBooleanExt};
        use util::test::print;

        #[test]
        fn no_args() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Process mode, to define a process's operations and tasks

                Usage: planner_cli process [OPTIONS] --process-file <PROCESS_FILE> <COMMAND>

                Commands:
                  create            Create a process file
                  set-reference     Change the process's reference
                  add-operation     Add an operation, after the others
                  remove-operation  Remove an operation
                  add-task          Add a task to an operation, after its other tasks
                  remove-task       Remove a task from an operation
                  add-rule          Add a rule
                  remove-rule       Remove a rule
                  help              Print this message or the help of the given subcommand(s)

                Options:
                      --process-file <PROCESS_FILE>  Specify a process context, e.g. 'glue_and_wave.process.json'
                  -v, --verbose...                   Increase logging verbosity
                  -q, --quiet...                     Decrease logging verbosity
                  -h, --help                         Print help
            "};

            // when
            cmd.args(&["process"])
                // then
                .assert()
                .failure()
                .stderr(print("stderr").and(predicate::str::diff(expected_output)))
                .stdout(print("stdout"));
        }

        #[test]
        fn help_for_create() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Create a process file

                Usage: planner_cli process --process-file <PROCESS_FILE> create [OPTIONS] --reference <REFERENCE>

                Options:
                      --reference <REFERENCE>  Process reference, e.g. 'glue_and_wave'
                      --preset <PRESET>        The process to start from, otherwise the process has no operations [possible values: pnp, manual]
                  -v, --verbose...             Increase logging verbosity
                  -q, --quiet...               Decrease logging verbosity
                  -h, --help                   Print help
            "};

            // when
            cmd.args(["process", "create", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_add_task() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Add a task to an operation, after its other tasks

                Usage: planner_cli process --process-file <PROCESS_FILE> add-task [OPTIONS] --operation <OPERATION> --task <TASK> --kind <KIND>

                Options:
                      --operation <OPERATION>
                          Operation reference

                      --task <TASK>
                          Task reference, e.g. 'wave_solder'

                      --kind <KIND>
                          What the task does, as far as the planner needs to know

                          Possible values:
                          - generic:          Started, and then completed or abandoned, by the user, e.g. applying glue or wave soldering
                          - load_pcbs:        As 'generic', and the project's report lists the PCBs to load
                          - place_components: Places the phase's components, complete once every placement has been placed or skipped.  At most one per process

                  -v, --verbose...
                          Increase logging verbosity

                  -q, --quiet...
                          Decrease logging verbosity

                  -h, --help
                          Print help (see a summary with '-h')
            "};

            // when
            cmd.args(["process", "add-task", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }
    }

    mod project_help {
        use assert_cmd::Command;
        use indoc::indoc;
        use predicates::prelude::{predicate, PredicateBooleanExt};
        use util::test::print;

        #[test]
        fn no_args() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Project mode
                
                Usage: planner_cli project [OPTIONS] --project <PROJECT_NAME> <COMMAND>
                
                Commands:
                  create                          Create a new job
                  add-pcb                         Add a PCB file to the project
                  remove-pcb                      Remove a PCB from the project
                  assign-variant-to-unit          Assign a design variant to a PCB unit
                  refresh-from-design-variants    Refresh from design variants
                  add-process                     Add a copy of a process, from its file, to the project
                  remove-process                  Remove a process from the project, its file is not deleted
                  refresh-processes               Update the project's copy of each process from its file
                  assign-process-to-parts         Assign a process to parts
                  create-phase                    Create a phase
                  assign-placements-to-phase      Assign placements to a phase
                  assign-feeder-to-load-out-item  Assign feeder to load-out item
                  set-placement-ordering          Set placement ordering for a phase
                  generate-artifacts              Generate artifacts
                  record-phase-operation          Record phase operation
                  record-placements-operation     Record placements operation
                  reset-operations                Reset operations
                  help                            Print this message or the help of the given subcommand(s)
                
                Options:
                      --path <PATH>             Path [default: .]
                      --project <PROJECT_NAME>  Project name
                  -v, --verbose...              Increase logging verbosity
                  -q, --quiet...                Decrease logging verbosity
                  -h, --help                    Print help
            "};

            // when
            cmd.args(&["project"])
                // then
                .assert()
                .failure()
                .stderr(print("stderr").and(predicate::str::diff(expected_output)))
                .stdout(print("stdout"));
        }

        #[test]
        fn help_for_create() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Create a new job

                Usage: planner_cli project --project <PROJECT_NAME> create [OPTIONS]

                Options:
                      --packages <PACKAGES>                  The source for packages
                      --package-mappings <PACKAGE_MAPPINGS>  The source for part to package mappings
                  -v, --verbose...                           Increase logging verbosity
                  -q, --quiet...                             Decrease logging verbosity
                  -h, --help                                 Print help
            "};

            // when
            cmd.args(["project", "create", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_add_pcb() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Add a PCB file to the project

                Usage: planner_cli project --project <PROJECT_NAME> add-pcb [OPTIONS] --file <FILE_REFERENCE>

                Options:
                      --file <FILE_REFERENCE>  The path of the PCB, e.g. 'relative:<some_relative_path>' or '<some_absolute_path>' paths can be prefixed with `relative:` to make them relative to the project path
                  -v, --verbose...             Increase logging verbosity
                  -q, --quiet...               Decrease logging verbosity
                  -h, --help                   Print help
            "};

            // when
            cmd.args(["project", "add-pcb", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_remove_pcb() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Remove a PCB from the project

                Usage: planner_cli project --project <PROJECT_NAME> remove-pcb [OPTIONS] --index <INDEX>

                Options:
                      --index <INDEX>  The zero-based index of the PCB
                  -v, --verbose...     Increase logging verbosity
                  -q, --quiet...       Decrease logging verbosity
                  -h, --help           Print help
            "};

            // when
            cmd.args(["project", "remove-pcb", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_assign_variant_to_unit() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Assign a design variant to a PCB unit

                Usage: planner_cli project --project <PROJECT_NAME> assign-variant-to-unit [OPTIONS] --unit <OBJECT_PATH>

                Options:
                      --unit <OBJECT_PATH>      PCB unit path
                      --variant <VARIANT_NAME>  Variant of the design
                  -v, --verbose...              Increase logging verbosity
                  -q, --quiet...                Decrease logging verbosity
                  -h, --help                    Print help
            "};

            // when
            cmd.args(["project", "assign-variant-to-unit", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_refresh_from_design_variants() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Refresh from design variants

                Usage: planner_cli project --project <PROJECT_NAME> refresh-from-design-variants [OPTIONS]
                
                Options:
                  -v, --verbose...  Increase logging verbosity
                  -q, --quiet...    Decrease logging verbosity
                  -h, --help        Print help
            "};

            // when
            cmd.args(["project", "refresh-from-design-variants", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_add_process() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Add a copy of a process, from its file, to the project

                Usage: planner_cli project --project <PROJECT_NAME> add-process [OPTIONS] --file <FILE_REFERENCE>

                Options:
                      --file <FILE_REFERENCE>  The path of the process file, e.g. 'relative=<some_relative_path>' or '<some_absolute_path>' paths can be prefixed with `relative=` to make them relative to the project path
                  -v, --verbose...             Increase logging verbosity
                  -q, --quiet...               Decrease logging verbosity
                  -h, --help                   Print help
            "};

            // when
            cmd.args(["project", "add-process", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_remove_process() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Remove a process from the project, its file is not deleted

                Usage: planner_cli project --project <PROJECT_NAME> remove-process [OPTIONS] --process <PROCESS>

                Options:
                      --process <PROCESS>  Process reference
                  -v, --verbose...         Increase logging verbosity
                  -q, --quiet...           Decrease logging verbosity
                  -h, --help               Print help
            "};

            // when
            cmd.args(["project", "remove-process", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_refresh_processes() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Update the project's copy of each process from its file

                Usage: planner_cli project --project <PROJECT_NAME> refresh-processes [OPTIONS]

                Options:
                  -v, --verbose...  Increase logging verbosity
                  -q, --quiet...    Decrease logging verbosity
                  -h, --help        Print help
            "};

            // when
            cmd.args(["project", "refresh-processes", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_assign_process_to_parts() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Assign a process to parts

                Usage: planner_cli project --project <PROJECT_NAME> assign-process-to-parts [OPTIONS] --process <PROCESS> --operation <OPERATION> --manufacturer <MANUFACTURER> --mpn <MPN>

                Options:
                      --process <PROCESS>            Process name
                      --operation <OPERATION>        Operation [possible values: add, remove]
                      --manufacturer <MANUFACTURER>  Manufacturer pattern (regexp)
                      --mpn <MPN>                    Manufacturer part number (regexp)
                  -v, --verbose...                   Increase logging verbosity
                  -q, --quiet...                     Decrease logging verbosity
                  -h, --help                         Print help
            "};

            // when
            cmd.args(["project", "assign-process-to-parts", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_create_phase() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Create a phase

                Usage: planner_cli project --project <PROJECT_NAME> create-phase [OPTIONS] --process <PROCESS> --reference <REFERENCE> --load-out <LOAD_OUT> --pcb-side <PCB_SIDE>

                Options:
                      --process <PROCESS>      Process name
                      --reference <REFERENCE>  Phase reference (e.g. 'top_1')
                      --load-out <LOAD_OUT>    Load-out source (e.g. 'load_out_1')
                      --pcb-side <PCB_SIDE>    PCB side [possible values: top, bottom]
                  -v, --verbose...             Increase logging verbosity
                  -q, --quiet...               Decrease logging verbosity
                  -h, --help                   Print help
            "};

            // when
            cmd.args(["project", "create-phase", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_assign_placements_to_phase() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Assign placements to a phase

                Usage: planner_cli project --project <PROJECT_NAME> assign-placements-to-phase [OPTIONS] --phase <PHASE> --operation <OPERATION> --placements <PLACEMENTS>

                Options:
                      --phase <PHASE>            Phase reference (e.g. 'top_1')
                      --operation <OPERATION>    Operation [possible values: set, clear]
                      --placements <PLACEMENTS>  Placements object path pattern (regexp)
                  -v, --verbose...               Increase logging verbosity
                  -q, --quiet...                 Decrease logging verbosity
                  -h, --help                     Print help
            "};

            // when
            cmd.args(["project", "assign-placements-to-phase", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_assign_feeder_to_load_out_item() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Assign feeder to load-out item

                Usage: planner_cli project --project <PROJECT_NAME> assign-feeder-to-load-out-item [OPTIONS] --phase <PHASE> --manufacturer <MANUFACTURER> --mpn <MPN>

                Options:
                      --phase <PHASE>                        Phase reference (e.g. 'top_1')
                      --feeder-reference <FEEDER_REFERENCE>  Feeder reference (e.g. 'FEEDER_1')
                      --manufacturer <MANUFACTURER>          Manufacturer pattern (regexp)
                      --mpn <MPN>                            Manufacturer part number (regexp)
                  -v, --verbose...                           Increase logging verbosity
                  -q, --quiet...                             Decrease logging verbosity
                  -h, --help                                 Print help
            "};

            // when
            cmd.args(["project", "assign-feeder-to-load-out-item", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_set_placement_ordering() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Set placement ordering for a phase

                Usage: planner_cli project --project <PROJECT_NAME> set-placement-ordering [OPTIONS] --phase <PHASE> --placement-orderings [<PLACEMENT_ORDERINGS>...]

                Options:
                      --phase <PHASE>
                          Phase reference (e.g. 'top_1')
                      --placement-orderings [<PLACEMENT_ORDERINGS>...]
                          Orderings (e.g. 'PCB_UNIT:ASC,FEEDER_REFERENCE:ASC,REF_DES:ASC')
                  -v, --verbose...
                          Increase logging verbosity
                  -q, --quiet...
                          Decrease logging verbosity
                  -h, --help
                          Print help
            "};

            // when
            cmd.args(["project", "set-placement-ordering", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_generate_artifacts() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Generate artifacts

                Usage: planner_cli project --project <PROJECT_NAME> generate-artifacts [OPTIONS]

                Options:
                  -v, --verbose...  Increase logging verbosity
                  -q, --quiet...    Decrease logging verbosity
                  -h, --help        Print help
            "};

            // when
            cmd.args(["project", "generate-artifacts", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_record_phase_operation() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Record phase operation

                Usage: planner_cli project --project <PROJECT_NAME> record-phase-operation [OPTIONS] --phase <PHASE> --operation <OPERATION> --task <TASK> --action <ACTION>

                Options:
                      --phase <PHASE>          Phase reference (e.g. 'top_1')
                      --operation <OPERATION>  Operation reference
                      --task <TASK>            The task to update
                      --action <ACTION>        The task action to apply [possible values: start, complete, abandon]
                  -v, --verbose...             Increase logging verbosity
                  -q, --quiet...               Decrease logging verbosity
                  -h, --help                   Print help
            "};

            // when
            cmd.args(["project", "record-phase-operation", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_record_placements_operation() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Record placements operation

                Usage: planner_cli project --project <PROJECT_NAME> record-placements-operation [OPTIONS] --object-path-patterns <OBJECT_PATH_PATTERNS>... --operation <OPERATION>

                Options:
                      --object-path-patterns <OBJECT_PATH_PATTERNS>...
                          List of reference designators to apply the operation to
                      --operation <OPERATION>
                          The completed operation to apply [possible values: placed, skipped, reset]
                  -v, --verbose...
                          Increase logging verbosity
                  -q, --quiet...
                          Decrease logging verbosity
                  -h, --help
                          Print help
            "};

            // when
            cmd.args(["project", "record-placements-operation", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }

        #[test]
        fn help_for_reset_operations() {
            // given
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner_cli"));

            // and
            let expected_output = indoc! {"
                Reset operations

                Usage: planner_cli project --project <PROJECT_NAME> reset-operations [OPTIONS]

                Options:
                  -v, --verbose...  Increase logging verbosity
                  -q, --quiet...    Decrease logging verbosity
                  -h, --help        Print help
            "};

            // when
            cmd.args(["project", "reset-operations", "--help"])
                // then
                .assert()
                .success()
                .stderr(print("stderr"))
                .stdout(print("stdout").and(predicate::str::diff(expected_output)));
        }
    }
}

/// calculate the offset for the bottom left position of a unit.
///
/// assumes the routing gap fully surrounds each unit
/// i.e. does not assume routing goes into the rails
/// a routing gap of 0 would be common for panels that use v-scoring and no routing.
/// assumes the combination of unit map and design_sizings will result in a rectangular panel.
/// assumes designs are not rotated.
fn calculate_offset(
    unit_map: &BTreeMap<PcbUnitIndex, DesignIndex>,
    design_sizings: &Vec<TestDesignSizing>,
    edge_rails: &Dimensions<f64>,
    routing_gap: f64,
    x_count: usize,
    y_count: usize,
    x_index: usize,
    y_index: usize,
) -> Vector2<f64> {
    let units = x_count * y_count;
    assert_eq!(units, unit_map.len());

    let mut x_value = edge_rails.left + routing_gap;
    let mut y_value = edge_rails.bottom + routing_gap;

    for x in 0..x_index {
        let map_index = y_index * x_count + x;

        let design_sizing_index = unit_map
            .get(&(map_index as PcbUnitIndex))
            .unwrap();
        let design_sizing = &design_sizings[*design_sizing_index];

        x_value += design_sizing.size.x + routing_gap;
    }

    for y in 0..y_index {
        let map_index = y * x_count + x_index;

        let design_sizing_index = unit_map
            .get(&(map_index as PcbUnitIndex))
            .unwrap();
        let design_sizing = &design_sizings[*design_sizing_index];

        y_value += design_sizing.size.y + routing_gap;
    }

    Vector2::new(x_value, y_value)
}

/// calculate the size of a panel.
///
/// assumes the routing gap fully surrounds each unit
/// i.e. does not assume routing goes into the rails
/// a routing gap of 0 would be common for panels that use v-scoring and no routing.
///
/// uses the first row of designs and first column of designs to calculate the size.
/// assumes the combination of unit map and design_sizings will result in a rectangular panel.
/// assumes designs are not rotated.
fn calculate_size(
    unit_map: &BTreeMap<PcbUnitIndex, DesignIndex>,
    design_sizings: &Vec<TestDesignSizing>,
    edge_rails: &Dimensions<f64>,
    routing_gap: f64,
    x_count: usize,
    y_count: usize,
) -> Vector2<f64> {
    let units = x_count * y_count;
    assert_eq!(units, unit_map.len());

    let mut x_value = edge_rails.left;
    let mut y_value = edge_rails.bottom;

    for x in 0..x_count {
        let map_index = x;

        let design_sizing_index = unit_map
            .get(&(map_index as PcbUnitIndex))
            .unwrap();
        let design_sizing = &design_sizings[*design_sizing_index];
        x_value += routing_gap;
        x_value += design_sizing.size.x;
    }

    for y in 0..y_count {
        let map_index = y * x_count;

        let design_sizing_index = unit_map
            .get(&(map_index as PcbUnitIndex))
            .unwrap();
        let design_sizing = &design_sizings[*design_sizing_index];

        y_value += routing_gap;
        y_value += design_sizing.size.y;
    }

    Vector2::new(
        x_value + routing_gap + edge_rails.right,
        y_value + routing_gap + edge_rails.top,
    )
}

#[cfg(test)]
mod sizing_tests {
    use std::collections::BTreeMap;

    use nalgebra::Vector2;
    use pnp::panel::Dimensions;

    use crate::common::project_builder::TestDesignSizing;
    use crate::{calculate_offset, calculate_size};

    #[test]
    pub fn panel_size_and_unit_offsets() {
        // given
        let edge_rails = Dimensions {
            left: 10.0,
            right: 10.0,
            top: 5.0,
            bottom: 5.0,
        };
        let routing_gap = 2.0;

        let design_a_sizing = TestDesignSizing {
            size: Vector2::new(60.0, 50.0),
            origin: Vector2::new(30.0, 25.0),
            gerber_offset: Default::default(),
            placement_offset: Default::default(),
        };
        let design_b_sizing = TestDesignSizing {
            size: Vector2::new(40.0, 50.0),
            origin: Vector2::new(20.0, 25.0),
            gerber_offset: Default::default(),
            placement_offset: Default::default(),
        };
        let design_sizings = vec![design_a_sizing, design_b_sizing];

        // layout:         unit_map:
        //
        // Row 2  : A,B,A    6,7,8
        // Row 1  : A,B,A    3,4,5
        // Row 0  : A,B,A    0,1,2
        // Columns: 0 1 2
        //
        // A and B have equal heights, but different widths

        let unit_map = BTreeMap::from_iter([(0, 0), (1, 1), (2, 0), (3, 0), (4, 1), (5, 0), (6, 0), (7, 1), (8, 0)]);

        // when
        let size = calculate_size(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3);
        let offsets = vec![
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 0, 0),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 1, 0),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 2, 0),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 0, 1),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 1, 1),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 2, 1),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 0, 2),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 1, 2),
            calculate_offset(&unit_map, &design_sizings, &edge_rails, routing_gap, 3, 3, 2, 2),
        ];

        // then
        assert_eq!(size, Vector2::new(188.0, 168.0));
        assert_eq!(offsets, vec![
            Vector2::new(10.0 + 2.0, 5.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0, 5.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0 + 40.0 + 2.0, 5.0 + 2.0),
            Vector2::new(10.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0 + 40.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(10.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(10.0 + 2.0 + 60.0 + 2.0, 5.0 + 2.0 + 50.0 + 2.0 + 50.0 + 2.0),
            Vector2::new(
                10.0 + 2.0 + 60.0 + 2.0 + 40.0 + 2.0,
                5.0 + 2.0 + 50.0 + 2.0 + 50.0 + 2.0
            ),
        ]);
    }
}

/// Projects from before processes had files, whose processes are kept in the project file.
mod legacy_processes {
    use std::fs;

    use assert_cmd::Command;
    use indoc::indoc;
    use planning::process::TaskKind;
    use tempfile::tempdir;
    use util::test::{prepare_args, print};

    use crate::common::project_builder::{TestOperationDefinition, TestProcessDefinition};

    const LEGACY_PROJECT: &str = indoc! {r#"
        {
            "name": "job1",
            "processes": [
                {
                    "reference": "manual",
                    "operations": [
                        { "reference": "load_pcbs", "tasks": ["core::load_pcbs"] },
                        { "reference": "manually_solder_components", "tasks": ["core::place_components", "core::manual_soldering"] }
                    ],
                    "rules": []
                }
            ]
        }
    "#};

    /// As the legacy project's process, the legacy task references are kept, so the states of its phases still match.
    fn legacy_manual_process() -> TestProcessDefinition {
        let mut definition = TestProcessDefinition::manual();
        definition.operations = vec![
            TestOperationDefinition::new("load_pcbs", &[("core::load_pcbs", TaskKind::LoadPcbs)]),
            TestOperationDefinition::new("manually_solder_components", &[
                ("core::place_components", TaskKind::PlaceComponents),
                ("core::manual_soldering", TaskKind::Generic),
            ]),
        ];
        definition
    }

    /// `PROCDEF-8`
    #[test]
    fn loading_a_project_moves_its_processes_to_their_own_files() -> Result<(), anyhow::Error> {
        // given
        let temp_dir = tempdir()?;
        let project_path = temp_dir
            .path()
            .join("project-job1.mpnp.json");
        fs::write(&project_path, LEGACY_PROJECT)?;

        // and a different process already uses the name of the file the process would be given
        let existing_path = temp_dir
            .path()
            .join("manual.process.json");
        let existing_content = TestProcessDefinition::pnp().content();
        fs::write(&existing_path, &existing_content)?;

        // and
        let path_arg = format!("--path {}", temp_dir.path().to_str().unwrap());
        let args = prepare_args(vec![
            "project",
            path_arg.as_str(),
            "--project job1",
            "refresh-processes",
        ]);

        // when
        Command::new(env!("CARGO_BIN_EXE_planner_cli"))
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"))
            .success();

        // and the process has a file of its own, with the kinds of the tasks that were built into the planner
        let process_content = fs::read_to_string(
            temp_dir
                .path()
                .join("manual_2.process.json"),
        )?;
        assert_eq!(process_content, legacy_manual_process().content());

        // and the other file is unchanged
        assert_eq!(fs::read_to_string(&existing_path)?, existing_content);

        // and the project refers to the file
        let project: serde_json::Value = serde_json::from_str(&fs::read_to_string(&project_path)?)?;
        assert_eq!(
            project["processes"][0]["process_file"],
            serde_json::json!({ "Relative": "manual_2.process.json" })
        );
        assert_eq!(
            project["processes"][0]["operations"][1]["tasks"][0],
            serde_json::json!({
                "reference": "core::place_components",
                "kind": "place_components"
            })
        );

        Ok(())
    }
}

mod relative_load_outs {
    use std::fs;

    use assert_cmd::Command;
    use indoc::indoc;
    use tempfile::tempdir;
    use util::test::{prepare_args, print};

    const PROJECT: &str = indoc! {r#"
        {
            "name": "job1",
            "processes": [
                {
                    "reference": "manual",
                    "operations": [
                        { "reference": "load_pcbs", "tasks": ["core::load_pcbs"] }
                    ],
                    "rules": []
                }
            ]
        }
    "#};

    /// `ISSUE-7`, `STORE-11`, a phase's relative load-out is created in the project's folder, not the CLI's working
    /// directory, and the log names the file created.
    #[test]
    fn a_relative_load_out_is_created_in_the_projects_folder() -> Result<(), anyhow::Error> {
        // given the CLI is run from a folder above the project's
        let temp_dir = tempdir()?;
        let job = temp_dir.path().join("job");
        fs::create_dir(&job)?;
        fs::write(job.join("project-job1.mpnp.json"), PROJECT)?;
        let trace_path = temp_dir.path().join("trace.log");

        // and
        let trace_arg = format!("--trace {}", trace_path.to_str().unwrap());
        let args = prepare_args(vec![
            trace_arg.as_str(),
            "-v",
            "project",
            "--path job",
            "--project job1",
            "create-phase",
            "--process manual",
            "--reference top_1",
            "--load-out load_out_1",
            "--pcb-side top",
        ]);

        // when
        Command::new(env!("CARGO_BIN_EXE_planner_cli"))
            .current_dir(temp_dir.path())
            .args(args)
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"))
            .success();

        // and
        let trace_content = fs::read_to_string(&trace_path)?;
        let load_out = job.join("load_out_1");
        let relative_load_out = std::path::Path::new("job").join("load_out_1");
        let created = format!("Created load-out. source: '{}'", relative_load_out.display());
        let storing = format!("Storing load-out. source: '{}'", relative_load_out.display());
        assert_contains_inorder!(trace_content, [
            "Received event. event: CreatePhase {",
            "load_out: File(\"load_out_1\")",
            &created,
            "Created phase. reference: 'top_1', process: manual, load_out: \"load_out_1\"",
            &storing,
        ]);

        // and
        assert!(load_out.exists());
        assert!(!temp_dir
            .path()
            .join("load_out_1")
            .exists());

        Ok(())
    }
}
