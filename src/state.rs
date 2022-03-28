use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw_storage_plus::{Item, Map, U32Key};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,
    pub total_supply: Uint128,
    pub locked_b_luna: Uint128,
    pub paused: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenRecord {
    pub amount: Uint128,
    pub timestamp: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Permission {
    pub submit_bid: bool,
}

pub const BALANCES: Map<&[u8], Uint128> = Map::new("balance");

pub const PERMISSIONS: Map<&[u8], Permission> = Map::new("permission");

pub const STATE: Item<State> = Item::new("state");

pub const CLAIM_LIST: Map<U32Key, TokenRecord> = Map::new("claim_list");

pub const ANCHOR_LIQUIDATION_QUEUE_ADDR: &str = "terra1e25zllgag7j9xsun3me4stnye2pcg66234je3u";

pub const B_LUNA_ADDR: &str = "terra1kc87mu460fwkqte29rquh4hc20m54fxwtsx7gp";

pub const PRICE_ORACLE_ADDR: &str = "terra1cgg6yef7qcdm070qftghfulaxmllgmvk77nc7t";

pub const ASTROPORT_ROUTER: &str = "terra16t7dpwwgx9n3lq6l6te3753lsjqwhxwpday9zx";

pub const LOCK_PERIOD: u64 = 1 * 1 * 10 * 60;
