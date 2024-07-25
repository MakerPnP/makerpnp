#[macro_use]
extern crate makerpnp;

#[path = "inc/int_test.rs"]
pub mod int_test;

#[cfg(feature="cli")]
mod tests {
    use std::fs::read_to_string;
    use assert_cmd::Command;
    use indoc::indoc;
    use predicates::prelude::{predicate, PredicateBooleanExt};
    use tempfile::tempdir;
    use crate::int_test::{build_temp_file, print};

    #[test]
    fn create_job() -> Result<(), std::io::Error> {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let temp_dir = tempdir()?;

        // and
        let path_arg = format!("--path={}", temp_dir.path().to_str().unwrap());

        // and
        let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
        let trace_log_arg = format!("--trace={}", test_trace_log_file_name.to_str().unwrap());

        // and
        let (test_job_status_path, _test_job_status_file_name) = build_temp_file(&temp_dir, "project-job1", "mpnp.json");
        let expected_job_status_content = indoc!{r#"
            {
                "name": "job1"
            }
        "#};

        // and
        let args = [
            trace_log_arg.as_str(),
            path_arg.as_str(),
            "create",
            "--name=job1",
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
        let trace_content: String = read_to_string(test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Created job: job1\n",
        ]);

        // and
        let job_status_content: String = read_to_string(test_job_status_path.clone())?;
        println!("{}", job_status_content);

        assert_eq!(job_status_content, expected_job_status_content);


        Ok(())
    }

    #[test]
    fn no_args() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planner"));

        // and
        let expected_output = indoc! {"
            Usage: planner [OPTIONS] [COMMAND]

            Commands:
              create  Create a new job
              help    Print this message or the help of the given subcommand(s)

            Options:
                  --trace[=<TRACE>]  Trace log file
                  --path=<PATH>      Path [default: .]
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

            Usage: planner create --name=<NAME>

            Options:
                  --name=<NAME>  Job name
              -h, --help         Print help
        "};

        // when
        cmd.args(["create", "--help"])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }
}