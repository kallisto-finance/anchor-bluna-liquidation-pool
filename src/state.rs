use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal256, Uint128, Uint256};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,
    pub cap: Uint128,
    pub withdrawable: Uint128,
}

pub struct DepositInfo {
    pub bids_idx: Option<Uint128>,
    pub collateral_token: String,
    pub premium_slot: u8,
    pub bidder: Addr,
    pub amount: Uint256,
}

pub const BALANCES: Map<&[u8], Uint128> = Map::new("balance");

pub const STATE: Item<State> = Item::new("state");
