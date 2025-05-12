use crate::error::PaymentEngineError;
use crate::models::{Account, AccountStore, Transaction, TransactionStore, TransactionType};
use anyhow::Result;
use tracing::{debug, info, warn};

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