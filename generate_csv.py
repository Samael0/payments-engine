#!/usr/bin/env python3
import random
import decimal

# Configure file parameters
NUM_CLIENTS = 1000
NUM_TRANSACTIONS = 500000
OUTPUT_FILE = "transactions.csv"

# Initialize transaction type weights
# (deposit, withdrawal, dispute, resolve, chargeback)
# Higher weight = more frequent occurrence
TRANSACTION_WEIGHTS = [60, 30, 5, 3, 2]

# Generate random decimal amount between min and max value
def random_amount():
    rand = random.uniform(1.0, 10000.0)
    return decimal.Decimal(rand).quantize(decimal.Decimal('0.0001'))

# Track transactions for dispute reference
transactions = {}
client_transactions = {}

with open(OUTPUT_FILE, 'w') as f:
    # Write header
    f.write("type,client,tx,amount\n")
    
    tx_id = 1
    
    # Generate transactions
    for _ in range(NUM_TRANSACTIONS):
        # Select transaction type based on weights
        tx_type_idx = random.choices(range(5), weights=TRANSACTION_WEIGHTS, k=1)[0]
        tx_type = ["deposit", "withdrawal", "dispute", "resolve", "chargeback"][tx_type_idx]
        
        # Select a random client ID
        client_id = random.randint(1, NUM_CLIENTS)
        
        if client_id not in client_transactions:
            client_transactions[client_id] = {'deposits': [], 'withdrawals': [], 'disputes': []}
        
        if tx_type == "deposit":
            # Generate deposit transaction
            amount = random_amount()
            f.write(f"deposit,{client_id},{tx_id},{amount}\n")
            transactions[tx_id] = {"type": "deposit", "client": client_id, "amount": amount}
            client_transactions[client_id]['deposits'].append(tx_id)
            tx_id += 1
            
        elif tx_type == "withdrawal":
            # Generate withdrawal transaction
            if client_transactions[client_id]['deposits']:  # Only if client has some deposits
                amount = random_amount() / decimal.Decimal('2.0')  # Typically withdraw less than deposited
                f.write(f"withdrawal,{client_id},{tx_id},{amount}\n")
                transactions[tx_id] = {"type": "withdrawal", "client": client_id, "amount": amount}
                client_transactions[client_id]['withdrawals'].append(tx_id)
                tx_id += 1
            
        elif tx_type == "dispute":
            # Generate dispute transaction (can only dispute existing deposits or withdrawals)
            disputable_tx = (client_transactions[client_id]['deposits'] + 
                             client_transactions[client_id]['withdrawals'])
            
            if disputable_tx and len(client_transactions[client_id]['disputes']) < len(disputable_tx):
                # Select a transaction that hasn't been disputed yet
                available_tx = [tx for tx in disputable_tx if tx not in client_transactions[client_id]['disputes']]
                if available_tx:
                    dispute_tx_id = random.choice(available_tx)
                    f.write(f"dispute,{client_id},{dispute_tx_id},\n")
                    client_transactions[client_id]['disputes'].append(dispute_tx_id)
            
        elif tx_type == "resolve":
            # Generate resolve transaction (can only resolve disputes)
            if client_transactions[client_id]['disputes']:
                resolve_tx_id = random.choice(client_transactions[client_id]['disputes'])
                f.write(f"resolve,{client_id},{resolve_tx_id},\n")
                # Remove from disputes as it's been resolved
                client_transactions[client_id]['disputes'].remove(resolve_tx_id)
            
        elif tx_type == "chargeback":
            # Generate chargeback transaction (can only chargeback disputes)
            if client_transactions[client_id]['disputes']:
                chargeback_tx_id = random.choice(client_transactions[client_id]['disputes'])
                f.write(f"chargeback,{client_id},{chargeback_tx_id},\n")
                # Remove from disputes as it's been charged back
                client_transactions[client_id]['disputes'].remove(chargeback_tx_id)

print(f"Generated {NUM_TRANSACTIONS} transactions for {NUM_CLIENTS} clients in {OUTPUT_FILE}")