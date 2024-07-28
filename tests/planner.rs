#[macro_use]
extern crate makerpnp;

#[path = "inc/int_test.rs"]
pub mod int_test;

#[cfg(feature="cli")]
mod operation_sequence_1 {
    use std::fs::read_to_string;
    use std::path::PathBuf;
    use assert_cmd::Command;
    use indoc::indoc;
    use tempfile::tempdir;
    use crate::int_test::{build_temp_file, print};

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
            pub test_trace_log_path: PathBuf,
        }

        impl Context {
            pub fn new() -> Self {
                let temp_dir = tempdir().unwrap();

                let path_arg = format!("--path={}", temp_dir.path().to_str().unwrap());

                let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
                let trace_log_arg = format!("--trace={}", test_trace_log_file_name.to_str().unwrap());

                Context {
                    temp_dir,
                    path_arg,
                    trace_log_arg,
                    test_trace_log_path,
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
        let (test_project_path, _test_project_file_name) = build_temp_file(&ctx.temp_dir, "project-job1", "mpnp.json");
        let expected_project_content = indoc! {r#"
            {
                "name": "job1",
                "unit_assignments": []
            }
        "#};

        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            "--name=job1",
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
        let job_status_content: String = read_to_string(test_project_path.clone())?;
        println!("{}", job_status_content);

        assert_eq!(job_status_content, expected_project_content);

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
        let (test_project_path, _test_project_file_name) = build_temp_file(&ctx.temp_dir, "project-job1", "mpnp.json");
        let expected_project_content = indoc! {r#"
            {
                "name": "job1",
                "unit_assignments": [
                    {
                        "unit_path": "panel:1:unit:1",
                        "design_name": "design_a",
                        "variant_name": "variant_a"
                    }
                ]
            }
        "#};

        // and
        let args = [
            ctx.trace_log_arg.as_str(),
            ctx.path_arg.as_str(),
            "--name=job1",
            "assign-variant-to-unit",
            "--design=design_a",
            "--variant=variant_a",
            "--unit=panel:1:unit:1",
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
            "Assignment created. unit: panel:1:unit:1, design: design_a, variant: variant_a\n",
        ]);

        // and
        let job_status_content: String = read_to_string(test_project_path.clone())?;
        println!("{}", job_status_content);

        assert_eq!(job_status_content, expected_project_content);

        Ok(())
    }

    #[test]
    fn sequence_3_cleanup() {
        let mut ctx_guard = context::aquire(3);
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
              create                  Create a new job
              assign-variant-to-unit  Assign a design variant to a PCB unit
              help                    Print this message or the help of the given subcommand(s)

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
}