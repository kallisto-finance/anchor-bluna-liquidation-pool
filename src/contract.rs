use crate::ContractError::Unauthorized;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, ExternalExecuteMsg, InstantiateMsg, OwnerResponse, QueryMsg};
use crate::state::{State, ANCHOR_LIQUIDATION_QUEUE_ADDR, BALANCES, B_LUNA_ADDR, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:terra-deposit-withdraw";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: info.sender.clone(),
        premium_slot: msg.premium_slot,
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => deposit(deps, info),
        ExecuteMsg::Withdraw { amount } => withdraw(deps, info, amount),
        ExecuteMsg::Activate {} => activate(info),
    }
}

fn deposit(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    if info.funds.len() == 1 {
        let denom: String = info.funds[0].denom.clone();
        let amount: Uint128 = info.funds[0].amount;
        if denom == "uusd" && amount > Uint128::zero() {
            BALANCES.update(
                deps.storage,
                deps.api
                    .addr_canonicalize(&info.sender.to_string())?
                    .as_slice(),
                |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_add(amount)?) },
            )?;
            let state: State = STATE.load(deps.storage)?;
            let message: CosmosMsg = submit_bid(amount, state.premium_slot)?;
            Ok(Response::new()
                .add_attributes(vec![
                    attr("action", "deposit"),
                    attr("from", info.sender),
                    attr("amount", amount),
                ])
                .add_message(message))
        } else {
            Err(Unauthorized {})
        }
    } else {
        Err(Unauthorized {})
    }
}

fn submit_bid(amount: Uint128, premium_slot: u8) -> Result<CosmosMsg, ContractError> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
        funds: vec![Coin::new(amount.u128(), "uusd")],
        msg: to_binary(&ExternalExecuteMsg::SubmitBid {
            collateral_token: B_LUNA_ADDR.to_string(),
            premium_slot,
        })?,
    }))
}

pub fn activate(info: MessageInfo) -> Result<Response, ContractError> {
    Ok(Response::new()
        .add_attributes(vec![attr("action", "activate"), attr("from", info.sender)])
        .add_message(activate_all_bids()?))
}

fn activate_all_bids() -> Result<CosmosMsg, ContractError> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: ANCHOR_LIQUIDATION_QUEUE_ADDR.to_string(),
        funds: vec![],
        msg: to_binary(&ExternalExecuteMsg::ActivateBids {
            collateral_token: B_LUNA_ADDR.to_string(),
            bids_idx: None,
        })?,
    }))
}

fn withdraw(deps: DepsMut, info: MessageInfo, amount: Uint128) -> Result<Response, ContractError> {
    if amount > Uint128::zero() {
        BALANCES.update(
            deps.storage,
            deps.api
                .addr_canonicalize(&info.sender.to_string())?
                .as_slice(),
            |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_sub(amount)?) },
        )?;
    } else {
        return Err(Unauthorized {});
    }
    Ok(Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount,
            }],
        }))
        .add_attributes(vec![
            attr("action", "withdraw"),
            attr("to", info.sender),
            attr("amount", amount),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetOwner {} => to_binary(&query_owner(deps)?),
    }
}

fn query_owner(deps: Deps) -> StdResult<OwnerResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(OwnerResponse {
        owner: state.owner.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);
        let msg = InstantiateMsg { premium_slot: 1 };
        let info = mock_info("creator", &coins(1000, "uusd"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetOwner {}).unwrap();
        let value: OwnerResponse = from_binary(&res).unwrap();
        assert_eq!("creator", value.owner);
    }

    #[test]
    fn deposit_withdraw() {
        let mut deps = mock_dependencies(&[]);
        let msg = InstantiateMsg { premium_slot: 1 };
        let info = mock_info("creator", &coins(1000, "uusd"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(100, "uusd"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::Withdraw {
            amount: Uint128::new(10),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetOwner {}).unwrap();
        let value: OwnerResponse = from_binary(&res).unwrap();
        assert_eq!("creator", value.owner);
    }
}
