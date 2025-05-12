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