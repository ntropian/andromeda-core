use andromeda_modules::{permissioned_address::{
    PeriodType, PermissionedAddressParams, PermissionedAddresssResponse,
}, gatekeeper_spendlimit::CanSpendResponse};
use anyhow::{anyhow, Result};
use common::ado_base::ownership;
use cosmwasm_std::{
    to_binary, Addr, CosmosMsg, Empty, QueryRequest, StdError, Uint128, WasmMsg, WasmQuery, BankMsg, Coin, BlockInfo,
};
use cw_multi_test::{App, AppResponse, Contract, ContractWrapper, Executor};
use derivative::Derivative;
use dummy_price_contract::msg::AssetPrice;
use serde::{de::DeserializeOwned, Serialize};

#[allow(dead_code)]
fn mock_app() -> App {
    App::default()
}

#[allow(dead_code)]
fn unified_asset_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        andromeda_unified_asset::contract::execute,
        andromeda_unified_asset::contract::instantiate,
        andromeda_unified_asset::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
fn dummy_price_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dummy_price_contract::contract::execute,
        dummy_price_contract::contract::instantiate,
        dummy_price_contract::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
fn gatekeeper_spendlimit_contract() -> Box<dyn Contract<Empty>> {
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
        legacy_owner: Some("alice".to_string()),
        unified_price_contract: Some(price_contract),
    }
}

#[test]
fn spendlimit_gatekeeper_multi_test() {
    // Create the owner account
    let legacy_owner = Addr::unchecked("owner");

    // Create a mock App to handle state
    let mut router = mock_app();

    // Store code for various contracts
    let asset_unifier_contract_code_id = router.store_code(unified_asset_contract());
    let dummy_price_contract_code_id = router.store_code(dummy_price_contract());
    let gatekeeper_spendlimit_contract_code_id =
        router.store_code(gatekeeper_spendlimit_contract());

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
    let mocked_dummy_contract_addr = router
        .instantiate_contract(
            dummy_price_contract_code_id,
            legacy_owner.clone(),
            &init_msg,
            &[],
            "dummy_price",
            None,
        )
        .unwrap();

    // Setup asset unifier price contract, using dummy price contract address
    let init_msg = asset_unifier_instantiate_msg(
        Some(legacy_owner.to_string()),
        mocked_dummy_contract_addr.to_string(),
    );
    // Instantiate the asset unifier contract
    let mocked_asset_unifier_addr = router
        .instantiate_contract(
            asset_unifier_contract_code_id,
            legacy_owner.clone(),
            &init_msg,
            &[],
            "asset_unifier",
            None,
        )
        .unwrap();

    // last one: setup spendlimit gatekeeper contract (main contract we'll be interacting with)
    let init_msg = andromeda_modules::gatekeeper_spendlimit::InstantiateMsg {
        legacy_owner: Some(legacy_owner.to_string()),
        permissioned_addresses: vec![],
        asset_unifier_contract: mocked_asset_unifier_addr.to_string(),
    };
    // Instantiate the spendlimit gatekeeper contract
    let gatekeeper_spendlimit_contract_addr = router
        .instantiate_contract(
            gatekeeper_spendlimit_contract_code_id,
            legacy_owner.clone(),
            &init_msg,
            &[],
            "gatekeeper_spendlimit",
            None,
        )
        .unwrap();

    let authorized_spender = "alice".to_string();

    let block_info: BlockInfo = router.block_info();
    // We can now start executing actions on the contract and querying it as needed
    let msg = andromeda_modules::gatekeeper_spendlimit::ExecuteMsg::UpsertPermissionedAddress {
        new_permissioned_address: PermissionedAddressParams {
            address: authorized_spender.clone(),
            cooldown: block_info.time.seconds().checked_add(86400).unwrap(),
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![andromeda_modules::permissioned_address::CoinLimit {
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                .to_string(),
                amount: 100_000_000u64,
                limit_remaining: 100_000_000u64,
            }],
            usdc_denom: Some("true".to_string()),
            default: Some(true),
        },
    };
    let _ = router
        .execute_contract(
            legacy_owner.clone(),
            gatekeeper_spendlimit_contract_addr.clone(),
            &msg,
            &[],
        )
        .unwrap();
    // Query the contract to verify we now have a permissioned address
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::PermissionedAddresss {};
    let permissioned_address_response: PermissionedAddresssResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert_eq!(
        permissioned_address_response.permissioned_addresses.len(),
        1
    );

    // we have a $100 USDC spend limit, so we should be able to spend $99...
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend {
        sender: authorized_spender.clone(),
        funds: vec![], msgs: vec![CosmosMsg::Bank(BankMsg::Send {
            to_address: "bob".to_string(),
            amount: vec![Coin {
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
                amount: Uint128::from(99_000_000u128),
            }],
        })],
    };

    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(can_spend_response.can_spend);

    // spending it should update the spend limit (not implemented here; called by the account module)
    // so let's manually update
    // note that only limit remaining changes (safer implementation todo)
    let msg = andromeda_modules::gatekeeper_spendlimit::ExecuteMsg::UpdatePermissionedAddressSpendLimit {
        permissioned_address: authorized_spender.clone(),
        new_spend_limits: andromeda_modules::permissioned_address::CoinLimit {
            denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
            .to_string(),
            amount: 100_000_000u64,
            limit_remaining: 1_000_000u64,
        },
        is_beneficiary: "false".to_string(),
    };
    let _ = router
        .execute_contract(
            legacy_owner.clone(),
            gatekeeper_spendlimit_contract_addr.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Query and output permissioned addresses (for debugging)
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::PermissionedAddresss {};
    let permissioned_address_response: PermissionedAddresssResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    println!(
        "permissioned_address_response: {:?}",
        permissioned_address_response
    );

    // now we should NOT be able to spend even $2
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend {
        sender: authorized_spender.clone(),
        funds: vec![], msgs: vec![CosmosMsg::Bank(BankMsg::Send {
            to_address: "bob".to_string(),
            amount: vec![Coin {
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
                amount: Uint128::from(2_000_000u128),
            }],
        })],
    };

    // note this errors instead of returning false. Maybe a todo
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(!can_spend_response.can_spend);

    // but we can spend $1
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend {
        sender: authorized_spender.clone(),
        funds: vec![], msgs: vec![CosmosMsg::Bank(BankMsg::Send {
            to_address: "bob".to_string(),
            amount: vec![Coin {
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
                amount: Uint128::from(1_000_000u128),
            }],
        })],
    };

    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(can_spend_response.can_spend);

}
