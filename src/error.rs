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

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::io;
    
    #[test]
    fn test_file_read_error() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let error = PaymentEngineError::FileReadError(io_error);
        
        assert!(error.to_string().contains("Failed to read file"));
        assert!(error.source().is_some());
        
        // Test From trait implementation
        let error_from: PaymentEngineError = io::Error::new(io::ErrorKind::NotFound, "file not found").into();
        match error_from {
            PaymentEngineError::FileReadError(_) => assert!(true),
            _ => panic!("Wrong error variant"),
        }
    }
    
    #[test]
    fn test_csv_error() {
        // Generate a CSV error by trying to deserialize an invalid string
        let reader = csv::Reader::from_reader("type,client,tx\ndeposit,bad,1".as_bytes());
        let csv_error = reader.into_deserialize::<(String, u16, u32)>().next().unwrap().unwrap_err();
        
        let error = PaymentEngineError::CsvError(csv_error);
        
        assert!(error.to_string().contains("CSV error"));
        assert!(error.source().is_some());
        
        // Test From trait implementation
        let reader = csv::Reader::from_reader("type,client,tx\ndeposit,bad,1".as_bytes());
        let csv_error = reader.into_deserialize::<(String, u16, u32)>().next().unwrap().unwrap_err();
        let error_from: PaymentEngineError = csv_error.into();
        
        match error_from {
            PaymentEngineError::CsvError(_) => assert!(true),
            _ => panic!("Wrong error variant"),
        }
    }
    
    #[test]
    fn test_missing_amount() {
        let tx_id = 12345;
        let error = PaymentEngineError::MissingAmount(tx_id);
        
        assert!(error.to_string().contains("Missing amount for transaction 12345"));
        assert!(error.source().is_none()); // No source for this error type
    }
}