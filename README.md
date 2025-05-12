# Payment Engine

A high-performance, asynchronous transaction processing engine built in Rust. This application reads transaction data from CSV files, processes various types of payment transactions, and outputs account balances.

## Features

- **Asynchronous processing**: Uses Tokio for concurrent operations
- **Memory-efficient streaming**: Processes CSV data as a stream rather than loading it all at once
- **Full transaction support**: Handles deposits, withdrawals, disputes, resolutions, and chargebacks
- **Robust error handling**: Comprehensive error handling with custom error types
- **Precise decimal handling**: Uses rust_decimal for financial calculations with 4 decimal places precision
- **Organized logging**: Automatically saves logs to timestamped files in a logs directory

## Getting Started

### Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))
- Cargo (included with Rust)

### Installation

Clone the repository:
```
git clone https://github.com/yourusername/payment-system-V2.git
cd payment-system-V2
```

Build the project:
```
cargo build --release
```

## Usage

Basic usage:
```
cargo run -- transactions.csv > accounts.csv
```

Where `transactions.csv` is the input file containing the transactions to process and `accounts.csv` will contain the resulting account balances.

Advanced usage with custom log directory:
```
cargo run -- transactions.csv --log-dir=custom_logs > accounts.csv
```

### Command Line Arguments

| Argument | Description | Default |
|----------|-------------|---------|
| `FILE` | Input CSV file with transactions | Required |
| `--log-dir` | Directory where logs will be stored | `logs/` |

## Output

The program produces two outputs:

1. **Account balances (stdout)**: CSV data showing client account states with monetary values rounded to 4 decimal places
2. **Logs (files)**: Detailed transaction processing logs saved to timestamped files in the logs directory

Example accounts.csv output:
```
# Processing completed in 0.03s
client,available,held,total,locked
2,4.5000,0.0000,4.5000,false
1,1.5000,0.0000,1.5000,false
3,0.0000,0.0000,0.0000,true
```

## Design Decisions

### Error Handling

The application uses both `thiserror` and `anyhow` for error handling:

- `thiserror` for defining domain-specific error types (`PaymentEngineError`) with clear error messages
- `anyhow` for general error propagation and combining errors from different sources

This combination provides both strongly typed domain errors and convenient error handling at the application level.

### Data Streaming

Rather than loading the entire CSV file into memory, the application uses Tokio's asynchronous I/O combined with stream processing to handle data efficiently:

```rust
// Create a stream of CSV lines from a reader
fn create_csv_line_stream<R: AsyncRead + Unpin + 'static>(
    reader: BufReader<R>,
) -> impl futures::Stream<Item = Result<String, std::io::Error>> {
    LinesStream::new(tokio::io::AsyncBufReadExt::lines(reader))
}
```

This approach allows the engine to process very large files (with millions of transactions) without excessive memory usage.

### Concurrency

The application uses Tokio's async runtime to process transactions concurrently. This design would allow for processing transactions from multiple CSV files or TCP streams simultaneously with minimal code changes.

### Transaction Storage

Transactions are stored in a `TransactionStore` to support the dispute resolution process. This allows the engine to look up original transactions when processing disputes, resolutions, and chargebacks.

### Logging System

The application implements a structured logging system that:

1. Creates a `logs` directory if it doesn't exist
2. Generates uniquely named log files with timestamps (format: `payment_engine_YYYYMMDD_HHMMSS.log`)
3. Separates logs from CSV output for clean data processing

This approach makes it easy to track each run of the application and review processing details without affecting the CSV output.

### Decimal Precision

The application strictly adheres to the 4 decimal places precision requirement for all monetary values in the output, ensuring consistency in financial calculations.

### Sample Data

The application includes a sample `transactions.csv` file for manual testing that contains examples of all transaction types:

- Deposits and withdrawals
- Disputes, resolutions, and chargebacks
- Cases where transactions should fail (insufficient funds)

## Assumptions

1. Clients and transactions are uniquely identified by their IDs, and these IDs are valid.
2. Transaction amounts are positive decimal values with up to 4 decimal places.
3. Once an account is locked due to a chargeback, it cannot process any further transactions.
4. Withdrawals cannot exceed a client's available balance.
5. A dispute can only reference a deposit or withdrawal that exists and belongs to the same client.
6. Transaction IDs (tx) are globally unique
7. Transactions in the CSV file are chronologically ordered
8. Only deposit transactions can be disputed
9. When a chargeback occurs, the client's account is locked and no further transactions are processed
10. Withdrawals fail silently if there are insufficient funds (rather than throwing an error)
11. Client accounts are created as needed when processing transactions

## Project Structure

```
payment-system-V2/
├── Cargo.toml           # Project dependencies and configuration
├── Cargo.lock           # Locked dependencies 
├── src/
│   ├── main.rs          # Application entry point and CLI handling
│   ├── lib.rs           # Library exports and public API
│   ├── engine.rs        # Core payment processing engine
│   ├── processor.rs     # Transaction processing logic
│   ├── models.rs        # Data models for transactions and accounts
│   └── error.rs         # Custom error types
├── transactions.csv     # Sample transaction data
├── generate_csv.py      # Helper script to generate test data
└── logs/                # Directory for log files
```