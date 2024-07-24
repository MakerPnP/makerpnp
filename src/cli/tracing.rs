use std::fs::File;
use std::path::PathBuf;
use tracing_subscriber::fmt::Subscriber as FmtSubscriber;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;

pub fn configure_tracing(path: Option<PathBuf>) -> anyhow::Result<()> {
    const SUBSCRIBER_FAILED_MESSAGE: &'static str = "setting default subscriber failed";
    match path {
        Some(path) => {
            //println!("using file_subscriber");
            let trace_file: File = File::create(path)?;

            let file_subscriber = FmtSubscriber::builder()
                .with_writer(trace_file)
                .with_max_level(Level::TRACE)
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
                .with_max_level(Level::INFO)
                .finish();

            tracing::subscriber::set_global_default(stdout_subscriber)
                .expect(SUBSCRIBER_FAILED_MESSAGE);
        }
    };

    Ok(())
}