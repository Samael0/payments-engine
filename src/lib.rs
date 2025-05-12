pub mod engine;
pub mod models;
pub mod error;
mod processor;

// Re-export main processing functions for convenience
pub use processor::{process_transactions, process_transactions_with_options, ProcessingOptions};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;
    use std::fs::write;
    
    #[tokio::test]
    async fn test_integration_process_transactions() {
        // Create a temporary directory
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("integration_test.csv");
        
        // Create a test CSV file with various transaction types
        let csv_content = "type,client,tx,amount\n\
                          deposit,1,1,100.0\n\
                          deposit,2,2,200.0\n\
                          withdrawal,1,3,30.0\n\
                          deposit,1,4,50.0\n\
                          dispute,1,1,\n\
                          deposit,2,5,300.0\n\
                          withdrawal,2,6,100.0\n\
                          resolve,1,1,\n\
                          deposit,3,7,500.0\n\
                          withdrawal,3,8,100.0\n\
                          dispute,3,7,\n\
                          chargeback,3,7,\n";
                          
        write(&file_path, csv_content).unwrap();
        
        // Process the transactions with a small batch size for testing
        let options = ProcessingOptions {
            batch_size: 5,  // Use a small batch size for testing
        };
        process_transactions_with_options(Path::new(&file_path), options).await.unwrap();
        
        // Note: Since process_transactions writes to stdout, we can't easily capture
        // the output in this test. In a real-world scenario, we might want to
        // modify the API to return the results instead of writing to stdout directly
        // for better testability.
    }
    
    #[tokio::test]
    async fn test_integration_with_errors() {
        // Create a temporary directory
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("error_test.csv");
        
        // Create a test CSV file with some invalid transactions
        let csv_content = "type,client,tx,amount\n\
                          deposit,1,1,100.0\n\
                          withdrawal,1,2,200.0\n\
                          invalid,1,3,50.0\n\
                          deposit,abc,4,50.0\n\
                          deposit,2,5,abc\n\
                          deposit,3,6,100.0\n";
                          
        write(&file_path, csv_content).unwrap();
        
        // Process should complete without panic even with errors
        // Using a custom batch size to test the batch processing with errors
        let options = ProcessingOptions {
            batch_size: 2,  // Small batch size to test error handling in batches
        };
        process_transactions_with_options(Path::new(&file_path), options).await.unwrap();
    }
}