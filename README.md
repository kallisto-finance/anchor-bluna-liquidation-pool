# Terra-Deposit-Withdraw

This is a vault smart contract to help with Anchor liquidation bids.

Users can deposit with UST and get share of the vault.

And withdraw (in UST) or claim (in bLuna) as much as their share of the vault.

The owner can execute submit with premium slot and amount in the vault to Anchor liquidation queue.

And the owner can activate submitted bids and execute liquidate to withdraw pending bLuna from Anchor to the vault.

The owner can transfer ownership to another address.

Submit and transfer ownership is unique feature that only owner can execute.

## ExecuteMsg

### Deposit*

Deposit UST to vault.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### Withdraw

Withdraw UST from vault.

| Key   | Type    | Description                  |
|-------|---------|------------------------------|
| share | Uint128 | Share amount to withdraw UST |


### Claim

Withdraw bLuna from vault.

| Key   | Type    | Description                    |
|-------|---------|--------------------------------|
| share | Uint128 | Share amount to withdraw bLuna |

### Activate

Activate all bids.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### Submit**

Submit bid with amount and premium slot from service.

| Key          | Type    | Description              |
|--------------|---------|--------------------------|
| amount       | Uint128 | UST amount to submit bid |
| premium_slot | u8      | Premium Slot (%)         |

### Liquidate

Withdraw all liquidated bLuna from Anchor Liquidation Queue.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### TransferOwnership**

Transfer ownership to another address.

| Key          | Type | Description       |
|--------------|------|-------------------|
| new_owner    | Addr | New owner address |

## QueryMsg

### GetInfo

Get owner address and total supply.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### InfoResponse

| Key          | Type    | Description                      |
|--------------|---------|----------------------------------|
| owner        | String  | Owner address                    |
| total_supply | Uint128 | Total supply amount of the vault |

### Balance

Get share of vault from address.

| Key     | Type   | Description            |
|---------|--------|------------------------|
| address | String | Address to get balance |

### BalanceResponse

| Key          | Type    | Description                        |
|--------------|---------|------------------------------------|
| balance      | Uint128 | Balance amount of provided address |

### TotalCap

Get total cap in vault and anchor.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### TotalCapResponse

| Key       | Type    | Description                                                       |
|-----------|---------|-------------------------------------------------------------------|
| total_cap | Uint128 | Total cap amount in vault and pending in anchor liquidation queue |

### Activatable

Check if there are bids to activate.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### ActivatableResponse

| Key         | Type | Description                   |
|-------------|------|-------------------------------|
| activatable | bool | True if activate is available |


### Liquidatable

Check if there are pending liquidated collateral.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### LiquidatableResponse

| Key          | Type | Description                    |
|--------------|------|--------------------------------|
| liquidatable | bool | True if liquidate is available |

*: Requires UST to be sent beforehand.

**: Only owner can execute.
