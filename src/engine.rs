use crate::error::PaymentEngineError;
use crate::models::{Account, AccountStore, Transaction, TransactionStore, TransactionType};
use anyhow::Result;
use tracing::{debug, info, warn, error};

/// The payment engine that processes transactions
pub struct PaymentEngine {
    accounts: AccountStore,
    transactions: TransactionStore,
}

impl PaymentEngine {
    pub fn new() -> Self {
        Self {
            accounts: AccountStore::new(),
            transactions: TransactionStore::new(),
        }
    }

    /// Process a batch of transactions
    pub async fn process_transaction_batch(&mut self, transactions: &mut Vec<Transaction>) -> Result<()> {
        debug!("Processing batch of {} transactions", transactions.len());
        
        // Process each transaction in the batch
        let mut tx_ids = Vec::with_capacity(transactions.len());
        for transaction in transactions.drain(..) {
            tx_ids.push(transaction.tx);
            if let Err(e) = self.process_transaction(transaction).await {
                // Log the error but continue processing other transactions
                error!("Error processing transaction: {}", e);
            }
        }
        
        Ok(())
    }

    /// Process a single transaction
    pub async fn process_transaction(&mut self, transaction: Transaction) -> Result<()> {
        debug!(
            "Processing transaction: type={:?}, client={}, tx={}, amount={:?}",
            transaction.transaction_type, transaction.client, transaction.tx, transaction.amount
        );

        // Client accounts are locked and can't process further transactions
        let account = self.accounts.get_or_create_account(transaction.client);
        if account.locked && transaction.transaction_type != TransactionType::Dispute {
            warn!("Account {} is locked, ignoring transaction", transaction.client);
            return Ok(());
        }

        match transaction.transaction_type {
            TransactionType::Deposit => self.handle_deposit(transaction).await?,
            TransactionType::Withdrawal => self.handle_withdrawal(transaction).await?,
            TransactionType::Dispute => self.handle_dispute(transaction).await?,
            TransactionType::Resolve => self.handle_resolve(transaction).await?,
            TransactionType::Chargeback => self.handle_chargeback(transaction).await?,
        }

        Ok(())
    }

    /// Handle a deposit transaction
    async fn handle_deposit(&mut self, tx: Transaction) -> Result<()> {
        let amount = tx.amount.ok_or_else(|| {
            PaymentEngineError::MissingAmount(tx.tx)
        })?;

        let account = self.accounts.get_or_create_account(tx.client);
        account.deposit(amount);

        // Store transaction for potential future disputes
        self.transactions.add_transaction(tx);

        Ok(())
    }

    /// Handle a withdrawal transaction
    async fn handle_withdrawal(&mut self, tx: Transaction) -> Result<()> {
        let amount = tx.amount.ok_or_else(|| {
            PaymentEngineError::MissingAmount(tx.tx)
        })?;

        let account = self.accounts.get_or_create_account(tx.client);
        
        if !account.has_sufficient_funds(amount) {
            warn!("Insufficient funds for withdrawal: client={}, tx={}, amount={}", tx.client, tx.tx, amount);
            return Ok(());
        }

        account.withdraw(amount);
        
        // Store transaction for potential future disputes
        self.transactions.add_transaction(tx);

        Ok(())
    }

    /// Handle a dispute transaction
    async fn handle_dispute(&mut self, tx: Transaction) -> Result<()> {
        // Get the original transaction
        let orig_tx = match self.transactions.get_transaction(tx.tx) {
            Some(t) => t,
            None => {
                warn!("Transaction not found for dispute: tx={}", tx.tx);
                return Ok(());
            }
        };

        // Ensure the client matches
        if orig_tx.client != tx.client {
            warn!(
                "Client mismatch for dispute: original={}, dispute={}",
                orig_tx.client, tx.client
            );
            return Ok(());
        }

        // Ensure it's a transaction that can be disputed (deposit)
        if orig_tx.transaction_type != TransactionType::Deposit {
            warn!(
                "Cannot dispute non-deposit transaction: tx={}, type={:?}",
                tx.tx, orig_tx.transaction_type
            );
            return Ok(());
        }

        // Ensure it's not already disputed
        if self.transactions.is_disputed(tx.tx) {
            warn!("Transaction already disputed: tx={}", tx.tx);
            return Ok(());
        }

        // Get the amount from the original transaction
        let amount = orig_tx.amount.ok_or_else(|| {
            PaymentEngineError::MissingAmount(tx.tx)
        })?;

        // Mark the transaction as disputed
        self.transactions.set_disputed(tx.tx, true);

        // Hold the funds
        let account = self.accounts.get_or_create_account(tx.client);
        if !account.hold(amount) {
            warn!(
                "Failed to hold funds for dispute: client={}, tx={}, amount={}",
                tx.client, tx.tx, amount
            );
            // Reset dispute status since we couldn't hold the funds
            self.transactions.set_disputed(tx.tx, false);
        }

        Ok(())
    }

    /// Handle a resolve transaction
    async fn handle_resolve(&mut self, tx: Transaction) -> Result<()> {
        // Get the original transaction
        let orig_tx = match self.transactions.get_transaction(tx.tx) {
            Some(t) => t,
            None => {
                warn!("Transaction not found for resolve: tx={}", tx.tx);
                return Ok(());
            }
        };

        // Ensure the client matches
        if orig_tx.client != tx.client {
            warn!(
                "Client mismatch for resolve: original={}, resolve={}",
                orig_tx.client, tx.client
            );
            return Ok(());
        }

        // Ensure the transaction is disputed
        if !self.transactions.is_disputed(tx.tx) {
            warn!("Transaction not under dispute for resolve: tx={}", tx.tx);
            return Ok(());
        }

        // Get the amount from the original transaction
        let amount = orig_tx.amount.ok_or_else(|| {
            PaymentEngineError::MissingAmount(tx.tx)
        })?;

        // Mark the transaction as no longer disputed
        self.transactions.set_disputed(tx.tx, false);

        // Release the funds
        let account = self.accounts.get_or_create_account(tx.client);
        if !account.release(amount) {
            warn!(
                "Failed to release funds for resolve: client={}, tx={}, amount={}",
                tx.client, tx.tx, amount
            );
            // Restore dispute status since we couldn't release the funds
            self.transactions.set_disputed(tx.tx, true);
        }

        Ok(())
    }

    /// Handle a chargeback transaction
    async fn handle_chargeback(&mut self, tx: Transaction) -> Result<()> {
        // Get the original transaction
        let orig_tx = match self.transactions.get_transaction(tx.tx) {
            Some(t) => t,
            None => {
                warn!("Transaction not found for chargeback: tx={}", tx.tx);
                return Ok(());
            }
        };

        // Ensure the client matches
        if orig_tx.client != tx.client {
            warn!(
                "Client mismatch for chargeback: original={}, chargeback={}",
                orig_tx.client, tx.client
            );
            return Ok(());
        }

        // Ensure the transaction is disputed
        if !self.transactions.is_disputed(tx.tx) {
            warn!("Transaction not under dispute for chargeback: tx={}", tx.tx);
            return Ok(());
        }

        // Get the amount from the original transaction
        let amount = orig_tx.amount.ok_or_else(|| {
            PaymentEngineError::MissingAmount(tx.tx)
        })?;

        // Mark the transaction as no longer disputed
        self.transactions.set_disputed(tx.tx, false);

        // Process the chargeback
        let account = self.accounts.get_or_create_account(tx.client);
        if !account.chargeback(amount) {
            warn!(
                "Failed to process chargeback: client={}, tx={}, amount={}",
                tx.client, tx.tx, amount
            );
            // Restore dispute status since we couldn't process the chargeback
            self.transactions.set_disputed(tx.tx, true);
        } else {
            info!("Account {} locked due to chargeback", tx.client);
        }

        Ok(())
    }

    /// Get all client accounts
    pub fn get_accounts(&self) -> Vec<Account> {
        self.accounts.get_all_accounts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    
    // Helper function to create a deposit transaction
    fn create_deposit(client: u16, tx: u32, amount: rust_decimal::Decimal) -> Transaction {
        Transaction {
            transaction_type: TransactionType::Deposit,
            client,
            tx,
            amount: Some(amount),
        }
    }
    
    // Helper function to create a withdrawal transaction
    fn create_withdrawal(client: u16, tx: u32, amount: rust_decimal::Decimal) -> Transaction {
        Transaction {
            transaction_type: TransactionType::Withdrawal,
            client,
            tx,
            amount: Some(amount),
        }
    }
    
    // Helper function to create a dispute transaction
    fn create_dispute(client: u16, tx: u32) -> Transaction {
        Transaction {
            transaction_type: TransactionType::Dispute,
            client,
            tx,
            amount: None,
        }
    }
    
    // Helper function to create a resolve transaction
    fn create_resolve(client: u16, tx: u32) -> Transaction {
        Transaction {
            transaction_type: TransactionType::Resolve,
            client,
            tx,
            amount: None,
        }
    }
    
    // Helper function to create a chargeback transaction
    fn create_chargeback(client: u16, tx: u32) -> Transaction {
        Transaction {
            transaction_type: TransactionType::Chargeback,
            client,
            tx,
            amount: None,
        }
    }
    
    #[tokio::test]
    async fn test_deposit() {
        let mut engine = PaymentEngine::new();
        
        let tx = create_deposit(1, 1, dec!(100));
        engine.process_transaction(tx).await.unwrap();
        
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].client, 1);
        assert_eq!(accounts[0].available, dec!(100));
        assert_eq!(accounts[0].total, dec!(100));
    }
    
    #[tokio::test]
    async fn test_withdrawal() {
        let mut engine = PaymentEngine::new();
        
        // Deposit first
        let deposit_tx = create_deposit(1, 1, dec!(100));
        engine.process_transaction(deposit_tx).await.unwrap();
        
        // Then withdraw
        let withdraw_tx = create_withdrawal(1, 2, dec!(30));
        engine.process_transaction(withdraw_tx).await.unwrap();
        
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].available, dec!(70));
        assert_eq!(accounts[0].total, dec!(70));
    }
    
    #[tokio::test]
    async fn test_insufficient_funds_withdrawal() {
        let mut engine = PaymentEngine::new();
        
        // Deposit first
        let deposit_tx = create_deposit(1, 1, dec!(50));
        engine.process_transaction(deposit_tx).await.unwrap();
        
        // Try to withdraw more than available
        let withdraw_tx = create_withdrawal(1, 2, dec!(75));
        engine.process_transaction(withdraw_tx).await.unwrap();
        
        // Balance should remain unchanged
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].available, dec!(50));
        assert_eq!(accounts[0].total, dec!(50));
    }
    
    #[tokio::test]
    async fn test_dispute() {
        let mut engine = PaymentEngine::new();
        
        // Deposit
        let deposit_tx = create_deposit(1, 1, dec!(100));
        engine.process_transaction(deposit_tx).await.unwrap();
        
        // Dispute the deposit
        let dispute_tx = create_dispute(1, 1);
        engine.process_transaction(dispute_tx).await.unwrap();
        
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].available, dec!(0));
        assert_eq!(accounts[0].held, dec!(100));
        assert_eq!(accounts[0].total, dec!(100));
    }
    
    #[tokio::test]
    async fn test_resolve() {
        let mut engine = PaymentEngine::new();
        
        // Deposit
        let deposit_tx = create_deposit(1, 1, dec!(100));
        engine.process_transaction(deposit_tx).await.unwrap();
        
        // Dispute
        let dispute_tx = create_dispute(1, 1);
        engine.process_transaction(dispute_tx).await.unwrap();
        
        // Resolve
        let resolve_tx = create_resolve(1, 1);
        engine.process_transaction(resolve_tx).await.unwrap();
        
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].available, dec!(100));
        assert_eq!(accounts[0].held, dec!(0));
        assert_eq!(accounts[0].total, dec!(100));
    }
    
    #[tokio::test]
    async fn test_chargeback() {
        let mut engine = PaymentEngine::new();
        
        // Deposit
        let deposit_tx = create_deposit(1, 1, dec!(100));
        engine.process_transaction(deposit_tx).await.unwrap();
        
        // Dispute
        let dispute_tx = create_dispute(1, 1);
        engine.process_transaction(dispute_tx).await.unwrap();
        
        // Chargeback
        let chargeback_tx = create_chargeback(1, 1);
        engine.process_transaction(chargeback_tx).await.unwrap();
        
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].available, dec!(0));
        assert_eq!(accounts[0].held, dec!(0));
        assert_eq!(accounts[0].total, dec!(0));
        assert!(accounts[0].locked);
    }
    
    #[tokio::test]
    async fn test_locked_account() {
        let mut engine = PaymentEngine::new();
        
        // Deposit
        let deposit_tx = create_deposit(1, 1, dec!(100));
        engine.process_transaction(deposit_tx).await.unwrap();
        
        // Dispute and chargeback to lock the account
        engine.process_transaction(create_dispute(1, 1)).await.unwrap();
        engine.process_transaction(create_chargeback(1, 1)).await.unwrap();
        
        // Try another deposit after account is locked
        let new_deposit_tx = create_deposit(1, 2, dec!(50));
        engine.process_transaction(new_deposit_tx).await.unwrap();
        
        // Balance should remain unchanged since account is locked
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].available, dec!(0));
        assert_eq!(accounts[0].total, dec!(0));
        assert!(accounts[0].locked);
    }

    #[tokio::test]
    async fn test_multiple_clients() {
        let mut engine = PaymentEngine::new();
        
        // Client 1 transactions
        engine.process_transaction(create_deposit(1, 1, dec!(100))).await.unwrap();
        engine.process_transaction(create_withdrawal(1, 2, dec!(20))).await.unwrap();
        
        // Client 2 transactions
        engine.process_transaction(create_deposit(2, 3, dec!(200))).await.unwrap();
        engine.process_transaction(create_withdrawal(2, 4, dec!(50))).await.unwrap();
        
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 2);
        
        // Find client accounts (they might be in any order)
        let mut client_balances = HashMap::new();
        for account in accounts {
            client_balances.insert(account.client, (account.available, account.total));
        }
        
        assert_eq!(client_balances.get(&1), Some(&(dec!(80), dec!(80))));
        assert_eq!(client_balances.get(&2), Some(&(dec!(150), dec!(150))));
    }
    
    #[tokio::test]
    async fn test_dispute_non_existent_tx() {
        let mut engine = PaymentEngine::new();
        
        // Deposit
        engine.process_transaction(create_deposit(1, 1, dec!(100))).await.unwrap();
        
        // Dispute a non-existent transaction
        engine.process_transaction(create_dispute(1, 999)).await.unwrap();
        
        // Balance should remain unchanged
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].available, dec!(100));
        assert_eq!(accounts[0].held, dec!(0));
        assert_eq!(accounts[0].total, dec!(100));
    }
    
    #[tokio::test]
    async fn test_resolve_without_dispute() {
        let mut engine = PaymentEngine::new();
        
        // Deposit
        engine.process_transaction(create_deposit(1, 1, dec!(100))).await.unwrap();
        
        // Resolve without dispute
        engine.process_transaction(create_resolve(1, 1)).await.unwrap();
        
        // Balance should remain unchanged
        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].available, dec!(100));
        assert_eq!(accounts[0].held, dec!(0));
        assert_eq!(accounts[0].total, dec!(100));
    }
    
    #[tokio::test]
    async fn test_client_mismatch() {
        let mut engine = PaymentEngine::new();
        
        // Client 1 deposit
        engine.process_transaction(create_deposit(1, 1, dec!(100))).await.unwrap();
        
        // Client 2 tries to dispute client 1's transaction
        engine.process_transaction(create_dispute(2, 1)).await.unwrap();
        
        // Balance should remain unchanged
        let accounts = engine.get_accounts();
        let client1_account = accounts.iter().find(|a| a.client == 1).unwrap();
        assert_eq!(client1_account.available, dec!(100));
        assert_eq!(client1_account.held, dec!(0));
        assert_eq!(client1_account.total, dec!(100));
    }
}