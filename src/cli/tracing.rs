use std::fs::File;
use std::path::PathBuf;
use clap_verbosity_flag::{LogLevel, Verbosity};
use tracing_subscriber::fmt::Subscriber as FmtSubscriber;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_log::AsTrace;

pub fn configure_tracing<IL: LogLevel>(path: Option<PathBuf>, verbosity: Verbosity<IL>) -> anyhow::Result<()> {

    const SUBSCRIBER_FAILED_MESSAGE: &'static str = "setting default subscriber failed";
    match path {
        Some(path) => {
            //println!("using file_subscriber");
            let trace_file: File = File::create(path)?;

            let file_subscriber = FmtSubscriber::builder()
                .with_writer(trace_file)
                .with_max_level(verbosity.log_level_filter().as_trace())
                .finish();

            tracing::subscriber::set_global_default(file_subscriber)
                .expect(SUBSCRIBER_FAILED_MESSAGE);
        },
        _ => {
            //println!("using stdout_subscriber");
            let stdout_subscriber = FmtSubscriber::builder()
                .with_level(false)
                .with_line_number(false)
                .with_span_events(FmtSpan::NONE)
                .without_time()
                .with_max_level(verbosity.log_level_filter().as_trace())
                .finish();

            tracing::subscriber::set_global_default(stdout_subscriber)
                .expect(SUBSCRIBER_FAILED_MESSAGE);
        }
    };

    Ok(())
}