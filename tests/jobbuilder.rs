#[macro_use]
extern crate makerpnp;

#[path = "inc/int_test.rs"]
pub mod int_test;

#[cfg(feature="cli")]
mod tests {
    use assert_cmd::Command;
    use indoc::indoc;
    use predicates::prelude::{predicate, PredicateBooleanExt};
    use crate::int_test::print;

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