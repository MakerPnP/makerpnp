use std::ffi::OsString;
use std::path::PathBuf;
use predicates::function::FnPredicate;
use predicates::prelude::predicate;
use tempfile::TempDir;
#[cfg(test)]
pub mod lock;


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

pub fn prepare_args<'a>(args: Vec<&'a str>) -> Vec<&'a str> {
    args.iter().fold(vec![], |mut args: Vec<&str>, arg| {
        for &arg in arg.split(" ").collect::<Vec<&str>>().iter() {
            args.push(arg);
        }
        args
    })
}
