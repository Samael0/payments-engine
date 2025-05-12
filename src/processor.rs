use crate::engine::PaymentEngine;
use crate::models::Transaction;
use anyhow::Result;
use csv::Writer;
use futures::stream::StreamExt;
use std::path::Path;
use std::time::Instant;
use std::io::Write;
use tokio::fs::File;
use tokio::io::{AsyncRead, BufReader};
use tokio_stream::wrappers::LinesStream;
use tracing::{error, info};

/// Process transactions from a CSV file and output account balances
pub async fn process_transactions(file_path: &Path) -> Result<()> {
    info!("Processing transactions from: {:?}", file_path);
    
    // Track processing time
    let start_time = Instant::now();
    
    // Create a new payment engine
    let mut engine = PaymentEngine::new();
    
    // Process transactions in streaming fashion
    process_transactions_stream(file_path, &mut engine).await?;
    
    // Calculate elapsed time
    let duration = start_time.elapsed();
    
    // Write results to stdout (with duration at the top)
    write_account_balances(&engine, duration)?;
    
    Ok(())
}

/// Process transactions from a CSV file as a stream
async fn process_transactions_stream(file_path: &Path, engine: &mut PaymentEngine) -> Result<()> {
    // Open the file
    let file = File::open(file_path).await?;
    let reader = BufReader::new(file);
    
    // Create a stream of CSV lines
    let lines_stream = create_csv_line_stream(reader);
    
    // Skip the header line
    let mut lines = lines_stream.skip(1);
    
    // Process each line as it comes in
    let mut line_count = 0;
    while let Some(line_result) = lines.next().await {
        match line_result {
            Ok(line) => {
                line_count += 1;
                
                // Parse the transaction
                match parse_transaction(&line) {
                    Ok(transaction) => {
                        // Process the transaction
                        if let Err(e) = engine.process_transaction(transaction).await {
                            error!("Failed to process transaction on line {}: {}", line_count, e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse transaction on line {}: {}", line_count, e);
                    }
                }
            }
            Err(e) => {
                error!("Error reading line {}: {}", line_count + 1, e);
            }
        }
    }
    
    info!("Processed {} transactions", line_count);
    
    Ok(())
}

/// Create a stream of CSV lines from a reader
fn create_csv_line_stream<R: AsyncRead + Unpin + 'static>(
    reader: BufReader<R>,
) -> impl futures::Stream<Item = Result<String, std::io::Error>> {
    LinesStream::new(tokio::io::AsyncBufReadExt::lines(reader))
}

/// Parse a CSV line into a Transaction
fn parse_transaction(line: &str) -> Result<Transaction> {
    // Split the line by commas
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    
    // Ensure we have the required fields (type, client, tx, [amount])
    if parts.len() < 3 {
        anyhow::bail!("Invalid CSV line format: {}", line);
    }
    
    // Parse the CSV fields
    let transaction_type = match parts[0] {
        "deposit" => crate::models::TransactionType::Deposit,
        "withdrawal" => crate::models::TransactionType::Withdrawal,
        "dispute" => crate::models::TransactionType::Dispute,
        "resolve" => crate::models::TransactionType::Resolve,
        "chargeback" => crate::models::TransactionType::Chargeback,
        _ => anyhow::bail!("Invalid transaction type: {}", parts[0]),
    };
    
    let client: u16 = parts[1].parse()?;
    let tx: u32 = parts[2].parse()?;
    
    // Amount is optional (not present for dispute, resolve, chargeback)
    let amount = if parts.len() > 3 && !parts[3].is_empty() {
        Some(parts[3].parse()?)
    } else {
        None
    };
    
    Ok(Transaction {
        transaction_type,
        client,
        tx,
        amount,
    })
}

/// Write account balances to stdout as CSV
fn write_account_balances(engine: &PaymentEngine, duration: std::time::Duration) -> Result<()> {
    let accounts = engine.get_accounts();
    
    // Create a CSV writer to stdout
    let mut writer = Writer::from_writer(std::io::stdout());
    
    // Write the processing time as a comment at the top of the CSV
    writeln!(
        std::io::stdout(),
        "# Processing completed in {:.2?}",
        duration
    )?;
    
    // Format accounts to ensure 4 decimal places for monetary values
    for mut account in accounts {
        // Scale to 4 decimal places
        account.available = account.available.round_dp(4);
        account.held = account.held.round_dp(4);
        account.total = account.total.round_dp(4);
        
        // Serialize to CSV
        writer.serialize(account)?;
    }
    
    writer.flush()?;
    
    Ok(())
}
