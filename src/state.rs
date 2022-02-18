use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,
}

pub const BALANCES: Map<&[u8], Uint128> = Map::new("balance");

pub const STATE: Item<State> = Item::new("state");
