use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transaction types as defined in the specification
#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// Transaction record from the CSV input
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub client: u16,
    pub tx: u32,
    #[serde(default)]
    pub amount: Option<Decimal>,
}

/// Account state for a client
#[derive(Debug, Default, Clone, Serialize)]
pub struct Account {
    pub client: u16,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

impl Account {
    pub fn new(client_id: u16) -> Self {
        Self {
            client: client_id,
            available: dec!(0),
            held: dec!(0),
            total: dec!(0),
            locked: false,
        }
    }

    /// Check if account has sufficient funds for a withdrawal
    pub fn has_sufficient_funds(&self, amount: Decimal) -> bool {
        !self.locked && self.available >= amount
    }

    /// Deposit funds into the account
    pub fn deposit(&mut self, amount: Decimal) -> bool {
        if self.locked {
            return false;
        }
        
        self.available += amount;
        self.total += amount;
        true
    }

    /// Withdraw funds from the account
    pub fn withdraw(&mut self, amount: Decimal) -> bool {
        if !self.has_sufficient_funds(amount) {
            return false;
        }
        
        self.available -= amount;
        self.total -= amount;
        true
    }

    /// Hold funds for a dispute
    pub fn hold(&mut self, amount: Decimal) -> bool {
        if self.locked || self.available < amount {
            return false;
        }
        
        self.available -= amount;
        self.held += amount;
        true
    }

    /// Release funds from a dispute
    pub fn release(&mut self, amount: Decimal) -> bool {
        if self.locked || self.held < amount {
            return false;
        }
        
        self.held -= amount;
        self.available += amount;
        true
    }

    /// Process a chargeback
    pub fn chargeback(&mut self, amount: Decimal) -> bool {
        if self.locked || self.held < amount {
            return false;
        }
        
        self.held -= amount;
        self.total -= amount;
        self.locked = true;
        true
    }
}

/// Store for all processed transactions
#[derive(Debug, Default)]
pub struct TransactionStore {
    transactions: HashMap<u32, Transaction>,
    disputed: HashMap<u32, bool>,
}

impl TransactionStore {
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            disputed: HashMap::new(),
        }
    }

    pub fn add_transaction(&mut self, tx: Transaction) {
        self.transactions.insert(tx.tx, tx);
    }

    pub fn get_transaction(&self, tx_id: u32) -> Option<&Transaction> {
        self.transactions.get(&tx_id)
    }

    pub fn set_disputed(&mut self, tx_id: u32, status: bool) {
        self.disputed.insert(tx_id, status);
    }

    pub fn is_disputed(&self, tx_id: u32) -> bool {
        self.disputed.get(&tx_id).copied().unwrap_or(false)
    }
}

/// Store for all client accounts
#[derive(Debug, Default)]
pub struct AccountStore {
    accounts: HashMap<u16, Account>,
}

impl AccountStore {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    pub fn get_or_create_account(&mut self, client_id: u16) -> &mut Account {
        self.accounts.entry(client_id).or_insert_with(|| Account::new(client_id))
    }

    pub fn get_all_accounts(&self) -> Vec<Account> {
        self.accounts.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Tests for Account
    #[test]
    fn test_account_new() {
        let account = Account::new(123);
        assert_eq!(account.client, 123);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total, dec!(0));
        assert_eq!(account.locked, false);
    }

    #[test]
    fn test_account_deposit() {
        let mut account = Account::new(1);
        
        let result = account.deposit(dec!(100));
        assert!(result);
        assert_eq!(account.available, dec!(100));
        assert_eq!(account.total, dec!(100));
        
        // Test locked account
        account.locked = true;
        let result = account.deposit(dec!(50));
        assert!(!result);
        assert_eq!(account.available, dec!(100)); // Unchanged
    }

    #[test]
    fn test_account_withdraw() {
        let mut account = Account::new(1);
        account.deposit(dec!(100));
        
        // Successful withdrawal
        let result = account.withdraw(dec!(30));
        assert!(result);
        assert_eq!(account.available, dec!(70));
        assert_eq!(account.total, dec!(70));
        
        // Insufficient funds
        let result = account.withdraw(dec!(80));
        assert!(!result);
        assert_eq!(account.available, dec!(70)); // Unchanged
        
        // Locked account
        account.locked = true;
        let result = account.withdraw(dec!(10));
        assert!(!result);
        assert_eq!(account.available, dec!(70)); // Unchanged
    }

    #[test]
    fn test_account_hold() {
        let mut account = Account::new(1);
        account.deposit(dec!(100));
        
        // Successful hold
        let result = account.hold(dec!(30));
        assert!(result);
        assert_eq!(account.available, dec!(70));
        assert_eq!(account.held, dec!(30));
        assert_eq!(account.total, dec!(100)); // Total doesn't change
        
        // Insufficient available funds
        let result = account.hold(dec!(80));
        assert!(!result);
        assert_eq!(account.available, dec!(70)); // Unchanged
        assert_eq!(account.held, dec!(30)); // Unchanged
        
        // Locked account
        account.locked = true;
        let result = account.hold(dec!(10));
        assert!(!result);
        assert_eq!(account.available, dec!(70)); // Unchanged
        assert_eq!(account.held, dec!(30)); // Unchanged
    }

    #[test]
    fn test_account_release() {
        let mut account = Account::new(1);
        account.deposit(dec!(100));
        account.hold(dec!(30));
        
        // Successful release
        let result = account.release(dec!(20));
        assert!(result);
        assert_eq!(account.available, dec!(90));
        assert_eq!(account.held, dec!(10));
        assert_eq!(account.total, dec!(100)); // Total doesn't change
        
        // Insufficient held funds
        let result = account.release(dec!(20));
        assert!(!result);
        assert_eq!(account.available, dec!(90)); // Unchanged
        assert_eq!(account.held, dec!(10)); // Unchanged
        
        // Locked account
        account.locked = true;
        let result = account.release(dec!(5));
        assert!(!result);
        assert_eq!(account.available, dec!(90)); // Unchanged
        assert_eq!(account.held, dec!(10)); // Unchanged
    }

    #[test]
    fn test_account_chargeback() {
        let mut account = Account::new(1);
        account.deposit(dec!(100));
        account.hold(dec!(30));
        
        // Successful chargeback
        let result = account.chargeback(dec!(20));
        assert!(result);
        assert_eq!(account.available, dec!(70)); // Unchanged
        assert_eq!(account.held, dec!(10));
        assert_eq!(account.total, dec!(80)); // Reduced by chargeback amount
        assert!(account.locked); // Account is locked
        
        // Already locked, further chargebacks fail
        let result = account.chargeback(dec!(10));
        assert!(!result);
        assert_eq!(account.held, dec!(10)); // Unchanged
        assert_eq!(account.total, dec!(80)); // Unchanged
    }

    // Tests for TransactionStore
    #[test]
    fn test_transaction_store() {
        let mut store = TransactionStore::new();
        
        let tx = Transaction {
            transaction_type: TransactionType::Deposit,
            client: 1,
            tx: 123,
            amount: Some(dec!(100)),
        };
        
        // Add transaction
        store.add_transaction(tx.clone());
        
        // Get transaction
        let retrieved = store.get_transaction(123);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), &tx);
        
        // Unknown transaction
        let unknown = store.get_transaction(999);
        assert!(unknown.is_none());
        
        // Dispute status
        assert!(!store.is_disputed(123));
        
        // Set disputed
        store.set_disputed(123, true);
        assert!(store.is_disputed(123));
        
        // Clear disputed
        store.set_disputed(123, false);
        assert!(!store.is_disputed(123));
    }

    // Tests for AccountStore
    #[test]
    fn test_account_store() {
        let mut store = AccountStore::new();
        
        // Get non-existent account (should be created)
        let account = store.get_or_create_account(1);
        assert_eq!(account.client, 1);
        
        // Modify account
        account.deposit(dec!(100));
        
        // Get existing account
        let same_account = store.get_or_create_account(1);
        assert_eq!(same_account.available, dec!(100));
        
        // Get all accounts
        let accounts = store.get_all_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].client, 1);
        assert_eq!(accounts[0].available, dec!(100));
    }
}