use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::fs;
use chrono::Local;
use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

use payment_engine::{process_transactions_with_options, ProcessingOptions};

#[derive(Parser, Debug)]
#[command(about = "A payment transaction processor")]
struct Args {
    /// Input CSV file with transactions
    #[arg(name = "FILE")]
    input_file: PathBuf,

    /// Log directory (defaults to logs/)
    #[arg(long, default_value = "logs")]
    log_dir: PathBuf,
    
    /// Batch size for processing transactions (default: 1000)
    #[arg(long, default_value = "1000")]
    batch_size: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Create logs directory if it doesn't exist
    if !args.log_dir.exists() {
        fs::create_dir_all(&args.log_dir)?;
    }
    
    // Generate log filename with current datetime
    let datetime = Local::now().format("%Y%m%d_%H%M%S");
    let log_file = args.log_dir.join(format!("payment_engine_{}.log", datetime));
    
    // Initialize logging to a file
    let file_appender = tracing_appender::rolling::never(&args.log_dir, log_file.file_name().unwrap_or_default());
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    registry()
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
        )
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();
    
    // Configure processing options
    let options = ProcessingOptions {
        batch_size: args.batch_size,
    };
    
    // Process the transactions and output results
    process_transactions_with_options(&args.input_file, options).await?;
    
    Ok(())
}