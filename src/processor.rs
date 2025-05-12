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

// Default batch size for transaction processing
const DEFAULT_BATCH_SIZE: usize = 1000;

/// Processing options for transaction handling
pub struct ProcessingOptions {
    /// Batch size for processing transactions
    pub batch_size: usize,
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

/// Process transactions from a CSV file and output account balances
pub async fn process_transactions(file_path: &Path) -> Result<()> {
    // Use default options
    process_transactions_with_options(file_path, ProcessingOptions::default()).await
}

/// Process transactions from a CSV file with custom options
pub async fn process_transactions_with_options(file_path: &Path, options: ProcessingOptions) -> Result<()> {
    info!("Processing transactions from: {:?} with batch size: {}", file_path, options.batch_size);
    
    // Track processing time
    let start_time = Instant::now();
    
    // Create a new payment engine
    let mut engine = PaymentEngine::new();
    
    // Process transactions in streaming fashion
    process_transactions_stream(file_path, &mut engine, options.batch_size).await?;
    
    // Calculate elapsed time
    let duration = start_time.elapsed();
    
    // Write results to stdout (with duration at the top)
    write_account_balances(&engine, duration)?;
    
    Ok(())
}

/// Process transactions from a CSV file as a stream
async fn process_transactions_stream(file_path: &Path, engine: &mut PaymentEngine, batch_size: usize) -> Result<()> {
    // Open the file
    let file = File::open(file_path).await?;
    let reader = BufReader::new(file);
    
    // Create a stream of CSV lines
    let lines_stream = create_csv_line_stream(reader);
    
    // Skip the header line
    let mut lines = lines_stream.skip(1);
    
    // Process transactions in batches
    let mut line_count = 0;
    let mut batch = Vec::with_capacity(batch_size);
    
    while let Some(line_result) = lines.next().await {
        match line_result {
            Ok(line) => {
                line_count += 1;
                
                // Parse the transaction
                match parse_transaction(&line) {
                    Ok(transaction) => {
                        // Add to batch
                        batch.push(transaction);
                        
                        // Process batch if it reaches the specified size
                        if batch.len() >= batch_size {
                            if let Err(e) = engine.process_transaction_batch(&mut batch).await {
                                error!("Failed to process transaction batch: {}", e);
                            }
                            // Clear the batch for next iterations
                            batch.clear();
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
    
    // Process any remaining transactions in the last batch
    if !batch.is_empty() {
        if let Err(e) = engine.process_transaction_batch(&mut batch).await {
            error!("Failed to process final transaction batch: {}", e);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{TransactionType};
    use rust_decimal_macros::dec;
    use tempfile::tempdir;
    use std::fs::write;
    
    #[test]
    fn test_parse_transaction_deposit() {
        let line = "deposit,1,1,100.50";
        let tx = parse_transaction(line).unwrap();
        
        assert_eq!(tx.transaction_type, TransactionType::Deposit);
        assert_eq!(tx.client, 1);
        assert_eq!(tx.tx, 1);
        assert_eq!(tx.amount, Some(dec!(100.50)));
    }
    
    #[test]
    fn test_parse_transaction_withdrawal() {
        let line = "withdrawal,2,5,20.75";
        let tx = parse_transaction(line).unwrap();
        
        assert_eq!(tx.transaction_type, TransactionType::Withdrawal);
        assert_eq!(tx.client, 2);
        assert_eq!(tx.tx, 5);
        assert_eq!(tx.amount, Some(dec!(20.75)));
    }
    
    #[test]
    fn test_parse_transaction_dispute() {
        let line = "dispute,1,10,";
        let tx = parse_transaction(line).unwrap();
        
        assert_eq!(tx.transaction_type, TransactionType::Dispute);
        assert_eq!(tx.client, 1);
        assert_eq!(tx.tx, 10);
        assert_eq!(tx.amount, None);
    }
    
    #[test]
    fn test_parse_transaction_resolve() {
        let line = "resolve,3,15";
        let tx = parse_transaction(line).unwrap();
        
        assert_eq!(tx.transaction_type, TransactionType::Resolve);
        assert_eq!(tx.client, 3);
        assert_eq!(tx.tx, 15);
        assert_eq!(tx.amount, None);
    }
    
    #[test]
    fn test_parse_transaction_chargeback() {
        let line = "chargeback,4,20";
        let tx = parse_transaction(line).unwrap();
        
        assert_eq!(tx.transaction_type, TransactionType::Chargeback);
        assert_eq!(tx.client, 4);
        assert_eq!(tx.tx, 20);
        assert_eq!(tx.amount, None);
    }
    
    #[test]
    fn test_parse_transaction_invalid_type() {
        let line = "unknown,1,1,100";
        let result = parse_transaction(line);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_parse_transaction_invalid_format() {
        let line = "deposit,1";
        let result = parse_transaction(line);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_parse_transaction_invalid_client() {
        let line = "deposit,abc,1,100";
        let result = parse_transaction(line);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_parse_transaction_invalid_tx() {
        let line = "deposit,1,abc,100";
        let result = parse_transaction(line);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_parse_transaction_invalid_amount() {
        let line = "deposit,1,1,abc";
        let result = parse_transaction(line);
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_process_transactions_integration() {
        // Create a temporary directory
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_transactions.csv");
        
        // Create a test CSV file
        let csv_content = "type,client,tx,amount\n\
                          deposit,1,1,100.0\n\
                          deposit,2,2,200.0\n\
                          withdrawal,1,3,50.0\n\
                          withdrawal,2,4,25.0\n";
                          
        write(&file_path, csv_content).unwrap();
        
        // Process the file
        let mut engine = PaymentEngine::new();
        process_transactions_stream(&file_path, &mut engine, DEFAULT_BATCH_SIZE).await.unwrap();
        
        // Check the results
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 2);
        
        // Find each client's account
        let client1 = accounts.iter().find(|a| a.client == 1).unwrap();
        let client2 = accounts.iter().find(|a| a.client == 2).unwrap();
        
        assert_eq!(client1.available, dec!(50.0));
        assert_eq!(client1.total, dec!(50.0));
        
        assert_eq!(client2.available, dec!(175.0));
        assert_eq!(client2.total, dec!(175.0));
    }
    
    #[tokio::test]
    async fn test_process_transactions_with_dispute() {
        // Create a temporary directory
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_disputes.csv");
        
        // Create a test CSV file with disputes
        let csv_content = "type,client,tx,amount\n\
                          deposit,1,1,100.0\n\
                          dispute,1,1,\n\
                          resolve,1,1,\n\
                          deposit,2,2,200.0\n\
                          dispute,2,2,\n\
                          chargeback,2,2,\n";
                          
        write(&file_path, csv_content).unwrap();
        
        // Process the file
        let mut engine = PaymentEngine::new();
        process_transactions_stream(&file_path, &mut engine, DEFAULT_BATCH_SIZE).await.unwrap();
        
        // Check the results
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 2);
        
        // Find each client's account
        let client1 = accounts.iter().find(|a| a.client == 1).unwrap();
        let client2 = accounts.iter().find(|a| a.client == 2).unwrap();
        
        // Client 1 - deposit was disputed then resolved, so back to original
        assert_eq!(client1.available, dec!(100.0));
        assert_eq!(client1.held, dec!(0.0));
        assert_eq!(client1.total, dec!(100.0));
        assert!(!client1.locked);
        
        // Client 2 - deposit was disputed then chargebacked, so account is locked
        assert_eq!(client2.available, dec!(0.0));
        assert_eq!(client2.held, dec!(0.0));
        assert_eq!(client2.total, dec!(0.0));
        assert!(client2.locked);
    }

    // Test with different batch sizes
    #[tokio::test]
    async fn test_batch_processing() {
        // Create a temporary directory
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_batch.csv");
        
        // Create a test CSV file with multiple transactions
        let mut csv_content = String::from("type,client,tx,amount\n");
        
        // Add 100 deposit transactions
        for i in 1..=100 {
            csv_content.push_str(&format!("deposit,1,{},{}.0\n", i, i));
        }
        
        write(&file_path, csv_content).unwrap();
        
        // Process with small batch size (10)
        let small_batch_size = 10;
        let mut engine1 = PaymentEngine::new();
        process_transactions_stream(&file_path, &mut engine1, small_batch_size).await.unwrap();
        
        // Process with large batch size (50)
        let large_batch_size = 50;
        let mut engine2 = PaymentEngine::new();
        process_transactions_stream(&file_path, &mut engine2, large_batch_size).await.unwrap();
        
        // Results should be the same regardless of batch size
        let accounts1 = engine1.get_accounts();
        let accounts2 = engine2.get_accounts();
        
        assert_eq!(accounts1.len(), 1);
        assert_eq!(accounts2.len(), 1);
        
        let client1 = accounts1.iter().find(|a| a.client == 1).unwrap();
        let client2 = accounts2.iter().find(|a| a.client == 1).unwrap();
        
        // Sum of 1..=100 is 5050
        assert_eq!(client1.available, dec!(5050.0));
        assert_eq!(client1.total, dec!(5050.0));
        assert_eq!(client1.available, client2.available);
        assert_eq!(client1.total, client2.total);
    }
}
