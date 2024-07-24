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
    fn build() -> Result<(), std::io::Error> {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_jobbuilder"));

        // and
        let temp_dir = tempdir()?;

        // and
        let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
        let trace_log_arg = format!("--trace={}", test_trace_log_file_name.to_str().unwrap());

        // when
        cmd.args([
            trace_log_arg.as_str(),
            "build",
         ])
            // then
            .assert()
            .success()
            .stderr(print("stderr"))
            .stdout(print("stdout"));

        // and
        let trace_content: String = read_to_string(test_trace_log_path.clone())?;
        println!("{}", trace_content);

        assert_contains_inorder!(trace_content, [
            "Build",
        ]);

        Ok(())
    }

    #[test]
    fn no_args() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_jobbuilder"));

        // and
        let expected_output = indoc! {"
            Usage: jobbuilder [OPTIONS] [COMMAND]

            Commands:
              build  Build job
              help   Print this message or the help of the given subcommand(s)

            Options:
                  --trace[=<TRACE>]  Trace log file
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
}