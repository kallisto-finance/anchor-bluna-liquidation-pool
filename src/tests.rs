use crate::contract::{execute, instantiate, query};
use crate::msg::{ExecuteMsg, InstantiateMsg};
use cosmwasm_std::{Addr, Coin, coin, Empty, Uint128};
use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor};

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
    use crate::msg::{BidsResponse, Cw20BalanceResponse, ExternalQueryMsg};
    use cosmwasm_std::{Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult, to_binary, Uint128};
    use cw_multi_test::{Contract, ContractWrapper};

    pub type InstantiateMsg = ();
    pub type ExecuteMsg = ();

    pub fn b_luna_contract() -> Box<dyn Contract<Empty>> {
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
                ExternalQueryMsg::Balance { address: _ } => to_binary(&Cw20BalanceResponse{ balance: Uint128::zero() }),
                _ => unimplemented!(),
            }
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
                ExternalQueryMsg::BidsByUser { .. } => to_binary(&BidsResponse{ bids: vec![] }),
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

    let b_luna_addr = router
        .instantiate_contract(b_luna_contract_id, owner.clone(), &(), &[], "b_luna", None)
        .unwrap();

    let anchor_queue_addr = router
        .instantiate_contract(anchor_queue_contract_id, owner.clone(), &(), &[], "anchor_queue", None)
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
                price_oracle: None,
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
fn it_works() {
    let mut env = setup();
    env.router.init_bank_balance(&env.owner, vec![coin(1000000u128, "uusd")]).unwrap();

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
        print!("{}", res.unwrap_err().to_string());
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
