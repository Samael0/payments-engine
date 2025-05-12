pub mod engine;
pub mod models;
pub mod error;
mod processor;

// Re-export main processing function for convenience
pub use processor::process_transactions;