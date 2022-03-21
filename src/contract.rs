use crate::ContractError::{DivideByZeroError, Insufficient, Invalidate, Unauthorized};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128, Uint256, WasmMsg,
};
use cw2::set_contract_version;
use std::convert::TryFrom;
use std::ops::Mul;

use crate::error::ContractError;
use crate::msg::AssetInfo::{NativeToken, Token};
use crate::msg::SwapOperation::{AstroSwap, NativeSwap};
use crate::msg::{
    ActivatableResponse, BalanceResponse, BidsResponse, ClaimableResponse, Cw20BalanceResponse,
    ExecuteMsg, ExternalExecuteMsg, ExternalQueryMsg, InfoResponse, InstantiateMsg,
    PermissionResponse, PriceResponse, QueryMsg, TotalCapResponse, WithdrawableLimitResponse,
};
use crate::state::{
    Permission, State, TokenRecord, ANCHOR_LIQUIDATION_QUEUE_ADDR, ASTROPORT_ROUTER, BALANCES,
    B_LUNA_ADDR, CLAIM_LIST, LOCK_PERIOD, PERMISSIONS, PRICE_ORACLE_ADDR, STATE,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:terra-deposit-withdraw";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: msg.owner.clone(),
        total_supply: Uint128::zero(),
        locked_b_luna: Uint128::zero(),
        paused: false,
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;
    PERMISSIONS.save(
        deps.storage,
        deps.api
            .addr_canonicalize(&msg.owner.to_string())?
            .as_slice(),
        &Permission { submit_bid: true },
    )?;
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", state.owner))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Deposit UST to vault
        ExecuteMsg::Deposit {} => deposit(deps, env, info),
        // Withdraw UST from vault
        ExecuteMsg::WithdrawUst { share } => withdraw_ust(deps, env, info, share),
        // Activate all bids
        ExecuteMsg::ActivateBid {} => activate_bid(info),
        // Submit bid with amount and premium slot from service
        // Only owner can execute
        ExecuteMsg::SubmitBid {
            amount,
            premium_slot,
        } => submit_bid(deps, env, info, amount, premium_slot),
        // Withdraw all liquidated bLuna from Anchor
        ExecuteMsg::ClaimLiquidation {} => claim_liquidation(deps, env, info),
        // Transfer ownership to the other address
        // Owner will be service account address
        // Only owner can execute
        ExecuteMsg::TransferOwnership { new_owner } => transfer_ownership(deps, info, new_owner),
        ExecuteMsg::Unlock {} => unlock(deps, env, info),
        ExecuteMsg::Swap {} => swap(deps, env, info),
        ExecuteMsg::Pause { pause } => pause_resume(deps, info, pause),
        ExecuteMsg::SetPermission {
            address,
            new_permission,
        } => set_permission(deps, info, address, new_permission),
    }
}

fn deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Only one coin
    if info.funds.len() != 1 {
        return Err(Invalidate {});
    }
    let mut share: Uint128 = info.funds[0].amount;
    // Only UST and non-zero amount
    if info.funds[0].denom != "uusd" || share.is_zero() {
        return Err(Invalidate {});
    }
    let mut state = STATE.load(deps.storage)?;
    // UST in vault
    let mut usd_balance = deps
        .querier
        .query_balance(&env.contract.address, "uusd")?
        .amount
        - share;
    // bLuna in vault
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        B_LUNA_ADDR,
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    let mut b_luna_balance = b_luna_balance_response.balance;
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    // Iterate all valid bids
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: B_LUNA_ADDR.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            // Waiting UST for liquidation
            usd_balance += Uint128::try_from(item.amount)?;
            // Pending bLuna in Anchor
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    // Fetch bLuna price from oracle
    let price_response: PriceResponse = deps.querier.query_wasm_smart(
        PRICE_ORACLE_ADDR.to_string(),
        &ExternalQueryMsg::Price {
            base: B_LUNA_ADDR.to_string(),
            quote: "uusd".to_string(),
        },
    )?;
    let price = price_response.rate;
    let total_cap = Uint128::try_from(Uint256::from(b_luna_balance).mul(price))? + usd_balance;
    if !state.total_supply.is_zero() {
        if total_cap.is_zero() {
            return Err(DivideByZeroError {});
        }
        share = share.checked_mul(state.total_supply)? / total_cap;
    }
    state.total_supply += share;
    STATE.save(deps.storage, &state)?;
    BALANCES.update(
        deps.storage,
        deps.api
            .addr_canonicalize(&info.sender.to_string())?
            .as_slice(),
        |balance| -> StdResult<_> { Ok(balance.unwrap_or_default() + share) },
    )?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "deposit"),
        attr("from", info.sender),
        attr("amount", info.funds[0].amount),
        attr("share", share),
    ]))
}

fn submit_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    premium_slot: u8,
) -> Result<Response, ContractError> {
    let permission = PERMISSIONS
        .may_load(
            deps.storage,
            deps.api
                .addr_canonicalize(&info.sender.to_string())?
                .as_slice(),
        )?
        .unwrap_or(Permission { submit_bid: false });
    if !permission.submit_bid {
        return Err(Unauthorized {});
    }
    let usd_balance = deps
        .querier
        .query_balance(env.contract.address, "uusd")?
        .amount;
    if !amount.is_zero() && usd_balance >= amount {
        Ok(Response::new()
            .add_attributes(vec![
                attr("action", "submit_bid"),
                attr("from", info.sender),
                attr("amount", amount),
                attr("premium_slot", premium_slot.to_string()),
            ])
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
                funds: vec![Coin::new(amount.u128(), "uusd")],
                msg: to_binary(&ExternalExecuteMsg::SubmitBid {
                    collateral_token: B_LUNA_ADDR.to_string(),
                    premium_slot,
                })?,
            })))
    } else {
        Err(Insufficient {})
    }
}

pub fn activate_bid(info: MessageInfo) -> Result<Response, ContractError> {
    Ok(Response::new()
        .add_attributes(vec![attr("action", "activate"), attr("from", info.sender)])
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
            funds: vec![],
            msg: to_binary(&ExternalExecuteMsg::ActivateBids {
                collateral_token: B_LUNA_ADDR.to_string(),
                bids_idx: None,
            })?,
        })))
}

fn withdraw_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    share: Uint128,
) -> Result<Response, ContractError> {
    if share.is_zero() {
        return Err(Invalidate {});
    }
    BALANCES.update(
        deps.storage,
        deps.api
            .addr_canonicalize(&info.sender.to_string())?
            .as_slice(),
        |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_sub(share)?) },
    )?;
    let uusd_balance = deps
        .querier
        .query_balance(&env.contract.address, "uusd")?
        .amount;
    let mut usd_balance = uusd_balance;
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        B_LUNA_ADDR,
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    let mut b_luna_balance = b_luna_balance_response.balance;
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: B_LUNA_ADDR.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            usd_balance += Uint128::try_from(item.amount)?;
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    let price_response: PriceResponse = deps.querier.query_wasm_smart(
        PRICE_ORACLE_ADDR.to_string(),
        &ExternalQueryMsg::Price {
            base: B_LUNA_ADDR.to_string(),
            quote: "uusd".to_string(),
        },
    )?;
    let price = price_response.rate;
    // Calculate total cap
    let total_cap = Uint128::try_from(Uint256::from(b_luna_balance).mul(price))? + usd_balance;
    let mut state = STATE.load(deps.storage)?;
    // Calculate exact amount from share and total cap
    let withdraw_cap = total_cap * share / state.total_supply;
    state.total_supply -= share;
    STATE.save(deps.storage, &state)?;
    // Withdraw if UST in vault is enough
    if uusd_balance >= withdraw_cap {
        Ok(Response::new()
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: withdraw_cap,
                }],
            }))
            .add_attributes(vec![
                attr("action", "withdraw"),
                attr("to", info.sender),
                attr("share", share),
                attr("amount", withdraw_cap),
            ]))
    } else {
        // Retract bids for insufficient UST in vault
        let mut messages = vec![];
        usd_balance = withdraw_cap - uusd_balance;
        start_after = Some(Uint128::zero());
        loop {
            let res: BidsResponse = deps.querier.query_wasm_smart(
                ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
                &ExternalQueryMsg::BidsByUser {
                    collateral_token: B_LUNA_ADDR.to_string(),
                    bidder: env.contract.address.to_string(),
                    start_after,
                    limit: Some(31),
                },
            )?;
            for item in &res.bids {
                if item.amount < usd_balance.into() {
                    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
                        msg: to_binary(&ExternalExecuteMsg::RetractBid {
                            bid_idx: item.idx,
                            amount: None,
                        })?,
                        funds: vec![],
                    }));
                    usd_balance -= Uint128::try_from(item.amount)?;
                } else {
                    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
                        msg: to_binary(&ExternalExecuteMsg::RetractBid {
                            bid_idx: item.idx,
                            amount: Some(usd_balance.into()),
                        })?,
                        funds: vec![],
                    }));
                    usd_balance = Uint128::zero();
                    break;
                }
            }
            if usd_balance.is_zero() || res.bids.len() < 31 {
                break;
            }
            start_after = Some(res.bids.last().unwrap().idx);
        }
        if usd_balance.is_zero() {
            return Err(Insufficient {});
        }
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: withdraw_cap,
            }],
        }));
        Ok(Response::new().add_messages(messages).add_attributes(vec![
            attr("action", "withdraw"),
            attr("to", info.sender),
            attr("share", share),
            attr("amount", withdraw_cap),
        ]))
    }
}

fn claim_liquidation(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut b_luna_balance = Uint128::zero();
    let mut start_after = Some(Uint128::zero());
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: B_LUNA_ADDR.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    if b_luna_balance.is_zero() {
        return Err(Insufficient {});
    }
    let mut claim_list = CLAIM_LIST.load(deps.storage)?;
    claim_list.push(TokenRecord {
        amount: b_luna_balance,
        timestamp: env.block.time,
    });
    let mut state = STATE.load(deps.storage)?;
    state.locked_b_luna += b_luna_balance;
    let mut unlocked_b_luna = Uint128::zero();
    claim_list.retain(|claim| {
        if claim.timestamp.plus_seconds(LOCK_PERIOD) <= env.block.time {
            unlocked_b_luna += claim.amount;
            false
        } else {
            true
        }
    });
    state.locked_b_luna -= unlocked_b_luna;
    STATE.save(deps.storage, &state)?;
    CLAIM_LIST.save(deps.storage, &claim_list)?;
    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
            msg: to_binary(&ExternalExecuteMsg::ClaimLiquidations {
                collateral_token: B_LUNA_ADDR.to_string(),
                bids_idx: None,
            })?,
            funds: vec![],
        }))
        .add_attributes(vec![
            attr("action", "liquidate"),
            attr("from", &info.sender),
            attr("amount", b_luna_balance.to_string()),
        ])
        .add_attributes(vec![
            attr("action", "unlock"),
            attr("from", info.sender),
            attr("amount", unlocked_b_luna.to_string()),
        ]))
}

pub fn transfer_ownership(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: Addr,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    if state.owner != info.sender {
        return Err(Unauthorized {});
    }
    state.owner = new_owner.clone();
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "transfer_ownership"),
        attr("from", info.sender),
        attr("to", new_owner.to_string()),
    ]))
}

fn unlock(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let mut claim_list = CLAIM_LIST.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut unlocked_b_luna = Uint128::zero();
    claim_list.retain(|claim| {
        if claim.timestamp.plus_seconds(LOCK_PERIOD) <= env.block.time {
            unlocked_b_luna += claim.amount;
            false
        } else {
            true
        }
    });
    if unlocked_b_luna.is_zero() {
        return Err(Insufficient {});
    }
    CLAIM_LIST.save(deps.storage, &claim_list)?;
    state.locked_b_luna -= unlocked_b_luna;
    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "unlock"),
        attr("from", info.sender),
        attr("amount", unlocked_b_luna.to_string()),
    ]))
}
fn set_permission(
    deps: DepsMut,
    info: MessageInfo,
    address: Addr,
    new_permission: Permission,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;
    if state.owner != info.sender {
        return Err(Unauthorized {});
    }
    let permission = PERMISSIONS
        .may_load(
            deps.storage,
            deps.api.addr_canonicalize(&address.to_string())?.as_slice(),
        )?
        .unwrap_or(Permission { submit_bid: false });
    if permission == new_permission {
        return Err(Invalidate {});
    }
    if permission == (Permission { submit_bid: false }) {
        PERMISSIONS.remove(
            deps.storage,
            deps.api
                .addr_canonicalize(&info.sender.to_string())?
                .as_slice(),
        );
    } else {
        PERMISSIONS.save(
            deps.storage,
            deps.api
                .addr_canonicalize(&info.sender.to_string())?
                .as_slice(),
            &permission,
        )?;
    }
    Ok(Response::new().add_attributes(vec![
        attr("action", "set_permission"),
        attr("from", info.sender),
        attr("to", address.to_string()),
        attr("submit_bid", new_permission.submit_bid.to_string()),
    ]))
}
fn swap(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        B_LUNA_ADDR,
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    let swap_amount = b_luna_balance_response.balance - state.locked_b_luna;
    if swap_amount.is_zero() {
        return Err(Insufficient {});
    }
    let msg = ExternalExecuteMsg::Send {
        contract: ASTROPORT_ROUTER.to_string(),
        amount: swap_amount,
        msg: to_binary(&ExternalExecuteMsg::ExecuteSwapOperations {
            operations: vec![
                AstroSwap {
                    offer_asset_info: Token {
                        contract_addr: Addr::unchecked(B_LUNA_ADDR),
                    },
                    ask_asset_info: NativeToken {
                        denom: "uluna".to_string(),
                    },
                },
                NativeSwap {
                    offer_denom: "uluna".to_string(),
                    ask_denom: "uusd".to_string(),
                },
            ],
            minimum_receive: None,
            to: None,
            max_spread: None,
        })?,
    };
    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: B_LUNA_ADDR.to_string(),
            msg: to_binary(&msg)?,
            funds: vec![],
        }))
        .add_attributes(vec![
            attr("action", "swap"),
            attr("from", info.sender),
            attr("amount", swap_amount.to_string()),
        ]))
}

fn pause_resume(deps: DepsMut, info: MessageInfo, pause: bool) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    if state.owner != info.sender {
        return Err(Unauthorized {});
    }
    if state.paused == pause {
        return Err(Invalidate {});
    }
    state.paused = pause;
    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", if pause { "pause" } else { "resume" }),
        attr("from", info.sender),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        // Get owner address and total supply
        QueryMsg::GetInfo {} => to_binary(&query_info(deps)?),
        // Get share from address
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        // Get total cap in vault and anchor
        QueryMsg::TotalCap {} => to_binary(&query_total_cap(deps, env)?),
        // Return true if activate is needed
        QueryMsg::Activatable {} => to_binary(&query_activatable(deps, env)?),
        // Return true if liquidate is needed
        QueryMsg::Claimable {} => to_binary(&query_claimable(deps, env)?),
        QueryMsg::WithdrawableLimit { address } => {
            to_binary(&query_withdrawable_limit(deps, env, address)?)
        }
        QueryMsg::Permission { address } => to_binary(&query_permission(deps, address)?),
    }
}

fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(InfoResponse {
        owner: state.owner.to_string(),
        total_supply: state.total_supply,
        locked_b_luna: state.locked_b_luna,
        paused: state.paused,
    })
}

fn query_balance(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let address = deps.api.addr_canonicalize(&address)?;
    let balance = BALANCES
        .may_load(deps.storage, address.as_slice())?
        .unwrap_or_default();
    Ok(BalanceResponse { balance })
}

fn query_total_cap(deps: Deps, env: Env) -> StdResult<TotalCapResponse> {
    let mut usd_balance = deps
        .querier
        .query_balance(&env.contract.address, "uusd")?
        .amount;
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        B_LUNA_ADDR,
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    let mut b_luna_balance = b_luna_balance_response.balance;
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: B_LUNA_ADDR.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            usd_balance += Uint128::try_from(item.amount)?;
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    let price_response: PriceResponse = deps.querier.query_wasm_smart(
        PRICE_ORACLE_ADDR.to_string(),
        &ExternalQueryMsg::Price {
            base: B_LUNA_ADDR.to_string(),
            quote: "uusd".to_string(),
        },
    )?;
    let price = price_response.rate;
    let total_cap = Uint128::try_from(Uint256::from(b_luna_balance).mul(price))? + usd_balance;
    Ok(TotalCapResponse { total_cap })
}

fn query_activatable(deps: Deps, env: Env) -> StdResult<ActivatableResponse> {
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: B_LUNA_ADDR.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            if item.wait_end.is_some() && item.wait_end.unwrap() < env.block.time.seconds() {
                return Ok(ActivatableResponse { activatable: true });
            }
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    Ok(ActivatableResponse { activatable: false })
}

fn query_claimable(deps: Deps, env: Env) -> StdResult<ClaimableResponse> {
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: B_LUNA_ADDR.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            if !item.pending_liquidated_collateral.is_zero() {
                return Ok(ClaimableResponse { claimable: true });
            }
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    Ok(ClaimableResponse { claimable: false })
}

fn query_withdrawable_limit(
    deps: Deps,
    env: Env,
    address: String,
) -> StdResult<WithdrawableLimitResponse> {
    let address = deps.api.addr_canonicalize(&address)?;
    let balance = BALANCES
        .may_load(deps.storage, address.as_slice())?
        .unwrap_or_default();
    if balance.is_zero() {
        return Ok(WithdrawableLimitResponse {
            limit: Uint128::zero(),
        });
    }
    let mut usd_balance = deps
        .querier
        .query_balance(&env.contract.address, "uusd")?
        .amount;
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        B_LUNA_ADDR,
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    let mut b_luna_balance = b_luna_balance_response.balance;
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    // Iterate all valid bids
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: B_LUNA_ADDR.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            // Waiting UST for liquidation
            usd_balance += Uint128::try_from(item.amount)?;
            // Pending bLuna in Anchor
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    if usd_balance.is_zero() {
        return Ok(WithdrawableLimitResponse {
            limit: Uint128::zero(),
        });
    }
    // Fetch bLuna price from oracle
    let price_response: PriceResponse = deps.querier.query_wasm_smart(
        PRICE_ORACLE_ADDR.to_string(),
        &ExternalQueryMsg::Price {
            base: B_LUNA_ADDR.to_string(),
            quote: "uusd".to_string(),
        },
    )?;
    let price = price_response.rate;
    let total_cap = Uint128::try_from(Uint256::from(b_luna_balance).mul(price))? + usd_balance;
    let state = STATE.load(deps.storage)?;
    let withdraw_amount = balance * total_cap / state.total_supply;
    if withdraw_amount <= usd_balance {
        Ok(WithdrawableLimitResponse { limit: balance })
    } else {
        Ok(WithdrawableLimitResponse {
            limit: usd_balance * state.total_supply / total_cap,
        })
    }
}

fn query_permission(deps: Deps, address: String) -> StdResult<PermissionResponse> {
    let address = deps.api.addr_canonicalize(&address)?;
    let permission = PERMISSIONS
        .may_load(deps.storage, address.as_slice())?
        .unwrap_or(Permission { submit_bid: false });
    Ok(PermissionResponse { permission })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Addr};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);
        let msg = InstantiateMsg {
            owner: Addr::unchecked("owner"),
        };
        let info = mock_info("creator", &coins(1000, "uusd"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetInfo {}).unwrap();
        let value: InfoResponse = from_binary(&res).unwrap();
        assert_eq!("owner", value.owner);
    }
}
