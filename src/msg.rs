use cosmwasm_std::{Addr, Decimal256, Uint128, Uint256};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Deposit {},
    Withdraw { share: Uint128 },
    Claim { share: Uint128 },
    Activate {},
    Submit { premium_slot: u8 },
    Liquidate {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalExecuteMsg {
    ActivateBids {
        collateral_token: String,
        bids_idx: Option<Vec<Uint128>>,
    },
    SubmitBid {
        collateral_token: String,
        premium_slot: u8,
    },
    RetractBid {
        bid_idx: Uint128,
        amount: Option<Uint256>,
    },
    ClaimLiquidations {
        collateral_token: String,
        bids_idx: Option<Vec<Uint128>>,
    },
    Transfer {
        recipient: String,
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    GetInfo {},
    Balance { address: String },
    TotalCap {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InfoResponse {
    pub owner: String,
    pub total_supply: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BalanceResponse {
    pub balance: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TotalCapResponse {
    pub total_cap: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalQueryMsg {
    // GetCount returns the current count as a json-encoded number
    Balance {
        address: String,
    },
    BidsByUser {
        collateral_token: String,
        bidder: String,
        start_after: Option<Uint128>,
        limit: Option<u8>,
    },
    Price {
        base: String,
        quote: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Cw20BalanceResponse {
    pub balance: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BidsResponse {
    pub bids: Vec<BidResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BidResponse {
    pub idx: Uint128,
    pub collateral_token: String,
    pub premium_slot: u8,
    pub bidder: String,
    pub amount: Uint256,
    pub product_snapshot: Decimal256,
    pub sum_snapshot: Decimal256,
    pub pending_liquidated_collateral: Uint256,
    pub wait_end: Option<u64>,
    pub epoch_snapshot: Uint128,
    pub scale_snapshot: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceResponse {
    pub rate: Decimal256,
    pub last_updated_base: u64,
    pub last_updated_quote: u64,
}
