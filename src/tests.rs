use crate::contract::{execute, instantiate, query};
use crate::msg::{Cw20BalanceResponse, ExecuteMsg, ExternalQueryMsg, InstantiateMsg, QueryMsg};
use cosmwasm_std::{coin, Addr, Coin, Empty, Uint128, BlockInfo};
use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor};
use cw_storage_plus::Map;

fn mock_app() -> App {
    AppBuilder::new().build()
}

fn main_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(execute, instantiate, query);
    Box::new(contract)
}

struct TestEnv {
    router: App,
    owner: Addr,
    main_addr: Addr,
}

mod mock {
    use crate::msg::{BidsResponse, Cw20BalanceResponse, ExternalQueryMsg, PriceResponse};
    use cosmwasm_std::{
        to_binary, Binary, Decimal256, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult,
        Uint128,
    };
    use cw_multi_test::{Contract, ContractWrapper};
    use std::str::FromStr;
    use cw_storage_plus::Map;

    pub type InstantiateMsg = ();
    pub type ExecuteMsg = ();

    pub fn b_luna_contract() -> Box<dyn Contract<Empty>> {
        pub const BALANCES: Map<&[u8], Uint128> = Map::new("balance");

        pub fn instantiate(
            _deps: DepsMut,
            _env: Env,
            _info: MessageInfo,
            _msg: InstantiateMsg,
        ) -> StdResult<Response> {
            Ok(Response::default())
        }
        pub fn execute(
            _deps: DepsMut,
            _env: Env,
            _info: MessageInfo,
            _msg: ExecuteMsg,
        ) -> StdResult<Response> {
            unimplemented!()
        }
        pub fn query(deps: Deps, _env: Env, msg: ExternalQueryMsg) -> StdResult<Binary> {
            match msg {
                ExternalQueryMsg::Balance { address } => to_binary(&q_balance(deps, address)),
                _ => unimplemented!(),
            }
        }
        fn q_balance(deps: Deps, address: String) -> Cw20BalanceResponse {
            let address = deps.api.addr_canonicalize(&address).unwrap();
            let balance = BALANCES
                .may_load(deps.storage, address.as_slice()).unwrap()
                .unwrap_or_default();
            Cw20BalanceResponse { balance }
        }
        let contract = ContractWrapper::new(execute, instantiate, query);
        Box::new(contract)
    }
    pub fn anchor_queue_contract() -> Box<dyn Contract<Empty>> {
        pub fn instantiate(
            _deps: DepsMut,
            _env: Env,
            _info: MessageInfo,
            _msg: InstantiateMsg,
        ) -> StdResult<Response> {
            Ok(Response::default())
        }
        pub fn execute(
            _deps: DepsMut,
            _env: Env,
            _info: MessageInfo,
            _msg: ExecuteMsg,
        ) -> StdResult<Response> {
            unimplemented!()
        }
        pub fn query(_deps: Deps, _env: Env, msg: ExternalQueryMsg) -> StdResult<Binary> {
            match msg {
                ExternalQueryMsg::BidsByUser { .. } => to_binary(&BidsResponse { bids: vec![] }),
                _ => unimplemented!(),
            }
        }
        let contract = ContractWrapper::new(execute, instantiate, query);
        Box::new(contract)
    }
    pub fn price_oracle_contract() -> Box<dyn Contract<Empty>> {
        pub fn instantiate(
            _deps: DepsMut,
            _env: Env,
            _info: MessageInfo,
            _msg: InstantiateMsg,
        ) -> StdResult<Response> {
            Ok(Response::default())
        }
        pub fn execute(
            _deps: DepsMut,
            _env: Env,
            _info: MessageInfo,
            _msg: ExecuteMsg,
        ) -> StdResult<Response> {
            unimplemented!()
        }
        pub fn query(_deps: Deps, _env: Env, msg: ExternalQueryMsg) -> StdResult<Binary> {
            match msg {
                ExternalQueryMsg::Price { .. } => to_binary(&PriceResponse {
                    rate: Decimal256::from_str("100").unwrap(),
                    last_updated_base: 0,
                    last_updated_quote: 0,
                }),
                _ => unimplemented!(),
            }
        }
        let contract = ContractWrapper::new(execute, instantiate, query);
        Box::new(contract)
    }
}

fn setup() -> TestEnv {
    let mut router = mock_app();

    let owner = Addr::unchecked("owner");
    let swap_wallet = Addr::unchecked("swap_wallet");

    let main_contract_id = router.store_code(main_contract());
    let b_luna_contract_id = router.store_code(mock::b_luna_contract());
    let anchor_queue_contract_id = router.store_code(mock::anchor_queue_contract());
    let price_oracle_contract_id = router.store_code(mock::price_oracle_contract());

    let b_luna_addr = router
        .instantiate_contract(b_luna_contract_id, owner.clone(), &(), &[], "b_luna", None)
        .unwrap();

    let anchor_queue_addr = router
        .instantiate_contract(
            anchor_queue_contract_id,
            owner.clone(),
            &(),
            &[],
            "anchor_queue",
            None,
        )
        .unwrap();

    let price_oracle_addr = router
        .instantiate_contract(
            price_oracle_contract_id,
            owner.clone(),
            &(),
            &[],
            "price_oracle",
            None,
        )
        .unwrap();

    let main_addr = router
        .instantiate_contract(
            main_contract_id,
            owner.clone(),
            &InstantiateMsg {
                owner: owner.clone(),
                swap_wallet,
                anchor_liquidation_queue: Some(anchor_queue_addr),
                collateral_token: Some(b_luna_addr),
                price_oracle: Some(price_oracle_addr),
                astroport_router: None,
                lock_period: None,
                withdraw_lock: None,
            },
            &[],
            "main_contract",
            None,
        )
        .unwrap();

    TestEnv {
        router,
        owner,
        main_addr,
    }
}

#[test]
fn proper_initialization() {
    setup();
}

#[test]
fn deposit_withdraw_ust_all() {
    let mut env = setup();

    env.router
        .init_bank_balance(&env.owner, vec![coin(1000000u128, "uusd")])
        .unwrap();

    // execute run()
    let res = env.router.execute_contract(
        env.owner.clone(),
        env.main_addr.clone(),
        &ExecuteMsg::Deposit {},
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );

    if res.is_err() {
        println!("{}", res.unwrap_err().to_string());
        return;
    }
    for event in res.unwrap().events {
        for attr in event.attributes {
            println!("{}: {}", attr.key, attr.value);
        }
    }
    let balance_response: Cw20BalanceResponse = env.router.wrap().query_wasm_smart(&env.main_addr, &QueryMsg::Balance { address: env.owner.to_string() }).unwrap();
    println!("{}", balance_response.balance.to_string());
    env.router.update_block(|block_info| {
        block_info.time = block_info.time.plus_seconds(3601);
    });
    let res = env.router.execute_contract(
        env.owner.clone(),
        env.main_addr.clone(),
        &ExecuteMsg::WithdrawUst { share: balance_response.balance },
        &[]
    );
    if res.is_err() {
        println!("{}", res.unwrap_err().to_string());
        return;
    }
    for event in res.unwrap().events {
        for attr in event.attributes {
            println!("{}: {}", attr.key, attr.value);
        }
    }
}

#[test]
fn deposit_withdraw_b_luna() {
    let mut env = setup();

    env.router
        .init_bank_balance(&env.owner, vec![coin(1000000u128, "uusd")])
        .unwrap();

    // execute run()
    let res = env.router.execute_contract(
        env.owner.clone(),
        env.main_addr.clone(),
        &ExecuteMsg::Deposit {},
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );

    if res.is_err() {
        println!("{}", res.unwrap_err().to_string());
        return;
    }
    for event in res.unwrap().events {
        for attr in event.attributes {
            println!("{}: {}", attr.key, attr.value);
        }
    }
    let balance_response: Cw20BalanceResponse = env.router.wrap().query_wasm_smart(&env.main_addr, &QueryMsg::Balance { address: env.owner.to_string() }).unwrap();
    println!("{}", balance_response.balance.to_string());
    env.router.update_block(|block_info| {
        block_info.time = block_info.time.plus_seconds(3601);
    });
    let res = env.router.execute_contract(
        env.owner.clone(),
        env.main_addr.clone(),
        &ExecuteMsg::WithdrawBLuna { share: balance_response.balance },
        &[]
    );
    if res.is_err() {
        println!("{}", res.unwrap_err().to_string());
        return;
    }
    for event in res.unwrap().events {
        for attr in event.attributes {
            println!("{}: {}", attr.key, attr.value);
        }
    }


    // // query round
    // let round: chainlink_terra::state::Round = env
    //     .router
    //     .wrap()
    //     .query_wasm_smart(&env.hello_world_addr, &QueryMsg::Round {})
    //     .unwrap();
    // assert_eq!(mock::ROUND, round);
    //
    // // query decimals
    // let decimals: u8 = env
    //     .router
    //     .wrap()
    //     .query_wasm_smart(&env.hello_world_addr, &QueryMsg::Decimals {})
    //     .unwrap();
    // assert_eq!(mock::DECIMALS, decimals);
}
