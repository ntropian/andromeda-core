use andromeda_gatekeeper_spendlimit::constants::JUNO_MAINNET_AXLUSDC_IBC;
use andromeda_modules::{
    gatekeeper_spendlimit::CanSpendResponse,
    permissioned_address::{PeriodType, PermissionedAddressParams, PermissionedAddresssResponse},
    user_account::UserAccount, gatekeeper_common::UniversalMsg,
};
use cosmwasm_std::{Addr, BlockInfo, Coin, Empty, Timestamp, Uint128, CosmosMsg, BankMsg};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use dummy_price_contract::msg::AssetPrice;

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
        andromeda_gatekeeper_spendlimit::contract::execute,
        andromeda_gatekeeper_spendlimit::contract::instantiate,
        andromeda_gatekeeper_spendlimit::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
fn gatekeeper_message_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        andromeda_gatekeeper_message::contract::execute,
        andromeda_gatekeeper_message::contract::instantiate,
        andromeda_gatekeeper_message::contract::query,
    );
    Box::new(contract)
}

#[allow(dead_code)]
fn user_account_contract() -> Box<dyn Contract<Empty>> {
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
    starting_usd_debt: Option<u64>,
    owner_updates_delay_secs: Option<u64>,
) -> andromeda_modules::user_account::InstantiateMsg {
    andromeda_modules::user_account::InstantiateMsg {
        account: UserAccount {
            legacy_owner,
            spendlimit_gatekeeper_contract_addr,
            message_gatekeeper_contract_addr,
            delay_gatekeeper_contract_addr: None,
            sessionkey_gatekeeper_contract_addr: None,
            debt_gatekeeper_contract_addr: None,
        },
        starting_usd_debt,
        owner_updates_delay_secs,
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
    let gatekeeper_message_contract_code_id = router.store_code(gatekeeper_message_contract());
    let user_account_code_id = router.store_code(user_account_contract());

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

    // setup spendlimit gatekeeper contract
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

    // Setup message gatekeeper contract
    let init_msg = andromeda_modules::gatekeeper_message::InstantiateMsg {
        legacy_owner: Some(legacy_owner.to_string()),
    };
    // Instantiate the gatekeeper message contract
    let gatekeeper_message_contract_addr = router
        .instantiate_contract(
            gatekeeper_message_contract_code_id,
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
            spendlimit_gatekeeper_contract_addr: Some(gatekeeper_spendlimit_contract_addr.to_string()),
            delay_gatekeeper_contract_addr: None,
            message_gatekeeper_contract_addr: Some(gatekeeper_message_contract_addr.to_string()),
            sessionkey_gatekeeper_contract_addr: None,
            debt_gatekeeper_contract_addr: None },
        starting_usd_debt: Some(10000u64),
        owner_updates_delay_secs: Some(10u64),
    };
    // Instantiate the user account contract
    let user_account_contract_addr = router
    .instantiate_contract(
        user_account_code_id,
        legacy_owner.clone(),
        &init_msg,
        &[],
        "user_account",
        None,
    )
    .unwrap();

    let authorized_spender = "alice".to_string();

    let block_info: BlockInfo = router.block_info();

    println!("\x1b[1;33;4m*** Contracts Instantiated Successfully ***\x1b[0m");
    // We can now start executing actions on the contract and querying it as needed

    println!("\x1b[1;33;4m*** Test 1: Non-Owner cannot update legacy owner ***\x1b[0m");
    let update_owner_msg = andromeda_modules::user_account::ExecuteMsg::UpdateLegacyOwner {
        new_owner: "alice".to_string()
    };
    let _ = router
        .execute_contract(
            Addr::unchecked(authorized_spender.clone()),
            user_account_contract_addr.clone(),
            &update_owner_msg,
            &[],
        )
        .unwrap_err();
    println!("\x1b[1;32m...success\x1b[0m");
    println!("");

    println!("\x1b[1;33;4m*** Test 2: Add a permissioned user with a $100 daily spend limit ***\x1b[0m");
    // Let's have alice added as a permissioned user
    let msg = andromeda_modules::gatekeeper_spendlimit::ExecuteMsg::UpsertPermissionedAddress {
        new_permissioned_address: PermissionedAddressParams {
            address: authorized_spender.clone(),
            cooldown: block_info.time.seconds().checked_add(86400).unwrap(),
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![andromeda_modules::permissioned_address::CoinLimit {
                denom: JUNO_MAINNET_AXLUSDC_IBC
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
    println!("\x1b[1;32m...success\x1b[0m");
    println!("");

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
    // we could query with andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend,
    // but this is an integration test
    println!("\x1b[1;33;4m*** Test 3: Check that permissioned user can spend $99 ***\x1b[0m");
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        funds: vec![],
        msg: UniversalMsg::Legacy(CosmosMsg::Bank(BankMsg::Send {
            to_address: "bob".to_string(),
            amount: vec![Coin {
                denom: JUNO_MAINNET_AXLUSDC_IBC
                    .to_string(),
                amount: Uint128::from(99_000_000u128),
            }],
        })),
    };

    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(user_account_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("\x1b[1;32m...success\x1b[0m");
    println!("");

    // spending it should update the spend limit (not implemented here; called by the account module)
    // so let's manually update
    // note that only limit remaining changes (safer implementation todo)
    println!("\x1b[1;33;4m*** Test 4: Manually reduce today's spending limit to $1 ***\x1b[0m");
    let msg =
        andromeda_modules::gatekeeper_spendlimit::ExecuteMsg::UpdatePermissionedAddressSpendLimit {
            permissioned_address: authorized_spender.clone(),
            new_spend_limits: andromeda_modules::permissioned_address::CoinLimit {
                denom: JUNO_MAINNET_AXLUSDC_IBC
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
    println!("\x1b[1;32m...success\x1b[0m");
    println!("");

    // now we should NOT be able to spend even $2
    println!("\x1b[1;33;4m*** Test 5: Try (and fail) to send $2 ***\x1b[0m");
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend {
        sender: authorized_spender.clone(),
        funds: vec![Coin {
            denom: JUNO_MAINNET_AXLUSDC_IBC
                .to_string(),
            amount: Uint128::from(2_000_000u128),
        }],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(!can_spend_response.can_spend);
    // note that the above errors instead of returning false. Maybe a todo
    println!("\x1b[1;32m...failed as expected\x1b[0m");
    println!("");

    // nor can we spend 2 "ujunox"
    println!("\x1b[1;33;4m*** Test 6: Try (and fail) to send 2 Juno (valued by dummy dex at $4.56 each) ***\x1b[0m");
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend {
        sender: authorized_spender.clone(),
        funds: vec![Coin {
            denom: "ujunox".to_string(),
            amount: Uint128::from(2_000_000u128),
        }],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(!can_spend_response.can_spend);
    println!("\x1b[1;32m...failed as expected\x1b[0m");
    println!("");

    // but we can spend $1
    println!("\x1b[1;33;4m*** Test 7: Check we can spend $1 ***\x1b[0m");
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend {
        sender: authorized_spender.clone(),
        funds: vec![Coin {
            denom: JUNO_MAINNET_AXLUSDC_IBC
                .to_string(),
            amount: Uint128::from(1_000_000u128),
        }],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("\x1b[1;32m...success\x1b[0m");
    println!("");

    // or 0.1 JUNO
    println!("\x1b[1;33;4m*** Test 8: Check we can spend 0.1 Juno ($0.45) ***\x1b[0m");
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend {
        sender: authorized_spender.clone(),
        funds: vec![Coin {
            denom: "ujunox".to_string(),
            amount: Uint128::from(100_000u128),
        }],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("\x1b[1;32m...success\x1b[0m");
    println!("");
    

    println!("\x1b[1;33;4m*** Test 9: Go forward 1 day, and now we can spend $2 since limit has reset ***\x1b[0m");
    let old_block_info = router.block_info();
    router.set_block(BlockInfo {
        height: old_block_info.height + 17280,
        time: Timestamp::from_seconds(old_block_info.time.seconds() + 86400),
        chain_id: old_block_info.chain_id,
    });

    // and we can spend $2 now
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend {
        sender: authorized_spender.clone(),
        funds: vec![Coin {
            denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                .to_string(),
            amount: Uint128::from(2_000_000u128),
        }],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("\x1b[1;32m...success\x1b[0m");
    println!("");

    println!("\x1b[1;33;4m*** Test 10: We can spend 2 Juno now as well ***\x1b[0m");
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend {
        sender: authorized_spender.clone(),
        funds: vec![Coin {
            denom: "ujunox".to_string(),
            amount: Uint128::from(2_000_000u128),
        }],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(gatekeeper_spendlimit_contract_addr.clone(), &query_msg)
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("\x1b[1;32m...success\x1b[0m");
    println!("");

}
