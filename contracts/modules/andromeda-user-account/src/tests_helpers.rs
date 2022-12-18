use andromeda_modules::user_account::UserAccount;
use cosmwasm_std::{Addr, Empty, Uint128};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use dummy_counter_executable::msg::InstantiateMsg;
use dummy_price_contract::msg::AssetPrice;

const BLUE: &str = "\x1b[1;34m";
const WHITE: &str = "\x1b[0m";

#[allow(dead_code)]
pub fn mock_app() -> App {
    App::default()
}

#[allow(dead_code)]
pub fn asset_unifier_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        andromeda_unified_asset::contract::execute,
        andromeda_unified_asset::contract::instantiate,
        andromeda_unified_asset::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
pub fn dummy_dex_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dummy_price_contract::contract::execute,
        dummy_price_contract::contract::instantiate,
        dummy_price_contract::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
pub fn dummy_executable_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dummy_counter_executable::contract::execute,
        dummy_counter_executable::contract::instantiate,
        dummy_counter_executable::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
pub fn gatekeeper_sessionkey_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        andromeda_gatekeeper_sessionkey::contract::execute,
        andromeda_gatekeeper_sessionkey::contract::instantiate,
        andromeda_gatekeeper_sessionkey::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
pub fn gatekeeper_spendlimit_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        andromeda_gatekeeper_spendlimit::contract::execute,
        andromeda_gatekeeper_spendlimit::contract::instantiate,
        andromeda_gatekeeper_spendlimit::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
pub fn gatekeeper_message_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        andromeda_gatekeeper_message::contract::execute,
        andromeda_gatekeeper_message::contract::instantiate,
        andromeda_gatekeeper_message::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
pub fn user_account_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

pub fn asset_unifier_instantiate_msg(
    legacy_owner: Option<String>,
    price_contract: String,
) -> andromeda_modules::unified_asset::InstantiateMsg {
    andromeda_modules::unified_asset::InstantiateMsg {
        home_network: "multitest".to_string(),
        legacy_owner,
        unified_price_contract: Some(price_contract),
    }
}

pub fn user_account_instantiate_msg(
    legacy_owner: Option<String>,
    spendlimit_gatekeeper_contract_addr: Option<String>,
    message_gatekeeper_contract_addr: Option<String>,
    sessionkey_gatekeeper_contract_addr: Option<String>,
    starting_usd_debt: Option<u64>,
    owner_updates_delay_secs: Option<u64>,
) -> andromeda_modules::user_account::InstantiateMsg {
    andromeda_modules::user_account::InstantiateMsg {
        account: UserAccount {
            legacy_owner,
            owner_updates_delay_secs: owner_updates_delay_secs,
            spendlimit_gatekeeper_contract_addr,
            message_gatekeeper_contract_addr,
            delay_gatekeeper_contract_addr: None,
            sessionkey_gatekeeper_contract_addr,
            debt_gatekeeper_contract_addr: None,
        },
        starting_usd_debt,
    }
}

pub struct CodeIds {
    pub asset_unifier: u64,
    pub dummy_dex: u64,
    pub dummy_enterprise: u64,
    pub gatekeeper_spendlimit: u64,
    pub gatekeeper_message: u64,
    pub gatekeeper_sessionkey: u64,
    pub user_account: u64,
}

pub fn get_code_ids(app: &mut App) -> CodeIds {
    CodeIds {
        asset_unifier: app.store_code(asset_unifier_contract()),
        dummy_dex: app.store_code(dummy_dex_contract()),
        dummy_enterprise: app.store_code(dummy_executable_contract()),
        gatekeeper_spendlimit: app.store_code(gatekeeper_spendlimit_contract()),
        gatekeeper_sessionkey: app.store_code(gatekeeper_sessionkey_contract()),
        gatekeeper_message: app.store_code(gatekeeper_message_contract()),
        user_account: app.store_code(user_account_contract()),
    }
}

pub fn instantiate_contracts(
    router: &mut App,
    code_ids: CodeIds,
    legacy_owner: Addr,
) -> ContractAddresses {
    // Setup dummy price contract
    let init_msg = dummy_price_contract::msg::InstantiateMsg {
        asset_prices: vec![
            AssetPrice {
                denom: "ujunox".to_owned(),
                price: Uint128::from(137_000_000u128),
            },
            AssetPrice {
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_owned(),
                price: Uint128::from(30_000_000u128),
            },
            // not a real contract
            AssetPrice {
                denom: "juno1utkr0ep06rkxgsesq6uryug93daklyd6wneesmtvxjkz0xjlte9qdj2s8q".to_owned(),
                price: Uint128::from(1_000u128),
            },
        ],
    };
    // Instantiate the dummy price contract using its stored code id
    let mocked_dummy_dex_contract_addr = router
        .instantiate_contract(
            code_ids.dummy_dex,
            legacy_owner.clone(),
            &init_msg,
            &[],
            "dummy_price",
            None,
        )
        .unwrap();

    // Instantiate the dummy price contract using its stored code id
    let mocked_dummy_enterprise_contract_addr = router
        .instantiate_contract(
            code_ids.dummy_enterprise,
            legacy_owner.clone(),
            &InstantiateMsg {},
            &[],
            "dummy_enterprise",
            None,
        )
        .unwrap();

    // Setup asset unifier price contract, using dummy price contract address
    let init_msg = asset_unifier_instantiate_msg(
        Some(legacy_owner.to_string()),
        mocked_dummy_dex_contract_addr.to_string(),
    );
    // Instantiate the asset unifier contract
    let mocked_asset_unifier_addr = router
        .instantiate_contract(
            code_ids.asset_unifier,
            legacy_owner.clone(),
            &init_msg,
            &[],
            "asset_unifier",
            None,
        )
        .unwrap();

    // setup sessionkey gatekeeper contract
    let init_msg = andromeda_modules::gatekeeper_common::InstantiateMsg {
        legacy_owner: Some(legacy_owner.to_string()),
    };
    // Instantiate the spendlimit gatekeeper contract
    let gatekeeper_sessionkey_contract_addr = router
        .instantiate_contract(
            code_ids.gatekeeper_sessionkey,
            legacy_owner.clone(),
            &init_msg,
            &[],
            "gatekeeper_sessionkey",
            None,
        )
        .unwrap();

    // setup spendlimit gatekeeper contract
    let init_msg = andromeda_modules::gatekeeper_spendlimit::InstantiateMsg {
        legacy_owner: Some(legacy_owner.to_string()),
        permissioned_addresses: vec![],
        asset_unifier_contract: mocked_asset_unifier_addr.to_string(),
    };
    // Instantiate the spendlimit gatekeeper contract
    let gatekeeper_spendlimit_contract_addr = router
        .instantiate_contract(
            code_ids.gatekeeper_spendlimit,
            legacy_owner.clone(),
            &init_msg,
            &[],
            "gatekeeper_spendlimit",
            None,
        )
        .unwrap();

    // Setup message gatekeeper contract
    let init_msg = andromeda_modules::gatekeeper_message::InstantiateMsg {
        legacy_owner: Some(legacy_owner.to_string()),
    };
    // Instantiate the gatekeeper message contract
    let gatekeeper_message_contract_addr = router
        .instantiate_contract(
            code_ids.gatekeeper_message,
            legacy_owner.clone(),
            &init_msg,
            &[],
            "gatekeeper_message",
            None,
        )
        .unwrap();

    // Last one... Setup user account contract, for now with codes ids in instantiate
    let init_msg = andromeda_modules::user_account::InstantiateMsg {
        account: UserAccount {
            legacy_owner: Some(legacy_owner.to_string()),
            owner_updates_delay_secs: Some(10u64),
            spendlimit_gatekeeper_contract_addr: Some(
                gatekeeper_spendlimit_contract_addr.to_string(),
            ),
            delay_gatekeeper_contract_addr: None,
            message_gatekeeper_contract_addr: Some(gatekeeper_message_contract_addr.to_string()),
            sessionkey_gatekeeper_contract_addr: Some(
                gatekeeper_sessionkey_contract_addr.to_string(),
            ),
            debt_gatekeeper_contract_addr: None,
        },
        starting_usd_debt: Some(10000u64),
    };
    // Instantiate the user account contract
    let user_account_contract_addr = router
        .instantiate_contract(
            code_ids.user_account,
            legacy_owner,
            &init_msg,
            &[],
            "user_account",
            None,
        )
        .unwrap();

    ContractAddresses {
        spendlimit_gatekeeper: gatekeeper_spendlimit_contract_addr,
        message_gatekeeper: gatekeeper_message_contract_addr,
        delay_gatekeeper: Addr::unchecked("Undeployed"),
        sessionkey_gatekeeper: gatekeeper_sessionkey_contract_addr,
        debt_gatekeeper: Addr::unchecked("Undeployed"),
        user_account: user_account_contract_addr,
        asset_unifier: mocked_asset_unifier_addr,
        dummy_price: mocked_dummy_dex_contract_addr,
        dummy_enterprise: mocked_dummy_enterprise_contract_addr,
    }
}

#[derive(Clone)]
pub struct ContractAddresses {
    pub spendlimit_gatekeeper: Addr,
    pub message_gatekeeper: Addr,
    pub delay_gatekeeper: Addr,
    pub sessionkey_gatekeeper: Addr,
    pub debt_gatekeeper: Addr,
    pub user_account: Addr,
    pub asset_unifier: Addr,
    pub dummy_price: Addr,
    pub dummy_enterprise: Addr,
}

pub fn use_contract(addy: Addr, contracts: ContractAddresses, ty: String) -> Addr {
    let contract_human_name = match addy.to_string() {
        val if val == contracts.spendlimit_gatekeeper => "Spendlimit Gatekeeper".to_string(),
        val if val == contracts.message_gatekeeper => "Message Gatekeeper".to_string(),
        val if val == contracts.delay_gatekeeper => "Delay Gatekeeper".to_string(),
        val if val == contracts.sessionkey_gatekeeper => "Session Key Gatekeeper".to_string(),
        val if val == contracts.debt_gatekeeper => "Debt Gatekeeper".to_string(),
        val if val == contracts.user_account => "User Account".to_string(),
        val if val == contracts.asset_unifier => "Asset Unifier".to_string(),
        val if val == contracts.dummy_price => "Dummy DEX".to_string(),
        val if val == contracts.dummy_enterprise => "U.S.S. Executable".to_string(),
        _ => "Unknown contract".to_string(),
    };
    match ty {
        val if val == *"Execute" => {
            println!("Calling contract: {}{}{}", BLUE, contract_human_name, WHITE);
        }
        val if val == *"Query" => {
            println!(
                "Querying contract: {}{}{}",
                BLUE, contract_human_name, WHITE
            );
        }
        _ => panic!("bad type, use execute or query"),
    }
    addy
}
