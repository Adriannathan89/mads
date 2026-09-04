use std::{
    fs::OpenOptions,
    io::Write,
};

use mads::prelude::*;

#[mads::routes]
trait HealthRoutes {
    #[mads::get("/health")]
    async fn health(&self) -> &'static str;
}

#[mads::controller(routes = [HealthRoutes])]
struct HealthController;

impl HealthRoutes for HealthController {
    async fn health(&self) -> &'static str {
        "healthy"
    }
}

#[mads::module]
struct AppModule;

#[mads::main]
async fn main() -> Result<(), HttpRuntimeError> {
    let path = std::env::var("MADS_TEST_START_LOG")
        .expect("dev-loop test should provide a start log path");
    let arguments = std::env::args().skip(1).collect::<Vec<_>>().join("|");
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("start log should be writable");
    writeln!(log, "{}|{arguments}", std::process::id()).expect("start log entry should write");
    Mads::run::<AppModule>().await
}
