use crate::ContractError::Unauthorized;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, OwnerResponse, QueryMsg, WithdrawableResponse};
use crate::state::{State, BALANCES, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:terra-deposit-withdraw";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: info.sender.clone(),
        cap: Uint128::zero(),
        withdrawable: Uint128::zero(),
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
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => deposit(deps, env, info),
        ExecuteMsg::Withdraw { amount } => withdraw(deps, env, info, amount),
    }
}

pub fn deposit(deps: DepsMut, _env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    if info.funds.len() == 1 {
        let denom: String = info.funds[0].denom.clone();
        let mut share: Uint128 = info.funds[0].amount;
        if denom == "uusd" && share > Uint128::zero() {
            let mut state = STATE.load(deps.storage)?;
            if state.withdrawable != Uint128::zero() {
                share = share.checked_mul(state.withdrawable)?.checked_div(state.cap)?;
            }
            state.cap = state.cap.checked_add(info.funds[0].amount)?;
            state.withdrawable = state.withdrawable.checked_add(share)?;
            STATE.save(deps.storage, &state);
            BALANCES.update(
                deps.storage,
                deps.api
                    .addr_canonicalize(&info.sender.to_string())?
                    .as_slice(),
                |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_add(share)?) },
            )?;
            Ok(Response::new().add_attributes(vec![
                attr("action", "deposit"),
                attr("from", info.sender),
                attr("amount", info.funds[0].amount),
                attr("share", share),
            ]))
        } else {
            Err(Unauthorized {})
        }
    } else {
        Err(Unauthorized {})
    }
}

pub fn withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    share: Uint128,
) -> Result<Response, ContractError> {
    let mut amount= share;
    if amount > Uint128::zero() {
        let mut state = STATE.load(deps.storage)?;
        if state.cap == 0 {
            Err(Unauthorized {})
        }
        amount = amount.checked_mul(state.cap)?.checked_div(state.withdrawable)?;
        BALANCES.update(
            deps.storage,
            deps.api
                .addr_canonicalize(&info.sender.to_string())?
                .as_slice(),
            |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_sub(share)?) },
        )?;
        state.cap = state.cap.checked_sub(amount)?;
        state.withdrawable = state.withdrawable.checked_sub(share)?;
        STATE.save(deps.storage, &state);
    } else {
        Err(Unauthorized {})
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
            attr("share", share),
            attr("amount", amount),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetOwner {} => to_binary(&query_owner(deps)?),
        QueryMsg::GetWithdrawable {} => to_binary(&query_withdrawable(deps)?),
    }
}

fn query_owner(deps: Deps) -> StdResult<OwnerResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(OwnerResponse { owner: state.owner })
}

fn query_withdrawable(deps: Deps) -> StdResult<WithdrawableResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(WithdrawableResponse { withdrawable: state.withdrawable })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);
        let msg = InstantiateMsg {};
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
        let msg = InstantiateMsg {};
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
