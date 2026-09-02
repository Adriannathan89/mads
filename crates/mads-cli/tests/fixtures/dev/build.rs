use std::{
    fs::OpenOptions,
    io::Write,
};

fn main() {
    let path = std::env::var("MADS_TEST_BUILD_LOG")
        .expect("dev-loop test should provide a build log path");
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("build log should be writable");
    writeln!(log, "build").expect("build log entry should write");
    println!("cargo:rerun-if-changed=src");
}
