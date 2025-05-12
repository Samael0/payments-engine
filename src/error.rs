use thiserror::Error;

#[derive(Error, Debug)]
pub enum PaymentEngineError {
    #[error("Failed to read file: {0}")]
    FileReadError(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    CsvError(#[from] csv::Error),
    
    #[error("Missing amount for transaction {0}")]
    MissingAmount(u32),
}