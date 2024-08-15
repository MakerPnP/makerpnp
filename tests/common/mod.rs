use predicates::function::FnPredicate;
use predicates::prelude::predicate;
use tempfile::TempDir;
use std::ffi::OsString;
use std::path::PathBuf;

pub mod load_out_builder;
pub mod phase_placement_builder;
pub mod project_builder;
pub mod project_report_builder;

pub fn print(message: &str) -> FnPredicate<fn(&str) -> bool, str> {
    println!("{}:", message);
    predicate::function(|content| {
        println!("{}", content);
        true
    })
}

pub fn build_temp_csv_file(temp_dir: &TempDir, base: &str) -> (PathBuf, OsString) {
    build_temp_file(temp_dir, base, "csv")
}

pub fn build_temp_file(temp_dir: &TempDir, base: &str, extension: &str) -> (PathBuf, OsString) {
    let mut path_buf = temp_dir.path().to_path_buf();
    path_buf.push(format!("{}.{}", base, extension));

    let absolute_path = path_buf.clone().into_os_string();
    println!("{} file: {}",
             base.replace('_', " "),
             absolute_path.to_str().unwrap()
    );

    (path_buf, absolute_path)
}