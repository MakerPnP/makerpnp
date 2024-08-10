#[macro_use]
extern crate makerpnp;

#[path = "inc/int_test.rs"]
pub mod int_test;

#[cfg(feature="cli")]
mod operation_sequence_1 {
    use std::collections::BTreeMap;
    use std::fs::{File, read_to_string};
    use std::io::Write;
    use std::path::PathBuf;
    use assert_cmd::Command;
    use indoc::indoc;
    use rust_decimal_macros::dec;
    use tempfile::tempdir;
    use crate::int_test::{build_temp_file, print};
    use crate::int_test::load_out_builder::{LoadOutCSVBuilder, TestLoadOutRecord};
    use crate::int_test::phase_placement_builder::{PhasePlacementsCSVBuilder, TestPhasePlacementRecord};
    use crate::int_test::project_builder::TestProjectBuilder;
    use crate::int_test::project_report_builder::{ProjectReportBuilder, TestPcb, TestPcbUnitAssignment, TestPhaseLoadOutAssignmentItem, TestPhaseOperation, TestPhaseOverview, TestPhaseSpecification};

    /// A context, which will be dropped when the tests are completed.
    mod context {
        use std::sync::{Mutex, MutexGuard};
        use std::thread::sleep;
        use std::time::Duration;
        use super::*;

        #[derive(Debug)]
        pub struct Context {
            pub temp_dir: tempfile::TempDir,

            pub trace_log_arg: String,
            pub path_arg: String,
            pub project_arg: String,
            pub test_trace_log_path: PathBuf,
            pub test_project_path: PathBuf,
            pub phase_1_load_out_path: PathBuf,
        }

        impl Context {
            pub fn new() -> Self {
                let temp_dir = tempdir().unwrap();

                let path_arg = format!("--path={}", temp_dir.path().to_str().unwrap());

                let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
                let trace_log_arg = format!("--trace={}", test_trace_log_file_name.to_str().unwrap());

                let (test_project_path, _test_project_file_name) = build_temp_file(&temp_dir, "project-job1", "mpnp.json");

                let project_arg = "--project=job1".to_string();

                let mut phase_1_load_out_path = PathBuf::from(temp_dir.path());
                phase_1_load_out_path.push("phase_1_load_out_1.csv");

                Context {
                    temp_dir,
                    path_arg,
                    project_arg,
                    trace_log_arg,
                    test_trace_log_path,
                    test_project_path,
                    phase_1_load_out_path,
                }
            }
        }

        impl Drop for Context {
            fn drop(&mut self) {
                println!("destroying context. temp_dir: {}", self.temp_dir.path().to_str().unwrap());
            }
        }

        /// IMPORTANT: lock content must be dropped manually, as static items are never dropped.
        static LOCK: Mutex<(usize, Option<Context>)> = Mutex::new((0, None));

        /// Use a mutex to prevent multiple test threads interacting with the same static state.
        /// This can happen when tests use the same mock context.  Without this mechanism tests will
        /// interact with each other causing unexpected results and test failures.
        pub fn aquire(sequence: usize) -> MutexGuard<'static, (usize, Option<Context>)> {
            let mut lock = loop {
                let mut lock = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if lock.0 == sequence-1 {
                    lock.0 += 1;
                    break lock
                }
                drop(lock);

                sleep(Duration::from_millis(100));
            };

            if lock.1.is_none() {
                lock.1.replace(Context::new());
            }

            lock
        }
    }

    #[test]
    fn sequence_01_create_job() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(1);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_processes(&["pnp", "manual"])
            .content();

        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "create",
        ];
        println!("args: {:?}", args);

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Created job: job1\n",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_02_add_pcb() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(2);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_processes(&["pnp", "manual"])
            .with_pcbs(&[
                ("panel", "panel_a"),
            ])
            .content();

        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "add-pcb",
            "--kind=panel",
            "--name=panel_a",
        ];
        println!("args: {:?}", args);

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Added panel PCB. name: 'panel_a'\n",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_03_assign_variant_to_unit() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(3);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R1","RES_MFR1","RES1","true","Top","10","110","0"
            "C1","CAP_MFR1","CAP1","true","Bottom","30","130","180"
            "J1","CONN_MFR1","CONN1","true","Top","40","140","-90"
            "R3","RES_MFR1","RES1","true","Top","5","105","90"
        "#};
        // two refdes on the same side should use the same part (R1, R3)

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        let mut placments_file = File::create(placements_path)?;
        placments_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placments_file.flush()?;

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_processes(&["pnp", "manual"])
            .with_pcbs(&[
                ("panel", "panel_a"),
            ])
            .with_unit_assignments(&[
                (
                    "panel=1::unit=1",
                    BTreeMap::from([
                        ("design_name", "design_a"),
                        ("variant_name", "variant_a"),
                    ])
                )
            ])
            .with_part_states(&[
                (("CAP_MFR1", "CAP1"), &[]),
                (("CONN_MFR1", "CONN1"), &[]),
                (("RES_MFR1", "RES1"), &[]),
            ])
            .with_placements(&[
                (
                    "panel=1::unit=1::ref_des=C1",
                    "panel=1::unit=1",
                    ("C1", "CAP_MFR1", "CAP1", true, "bottom", dec!(30), dec!(130), dec!(180)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=J1",
                    "panel=1::unit=1",
                    ("J1", "CONN_MFR1", "CONN1", true, "top", dec!(40), dec!(140), dec!(-90)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R1",
                    "panel=1::unit=1",
                    ("R1", "RES_MFR1", "RES1", true, "top", dec!(10), dec!(110), dec!(0)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R3",
                    "panel=1::unit=1",
                    ("R3", "RES_MFR1", "RES1", true, "top", dec!(5), dec!(105), dec!(90)),
                    false,
                    "Known",
                    None,
                ),
            ])
            .content();

        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-variant-to-unit",
            "--design=design_a",
            "--variant=variant_a",
            "--unit=panel=1::unit=1",
        ];

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Unit assignment added. unit: 'panel=1::unit=1', design_variant: design_a-variant_a\n",
            "New part. part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }\n",
            "New part. part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }\n",
            "New part. part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }\n",
            "New placement. placement: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 10, y: 110, rotation: 0 }\n",
            "New placement. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom, x: 30, y: 130, rotation: 180 }\n",
            "New placement. placement: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Top, x: 40, y: 140, rotation: -90 }\n",
            "New placement. placement: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 5, y: 105, rotation: 90 }\n",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_04_assign_process_to_parts() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(4);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R1","RES_MFR1","RES1","true","Top","110","1110","1"
            "R2","RES_MFR2","RES2","true","Top","120","1120","91"
            "J1","CONN_MFR1","CONN1","true","Top","130","1130","-179"
            "R3","RES_MFR1","RES1","true","Top","105","1105","91"
        "#};

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        let mut placments_file = File::create(placements_path)?;
        placments_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placments_file.flush()?;

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_processes(&["pnp", "manual"])
            .with_pcbs(&[
                ("panel", "panel_a"),
            ])
            .with_unit_assignments(&[
                (
                    "panel=1::unit=1",
                    BTreeMap::from([
                        ("design_name", "design_a"),
                        ("variant_name", "variant_a"),
                    ])
                )
            ])
            .with_part_states(&[
                (("CONN_MFR1", "CONN1"), &["pnp"]),
                (("RES_MFR1", "RES1"), &["pnp"]),
                (("RES_MFR2", "RES2"), &["pnp"]),
            ])
            .with_placements(&[
                (
                    "panel=1::unit=1::ref_des=C1",
                    "panel=1::unit=1",
                    ("C1", "CAP_MFR1", "CAP1", true, "bottom", dec!(30), dec!(130), dec!(180)),
                    false,
                    "Unknown",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=J1",
                    "panel=1::unit=1",
                    ("J1", "CONN_MFR1", "CONN1", true, "top", dec!(130), dec!(1130), dec!(-179)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R1",
                    "panel=1::unit=1",
                    ("R1", "RES_MFR1", "RES1", true, "top", dec!(110), dec!(1110), dec!(1)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R2",
                    "panel=1::unit=1",
                    ("R2", "RES_MFR2", "RES2", true, "top", dec!(120), dec!(1120), dec!(91)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R3",
                    "panel=1::unit=1",
                    ("R3", "RES_MFR1", "RES1", true, "top", dec!(105), dec!(1105), dec!(91)),
                    false,
                    "Known",
                    None,
                ),
            ])
            .content();

        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-process-to-parts",
            "--process=pnp",
            "--manufacturer=.*",
            "--mpn=.*",
        ];

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "New part. part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }\n",
            "Removing previously part. part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }\n",
            "Updating placement. old: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 10, y: 110, rotation: 0 }, new: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 110, y: 1110, rotation: 1 }\n",
            "New placement. placement: Placement { ref_des: \"R2\", part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }, place: true, pcb_side: Top, x: 120, y: 1120, rotation: 91 }\n",
            "Updating placement. old: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Top, x: 40, y: 140, rotation: -90 }, new: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Top, x: 130, y: 1130, rotation: -179 }\n",
            "Updating placement. old: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 5, y: 105, rotation: 90 }, new: Placement { ref_des: \"R3\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top, x: 105, y: 1105, rotation: 91 }\n",
            "Marking placement as unused. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom, x: 30, y: 130, rotation: 180 }\n",
            "Added process. part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, applicable_processes: {Process(\"pnp\")}",
            "Added process. part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }, applicable_processes: {Process(\"pnp\")}",
            "Added process. part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, applicable_processes: {Process(\"pnp\")}",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_05_create_phase() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(5);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_processes(&["pnp", "manual"])
            .with_pcbs(&[
                ("panel", "panel_a"),
            ])
            .with_unit_assignments(&[
                (
                    "panel=1::unit=1",
                    BTreeMap::from([
                        ("design_name", "design_a"),
                        ("variant_name", "variant_a"),
                    ])
                )
            ])
            .with_part_states(&[
                (("CONN_MFR1", "CONN1"), &["pnp"]),
                (("RES_MFR1", "RES1"), &["pnp"]),
                (("RES_MFR2", "RES2"), &["pnp"]),
            ])
            .with_phases(
                &[
                    ("top_1", "pnp", ctx.phase_1_load_out_path.to_str().unwrap(), "top", &[])
                ]
            )
            .with_placements(&[
                (
                    "panel=1::unit=1::ref_des=C1",
                    "panel=1::unit=1",
                    ("C1", "CAP_MFR1", "CAP1", true, "bottom", dec!(30), dec!(130), dec!(180)),
                    false,
                    "Unknown",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=J1",
                    "panel=1::unit=1",
                    ("J1", "CONN_MFR1", "CONN1", true, "top", dec!(130), dec!(1130), dec!(-179)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R1",
                    "panel=1::unit=1",
                    ("R1", "RES_MFR1", "RES1", true, "top", dec!(110), dec!(1110), dec!(1)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R2",
                    "panel=1::unit=1",
                    ("R2", "RES_MFR2", "RES2", true, "top", dec!(120), dec!(1120), dec!(91)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R3",
                    "panel=1::unit=1",
                    ("R3", "RES_MFR1", "RES1", true, "top", dec!(105), dec!(1105), dec!(91)),
                    false,
                    "Known",
                    None,
                ),
            ])
            .content();

        // and
        let phase_1_load_out_arg = format!("--load-out={}", ctx.phase_1_load_out_path.to_str().unwrap());

        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "create-phase",
            "--reference=top_1",
            "--process=pnp",
            &phase_1_load_out_arg,
            "--pcb-side=top",
        ];

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        let load_out_creation_message = format!("Created load-out. source: '{}'", ctx.phase_1_load_out_path.to_str().unwrap());

        assert_contains_inorder!(trace_content, [
            &load_out_creation_message,
            "Created phase. reference: 'top_1', process: pnp",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_06_assign_placements_to_phase() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(6);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_processes(&["pnp", "manual"])
            .with_pcbs(&[
                ("panel", "panel_a"),
            ])
            .with_unit_assignments(&[
                (
                    "panel=1::unit=1",
                    BTreeMap::from([
                        ("design_name", "design_a"),
                        ("variant_name", "variant_a"),
                    ])
                )
            ])
            .with_part_states(&[
                (("CONN_MFR1", "CONN1"), &["pnp"]),
                (("RES_MFR1", "RES1"), &["pnp"]),
                (("RES_MFR2", "RES2"), &["pnp"]),
            ])
            .with_phases(
                &[
                    ("top_1", "pnp", ctx.phase_1_load_out_path.to_str().unwrap(), "top", &[])
                ]
            )
            .with_placements(&[
                (
                    "panel=1::unit=1::ref_des=C1",
                    "panel=1::unit=1",
                    ("C1", "CAP_MFR1", "CAP1", true, "bottom", dec!(30), dec!(130), dec!(180)),
                    false,
                    "Unknown",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=J1",
                    "panel=1::unit=1",
                    ("J1", "CONN_MFR1", "CONN1", true, "top", dec!(130), dec!(1130), dec!(-179)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R1",
                    "panel=1::unit=1",
                    ("R1", "RES_MFR1", "RES1", true, "top", dec!(110), dec!(1110), dec!(1)),
                    false,
                    "Known",
                    Some("top_1"),
                ),
                (
                    "panel=1::unit=1::ref_des=R2",
                    "panel=1::unit=1",
                    ("R2", "RES_MFR2", "RES2", true, "top", dec!(120), dec!(1120), dec!(91)),
                    false,
                    "Known",
                    Some("top_1"),
                ),
                (
                    "panel=1::unit=1::ref_des=R3",
                    "panel=1::unit=1",
                    ("R3", "RES_MFR1", "RES1", true, "top", dec!(105), dec!(1105), dec!(91)),
                    false,
                    "Known",
                    Some("top_1"),
                ),
            ])
            .content();

        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "assign-placements-to-phase",
            "--phase=top_1",

            // By placement path pattern
            //"--placements=panel=1::unit=1::ref_des=R1"
            "--placements=panel=1::unit=1::ref_des=R.*",
            //"--placements=panel=1::unit=1::ref_des=J1",
            //"--placements=panel=.*::unit=.*::ref_des=R1"
            //"--placements=panel=1::unit=.*::ref_des=.*"
            //"--placements=.*::ref_des=R.*"
            //"--placements=.*",

            // FUTURE By manufacturer and mpn
            // "--manufacturer=RES_MFR.*",
            // "--mpn=.*"
        ];
        
        // and
        let expected_phase_1_load_out_content = LoadOutCSVBuilder::new()
            .with_items(&[
                TestLoadOutRecord { reference: "".to_string(), manufacturer: "RES_MFR1".to_string(), mpn: "RES1".to_string() },
                TestLoadOutRecord { reference: "".to_string(), manufacturer: "RES_MFR2".to_string(), mpn: "RES2".to_string() },
            ])
            .as_string();

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        let loading_load_out_message = format!("Loading load-out. source: '{}'", ctx.phase_1_load_out_path.to_str().unwrap());
        let storing_load_out_message = format!("Storing load-out. source: '{}'", ctx.phase_1_load_out_path.to_str().unwrap());
        
        assert_contains_inorder!(trace_content, [
            "Assigning placement to phase. phase: top_1, placement_path: panel=1::unit=1::ref_des=R1",
            "Assigning placement to phase. phase: top_1, placement_path: panel=1::unit=1::ref_des=R2",
            "Assigning placement to phase. phase: top_1, placement_path: panel=1::unit=1::ref_des=R3",
            &loading_load_out_message,
            r#"Checking for part in load_out. part: Part { manufacturer: "RES_MFR1", mpn: "RES1" }"#,
            r#"Checking for part in load_out. part: Part { manufacturer: "RES_MFR2", mpn: "RES2" }"#,
            &storing_load_out_message,
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        // and
        let phase_1_load_out_content: String = read_to_string(ctx.phase_1_load_out_path.clone())?;
        println!("actual:\n{}", phase_1_load_out_content);
        println!("expected:\n{}", expected_phase_1_load_out_content);

        assert_eq!(phase_1_load_out_content, expected_phase_1_load_out_content);

        Ok(())
    }

    #[test]
    fn sequence_07_assign_feeder_to_load_out_item() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(7);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));
        
        // and
        let load_out_arg = format!("--load-out={}", ctx.phase_1_load_out_path.to_str().unwrap());
        
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            "assign-feeder-to-load-out-item",
            load_out_arg.as_str(),
            "--feeder-reference=FEEDER_1",
            "--manufacturer=.*",
            "--mpn=RES1",
        ];

        let expected_phase_1_load_out_content = LoadOutCSVBuilder::new()
            .with_items(&[
                TestLoadOutRecord { reference: "FEEDER_1".to_string(), manufacturer: "RES_MFR1".to_string(), mpn: "RES1".to_string() },
                TestLoadOutRecord { reference: "".to_string(), manufacturer: "RES_MFR2".to_string(), mpn: "RES2".to_string() },
            ])
            .as_string();
        
        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

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
    fn sequence_08_set_placement_ordering() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(8);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_processes(&["pnp", "manual"])
            .with_pcbs(&[
                ("panel", "panel_a"),
            ])
            .with_unit_assignments(&[
                (
                    "panel=1::unit=1",
                    BTreeMap::from([
                        ("design_name", "design_a"),
                        ("variant_name", "variant_a"),
                    ])
                )
            ])
            .with_part_states(&[
                (("CONN_MFR1", "CONN1"), &["pnp"]),
                (("RES_MFR1", "RES1"), &["pnp"]),
                (("RES_MFR2", "RES2"), &["pnp"]),
            ])
            .with_phases(
                &[(
                    "top_1",
                    "pnp",
                    ctx.phase_1_load_out_path.to_str().unwrap(),
                    "top",
                    &[("PcbUnit", "Asc"),("FeederReference", "Asc")],
                )]
            )
            .with_placements(&[
                (
                    "panel=1::unit=1::ref_des=C1",
                    "panel=1::unit=1",
                    ("C1", "CAP_MFR1", "CAP1", true, "bottom", dec!(30), dec!(130), dec!(180)),
                    false,
                    "Unknown",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=J1",
                    "panel=1::unit=1",
                    ("J1", "CONN_MFR1", "CONN1", true, "top", dec!(130), dec!(1130), dec!(-179)),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R1",
                    "panel=1::unit=1",
                    ("R1", "RES_MFR1", "RES1", true, "top", dec!(110), dec!(1110), dec!(1)),
                    false,
                    "Known",
                    Some("top_1"),
                ),
                (
                    "panel=1::unit=1::ref_des=R2",
                    "panel=1::unit=1",
                    ("R2", "RES_MFR2", "RES2", true, "top", dec!(120), dec!(1120), dec!(91)),
                    false,
                    "Known",
                    Some("top_1"),
                ),
                (
                    "panel=1::unit=1::ref_des=R3",
                    "panel=1::unit=1",
                    ("R3", "RES_MFR1", "RES1", true, "top", dec!(105), dec!(1105), dec!(91)),
                    false,
                    "Known",
                    Some("top_1"),
                ),
            ])
            .content();


        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "set-placement-ordering",
            "--phase=top_1",
            "--orderings=PCB_UNIT:ASC,FEEDER_REFERENCE:ASC",
            
            // example for PnP machine placement
            //"--orderings=PCB_UNIT:ASC,COST:ASC,AREA:ASC,HEIGHT;ASC,FEEDER_REFERENCE:ASC",
            // example for manual placement
            //"--orderings=COST:ASC,AREA:ASC,HEIGHT;ASC,PART:ASC,PCB_UNIT:ASC",
        ];

        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Phase orderings set. phase: 'top_1', orderings: [PCB_UNIT:ASC, FEEDER_REFERENCE:ASC]",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_09_generate_artifacts() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(9);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_phase_1_placements_content = PhasePlacementsCSVBuilder::new()
            .with_items(&[
                TestPhasePlacementRecord {
                    object_path: "panel=1::unit=1::ref_des=R2".to_string(),
                    feeder_reference: "".to_string(),
                    manufacturer: "RES_MFR2".to_string(),
                    mpn: "RES2".to_string(),
                    x: dec!(120),
                    y: dec!(1120),
                    rotation: dec!(91),
                },
                TestPhasePlacementRecord {
                    object_path: "panel=1::unit=1::ref_des=R1".to_string(),
                    feeder_reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                    x: dec!(110),
                    y: dec!(1110),
                    rotation: dec!(1),
                },
                TestPhasePlacementRecord {
                    object_path: "panel=1::unit=1::ref_des=R3".to_string(),
                    feeder_reference: "FEEDER_1".to_string(),
                    manufacturer: "RES_MFR1".to_string(),
                    mpn: "RES1".to_string(),
                    x: dec!(105),
                    y: dec!(1105),
                    rotation: dec!(91),
                },
            ])
            .as_string();

        let expected_project_report_content = ProjectReportBuilder::default()
            .with_name("job1")
            .with_phases_overview(&[
                TestPhaseOverview { phase_name: "top_1".to_string() },
            ])
            .with_phase_specification(&[
                TestPhaseSpecification {
                    phase_name: "top_1".to_string(),
                    operations: vec![
                        TestPhaseOperation::PreparePcbs { pcbs: vec![
                            TestPcb::Panel {
                                name: "panel_a".to_string(),
                                unit_assignments: vec![TestPcbUnitAssignment {
                                    unit_path: "panel=1::unit=1".to_string(),
                                    design_name: "design_a".to_string(),
                                    variant_name: "variant_a".to_string(),
                                }]
                            }
                        ] }
                    ],
                    load_out_assignments: vec![
                        TestPhaseLoadOutAssignmentItem {
                            feeder_reference: "FEEDER_1".to_string(),
                            manufacturer: "RES_MFR1".to_string(),
                            mpn: "RES1".to_string(),
                            quantity: 2, // R1 and R3
                        },
                        TestPhaseLoadOutAssignmentItem {
                            feeder_reference: "".to_string(),
                            manufacturer: "RES_MFR2".to_string(),
                            mpn: "RES2".to_string(),
                            quantity: 1,
                        },
                    ]
                }
            ])
            .as_string();
        
        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            ctx.project_arg.as_str(),
            "generate-artifacts",
        ];
        // when
        cmd.args(args)
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(ctx.test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Generated artifacts.\n",
        ]);
        
        // and
        let mut phase_1_placements_file_path = PathBuf::from(ctx.temp_dir.path());
        phase_1_placements_file_path.push("top_1_placements.csv");

        let phase_1_placements_content: String = read_to_string(phase_1_placements_file_path)?;
        println!("{}", phase_1_placements_content);

        assert_eq!(phase_1_placements_content, expected_phase_1_placements_content);

        // and
        println!("expected:\n{}", expected_project_report_content);

        let mut project_report_file_path = PathBuf::from(ctx.temp_dir.path());
        project_report_file_path.push("job1_report.json");

        let project_report_content: String = read_to_string(project_report_file_path)?;
        println!("actual:\n{}", project_report_content);

        assert_eq!(project_report_content, expected_project_report_content);

        Ok(())
    }
    
    #[test]
    fn sequence_10_cleanup() {
        let mut ctx_guard = context::aquire(10);
        let ctx = ctx_guard.1.take().unwrap();
        drop(ctx);
    }
}

#[cfg(feature="cli")]
mod help {
    use assert_cmd::Command;
    use indoc::indoc;
    use predicates::prelude::{predicate, PredicateBooleanExt};
    use crate::int_test::print;

    // FUTURE ideally we want to require the 'project' argument for project-specific sub-commands
    //        but without excessive code duplication.

    #[test]
    fn no_args() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Usage: planner [OPTIONS] <COMMAND>

            Commands:
              create                          Create a new job
              add-pcb                         Add a PCB
              assign-variant-to-unit          Assign a design variant to a PCB unit
              assign-process-to-parts         Assign a process to parts
              create-phase                    Create a phase
              assign-placements-to-phase      Assign placements to a phase
              assign-feeder-to-load-out-item  Assign feeder to load-out item
              set-placement-ordering          Set placement ordering for a phase
              generate-artifacts              Generate artifacts
              help                            Print this message or the help of the given subcommand(s)

            Options:
                  --trace[=<TRACE>]         Trace log file
                  --path=<PATH>             Path [default: .]
                  --project=<PROJECT_NAME>  Project name
              -h, --help                    Print help
              -V, --version                 Print version
        "};

        // when
        cmd
            // then
            .assert()
            .failure()
            .stderr(print("stderr").and(predicate::str::diff(expected_output)))
            .stdout(print("stdout"));
    }

    #[test]
    fn help_for_create() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Create a new job

            Usage: planner create

            Options:
              -h, --help  Print help
        "};

        // when
        cmd.args(["create", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }

    #[test]
    fn help_for_add_pcb() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Add a PCB

            Usage: planner add-pcb --kind=<KIND> --name=<NAME>

            Options:
                  --kind=<KIND>  PCB kind [possible values: single, panel]
                  --name=<NAME>  Name of the PCB, e.g. 'panel_1'
              -h, --help         Print help
        "};

        // when
        cmd.args(["add-pcb", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }

    #[test]
    fn help_for_assign_variant_to_unit() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Assign a design variant to a PCB unit

            Usage: planner assign-variant-to-unit --design=<DESIGN_NAME> --variant=<VARIANT_NAME> --unit=<UNIT_PATH>

            Options:
                  --design=<DESIGN_NAME>    Name of the design
                  --variant=<VARIANT_NAME>  Variant of the design
                  --unit=<UNIT_PATH>        PCB unit path
              -h, --help                    Print help
        "};

        // when
        cmd.args(["assign-variant-to-unit", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }

    #[test]
    fn help_for_assign_process_to_parts() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Assign a process to parts

            Usage: planner assign-process-to-parts --process=<PROCESS> --manufacturer=<MANUFACTURER> --mpn=<MPN>

            Options:
                  --process=<PROCESS>            Process name
                  --manufacturer=<MANUFACTURER>  Manufacturer pattern (regexp)
                  --mpn=<MPN>                    Manufacturer part number (regexp)
              -h, --help                         Print help
        "};

        // when
        cmd.args(["assign-process-to-parts", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }

    #[test]
    fn help_for_create_phase() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Create a phase

            Usage: planner create-phase --process=<PROCESS> --reference=<REFERENCE> --load-out=<LOAD_OUT> --pcb-side=<PCB_SIDE>

            Options:
                  --process=<PROCESS>      Process name
                  --reference=<REFERENCE>  Phase reference (e.g. 'top_1')
                  --load-out=<LOAD_OUT>    Load-out source (e.g. 'load_out_1')
                  --pcb-side=<PCB_SIDE>    PCB side [possible values: top, bottom]
              -h, --help                   Print help
        "};

        // when
        cmd.args(["create-phase", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }

    #[test]
    fn help_for_assign_placements_to_phase() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Assign placements to a phase

            Usage: planner assign-placements-to-phase --phase=<PHASE> --placements=<PLACEMENTS>

            Options:
                  --phase=<PHASE>            Phase reference (e.g. 'top_1')
                  --placements=<PLACEMENTS>  Placements pattern (regexp)
              -h, --help                     Print help
        "};

        // when
        cmd.args(["assign-placements-to-phase", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }

    #[test]
    fn help_for_assign_feeder_to_load_out_item() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Assign feeder to load-out item

            Usage: planner assign-feeder-to-load-out-item --load-out=<LOAD_OUT> --feeder-reference=<FEEDER_REFERENCE> --manufacturer=<MANUFACTURER> --mpn=<MPN>

            Options:
                  --load-out=<LOAD_OUT>                  Load-out source (e.g. 'load_out_1')
                  --feeder-reference=<FEEDER_REFERENCE>  Feeder reference (e.g. 'FEEDER_1')
                  --manufacturer=<MANUFACTURER>          Manufacturer pattern (regexp)
                  --mpn=<MPN>                            Manufacturer part number (regexp)
              -h, --help                                 Print help
        "};

        // when
        cmd.args(["assign-feeder-to-load-out-item", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }

    #[test]
    fn help_for_set_placement_ordering() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Set placement ordering for a phase

            Usage: planner set-placement-ordering [OPTIONS] --phase=<PHASE>

            Options:
                  --phase=<PHASE>               Phase reference (e.g. 'top_1')
                  --orderings[=<ORDERINGS>...]  Orderings (e.g. 'PCB_UNIT:ASC,FEEDER_REFERENCE:ASC')
              -h, --help                        Print help
        "};

        // when
        cmd.args(["set-placement-ordering", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }


    #[test]
    fn help_for_generate_artifacts() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Generate artifacts

            Usage: planner generate-artifacts

            Options:
              -h, --help  Print help
        "};

        // when
        cmd.args(["generate-artifacts", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }
}
