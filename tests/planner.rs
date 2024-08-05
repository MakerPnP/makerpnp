#[macro_use]
extern crate makerpnp;

use std::collections::BTreeMap;
use serde::Serialize;
use serde_json::{json, Map, Value};

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
    use tempfile::tempdir;
    use crate::int_test::{build_temp_file, print};
    use crate::int_test::load_out_builder::{LoadOutCSVBuilder, TestLoadOutRecord};
    use crate::TestProjectBuilder;

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
            pub name_arg: String,
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

                let name_arg = "--name=job1".to_string();

                let mut phase_1_load_out = PathBuf::from(temp_dir.path());
                phase_1_load_out.push("phase_1_load_out_1.csv");

                Context {
                    temp_dir,
                    path_arg,
                    name_arg,
                    trace_log_arg,
                    test_trace_log_path,
                    test_project_path,
                    phase_1_load_out_path: phase_1_load_out,
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
    fn sequence_1_create_job() -> Result<(), anyhow::Error> {
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
            ctx.name_arg.as_str(),
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
    fn sequence_2_assign_variant_to_unit() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(2);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide"
            "R1","RES_MFR1","RES1","true","Top"
            "C1","CAP_MFR1","CAP1","true","Bottom"
            "J1","CONN_MFR1","CONN1","true","Top"
        "#};

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        let mut placments_file = File::create(placements_path)?;
        placments_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placments_file.flush()?;

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_unit_assignments(&[
                (
                    "panel=1::unit=1",
                    BTreeMap::from([
                        ("design_name", "design_a"),
                        ("variant_name", "variant_a"),
                    ])
                )
            ])
            .with_processes(&["pnp", "manual"])
            .with_part_states(&[
                (("CAP_MFR1", "CAP1"), &[]),
                (("CONN_MFR1", "CONN1"), &[]),
                (("RES_MFR1", "RES1"), &[]),
            ])
            .with_placements(&[
                (
                    "panel=1::unit=1::ref_des=C1",
                    "panel=1::unit=1",
                    ("C1", "CAP_MFR1", "CAP1", true, "bottom"),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=J1",
                    "panel=1::unit=1",
                    ("J1", "CONN_MFR1", "CONN1", true, "top"),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R1",
                    "panel=1::unit=1",
                    ("R1", "RES_MFR1", "RES1", true, "top"),
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
            ctx.name_arg.as_str(),
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
            "New placement. placement: Placement { ref_des: \"R1\", part: Part { manufacturer: \"RES_MFR1\", mpn: \"RES1\" }, place: true, pcb_side: Top }\n",
            "New placement. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom }\n",
            "New placement. placement: Placement { ref_des: \"J1\", part: Part { manufacturer: \"CONN_MFR1\", mpn: \"CONN1\" }, place: true, pcb_side: Top }\n",
        ]);

        // and
        let project_content: String = read_to_string(ctx.test_project_path.clone())?;
        println!("{}", project_content);

        assert_eq!(project_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_3_assign_process_to_parts() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(3);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        let design_a_variant_a_placements_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide"
            "R1","RES_MFR1","RES1","true","Top"
            "R2","RES_MFR2","RES2","true","Top"
            "J1","CONN_MFR1","CONN1","true","Top"
        "#};

        let mut placements_path = ctx.temp_dir.path().to_path_buf();
        placements_path.push("design_a_variant_a_placements.csv");

        let mut placments_file = File::create(placements_path)?;
        placments_file.write(design_a_variant_a_placements_csv_content.as_bytes())?;
        placments_file.flush()?;

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_unit_assignments(&[
                (
                    "panel=1::unit=1",
                    BTreeMap::from([
                        ("design_name", "design_a"),
                        ("variant_name", "variant_a"),
                    ])
                )
            ])
            .with_processes(&["pnp", "manual"])
            .with_part_states(&[
                (("CONN_MFR1", "CONN1"), &["pnp"]),
                (("RES_MFR1", "RES1"), &["pnp"]),
                (("RES_MFR2", "RES2"), &["pnp"]),
            ])
            .with_placements(&[
                (
                    "panel=1::unit=1::ref_des=C1",
                    "panel=1::unit=1",
                    ("C1", "CAP_MFR1", "CAP1", true, "bottom"),
                    false,
                    "Unknown",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=J1",
                    "panel=1::unit=1",
                    ("J1", "CONN_MFR1", "CONN1", true, "top"),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R1",
                    "panel=1::unit=1",
                    ("R1", "RES_MFR1", "RES1", true, "top"),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R2",
                    "panel=1::unit=1",
                    ("R2", "RES_MFR2", "RES2", true, "top"),
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
            ctx.name_arg.as_str(),
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
            "New placement. placement: Placement { ref_des: \"R2\", part: Part { manufacturer: \"RES_MFR2\", mpn: \"RES2\" }, place: true, pcb_side: Top }\n",
            "Marking placement as unused. placement: Placement { ref_des: \"C1\", part: Part { manufacturer: \"CAP_MFR1\", mpn: \"CAP1\" }, place: true, pcb_side: Bottom }\n",
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
    fn sequence_4_create_phase() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(4);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_unit_assignments(&[
                (
                    "panel=1::unit=1",
                    BTreeMap::from([
                        ("design_name", "design_a"),
                        ("variant_name", "variant_a"),
                    ])
                )
            ])
            .with_processes(&["pnp", "manual"])
            .with_part_states(&[
                (("CONN_MFR1", "CONN1"), &["pnp"]),
                (("RES_MFR1", "RES1"), &["pnp"]),
                (("RES_MFR2", "RES2"), &["pnp"]),
            ])
            .with_phases(
                &[
                    ("top_1", "pnp", ctx.phase_1_load_out_path.to_str().unwrap(), "top")
                ]
            )
            .with_placements(&[
                (
                    "panel=1::unit=1::ref_des=C1",
                    "panel=1::unit=1",
                    ("C1", "CAP_MFR1", "CAP1", true, "bottom"),
                    false,
                    "Unknown",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=J1",
                    "panel=1::unit=1",
                    ("J1", "CONN_MFR1", "CONN1", true, "top"),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R1",
                    "panel=1::unit=1",
                    ("R1", "RES_MFR1", "RES1", true, "top"),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R2",
                    "panel=1::unit=1",
                    ("R2", "RES_MFR2", "RES2", true, "top"),
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
            ctx.name_arg.as_str(),
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
    fn sequence_5_assign_placements_to_phase() -> Result<(), anyhow::Error> {
        // given
        let mut ctx_guard = context::aquire(5);
        let ctx = ctx_guard.1.as_mut().unwrap();

        // and
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_project_content = TestProjectBuilder::new()
            .with_name("job1")
            .with_unit_assignments(&[
                (
                    "panel=1::unit=1",
                    BTreeMap::from([
                        ("design_name", "design_a"),
                        ("variant_name", "variant_a"),
                    ])
                )
            ])
            .with_processes(&["pnp", "manual"])
            .with_part_states(&[
                (("CONN_MFR1", "CONN1"), &["pnp"]),
                (("RES_MFR1", "RES1"), &["pnp"]),
                (("RES_MFR2", "RES2"), &["pnp"]),
            ])
            .with_phases(
                &[
                    ("top_1", "pnp", ctx.phase_1_load_out_path.to_str().unwrap(), "top")
                ]
            )
            .with_placements(&[
                (
                    "panel=1::unit=1::ref_des=C1",
                    "panel=1::unit=1",
                    ("C1", "CAP_MFR1", "CAP1", true, "bottom"),
                    false,
                    "Unknown",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=J1",
                    "panel=1::unit=1",
                    ("J1", "CONN_MFR1", "CONN1", true, "top"),
                    false,
                    "Known",
                    None,
                ),
                (
                    "panel=1::unit=1::ref_des=R1",
                    "panel=1::unit=1",
                    ("R1", "RES_MFR1", "RES1", true, "top"),
                    false,
                    "Known",
                    Some("top_1"),
                ),
                (
                    "panel=1::unit=1::ref_des=R2",
                    "panel=1::unit=1",
                    ("R2", "RES_MFR2", "RES2", true, "top"),
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
            ctx.name_arg.as_str(),
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
    fn sequence_6_cleanup() {
        let mut ctx_guard = context::aquire(6);
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

    #[test]
    fn no_args() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Usage: planner [OPTIONS] --name=<NAME> [COMMAND]

            Commands:
              create                      Create a new job
              assign-variant-to-unit      Assign a design variant to a PCB unit
              assign-process-to-parts     Assign a process to parts
              create-phase                Create a phase
              assign-placements-to-phase  Assign placements to a phase
              help                        Print this message or the help of the given subcommand(s)

            Options:
                  --trace[=<TRACE>]  Trace log file
                  --path=<PATH>      Path [default: .]
                  --name=<NAME>      Job name
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

    #[test]
    fn help_for_create() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Create a new job

            Usage: planner --name=<NAME> create

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
    fn help_for_assign_variant_to_unit() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Assign a design variant to a PCB unit

            Usage: planner --name=<NAME> assign-variant-to-unit --design=<DESIGN_NAME> --variant=<VARIANT_NAME> --unit=<UNIT_PATH>

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

            Usage: planner --name=<NAME> assign-process-to-parts --process=<PROCESS> --manufacturer=<MANUFACTURER> --mpn=<MPN>

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

            Usage: planner --name=<NAME> create-phase --process=<PROCESS> --reference=<REFERENCE> --load-out=<LOAD_OUT> --pcb-side=<PCB_SIDE>

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

            Usage: planner --name=<NAME> assign-placements-to-phase --phase=<PHASE> --placements=<PLACEMENTS>

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
}

#[derive(Default)]
struct TestProjectBuilder<'a> {
    name: Option<&'a str>,
    processes: Option<&'a [&'a str]>,
    unit_assignments: Option<&'a[(&'a str, BTreeMap<&'a str, &'a str>)]>,
    part_states: Option<&'a [((&'a str, &'a str), &'a [&'a str])]>,
    placements: Option<&'a [
        (&'a str, &'a str, (
            &'a str, &'a str, &'a str, bool, &'a str
        ), bool, &'a str, Option<&'a str>)
    ]>,
    phases: Option<&'a [(&'a str, &'a str, &'a str, &'a str)]>,
}

impl<'a> TestProjectBuilder<'a> {
    pub fn content(&self) -> String {
        let mut root = json!({});
        
        if let Some(name) = self.name {
            root["name"] = Value::String(name.to_string());
        }

        if let Some(unit_assignments) = self.unit_assignments {
            
            let values: Vec<Value> = unit_assignments.iter().map(|(key, values)|{
                Value::Array(vec![
                    Value::String(key.to_string()),
                    Value::Object(values.iter().fold(Map::new(), | mut map, (k,v)| {
                        map.insert(k.to_string(), Value::String(v.to_string()));
                        
                        map
                    })),
                ])
            }).collect();
                        
            root["unit_assignments"] = Value::Array(values);
        }

        if let Some(processes) = self.processes {
            root["processes"] = Value::Array(processes
                .to_vec().iter()
                .map(|process|Value::String(process.to_string())).collect()
            );
        }

        if let Some(part_states) = self.part_states {
            let values: Vec<Value> = part_states.iter().map(|((manufacturer, mpn), applicable_processes)|{

                let mut part_map = Map::new();
                part_map.insert("manufacturer".to_string(), Value::String(manufacturer.to_string()));
                part_map.insert("mpn".to_string(), Value::String(mpn.to_string()));

                
                let mut state_map = Map::new();
                if !applicable_processes.is_empty() {
                    state_map.insert("applicable_processes".to_string(), Value::Array(applicable_processes
                        .to_vec().iter()
                        .map(|process| Value::String(process.to_string())).collect()
                    ));
                }
                
                Value::Array(vec![
                    Value::Object(part_map),
                    Value::Object(state_map)
                ])
            }).collect();

            root["part_states"] = Value::Array(values);
        }
        
        if let Some(phases) = self.phases {
            let values: Vec<Value> = phases.iter().map(|(reference, process, load_out_source, pcb_side)|{
                
                let mut phase_map = Map::new();
                phase_map.insert("reference".to_string(), Value::String(reference.to_string()));
                phase_map.insert("process".to_string(), Value::String(process.to_string()));
                phase_map.insert("load_out".to_string(), Value::String(load_out_source.to_string()));
                phase_map.insert("pcb_side".to_string(), Value::String(pcb_side.to_string()));
                
                Value::Array(vec![
                    Value::String(reference.to_string()),
                    Value::Object(phase_map),
                ])
            }).collect();
            root["phases"] = Value::Array(values);
        }
        
        if let Some(placements) = self.placements {
            
            let values: Vec<Value> = placements.iter().map(|(
                key,
                unit_path, (
                    ref_des, manufacturer, mpn, place, pcb_side
                ),
                placed,
                status,
                phase,
            ) | {
                
                let mut part_map = Map::new();
                part_map.insert("manufacturer".to_string(), Value::String(manufacturer.to_string()));
                part_map.insert("mpn".to_string(), Value::String(mpn.to_string()));

                let mut placement_map = Map::new();
                placement_map.insert("ref_des".to_string(), Value::String(ref_des.to_string()));
                placement_map.insert("part".to_string(), Value::Object(part_map));
                placement_map.insert("place".to_string(), Value::Bool(*place));
                placement_map.insert("pcb_side".to_string(), Value::String(pcb_side.to_string()));

                let mut placement_state_map = Map::new();
                placement_state_map.insert("unit_path".to_string(), Value::String(unit_path.to_string()));
                placement_state_map.insert("placement".to_string(), Value::Object(placement_map));
                placement_state_map.insert("placed".to_string(), Value::Bool(*placed));
                placement_state_map.insert("status".to_string(), Value::String(status.to_string()));
                
                if let Some(phase) = phase {
                    placement_state_map.insert("phase".to_string(), Value::String(phase.to_string()));
                }

                Value::Array(vec![
                    Value::String(key.to_string()),
                    Value::Object(placement_state_map),
                ])
            }).collect();
            
            root["placements"] = Value::Array(values);
        }

        let mut buffer = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut ser = serde_json::Serializer::with_formatter(&mut buffer, formatter);

        root.serialize(&mut ser).expect("TODO");

        let mut content = String::from_utf8(buffer).unwrap();
        content.push('\n');

        content
    }

    pub fn with_phases(mut self, phases: &'a [(&'a str, &'a str, &'a str, &'a str)]) -> Self {
        self.phases = Some(phases);
        self
    }
    
    pub fn with_placements(mut self, placements: &'a [
        (&'a str, &'a str, (
            &'a str, &'a str, &'a str, bool, &'a str
        ), bool, &'a str, Option<&'a str>)
    ]) -> Self {
        self.placements = Some(placements);
        self
    }

    pub fn with_part_states(mut self, part_states: &'a [((&'a str, &'a str), &'a [&'a str])]) -> Self {
        self.part_states = Some(part_states);
        self
    }

    pub fn with_unit_assignments(mut self, unit_assignments: &'a [(&'a str, BTreeMap<&'a str, &'a str>)]) -> Self {
        self.unit_assignments = Some(unit_assignments);
        self
    }
    pub fn with_processes(mut self, processes: &'a [&'a str]) -> Self {
        self.processes = Some(processes);
        self
    }

    pub fn with_name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    pub fn new() -> Self {
        Default::default()
    }
}