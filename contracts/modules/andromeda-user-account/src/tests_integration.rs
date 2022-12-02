use andromeda_gatekeeper_spendlimit::constants::JUNO_MAINNET_AXLUSDC_IBC;
use andromeda_modules::{
    gatekeeper_common::UniversalMsg,
    gatekeeper_message::{Authorization, AuthorizationsResponse},
    gatekeeper_spendlimit::CanSpendResponse,
    permissioned_address::{PeriodType, PermissionedAddressParams, PermissionedAddresssResponse},
};
use common::error::ContractError;
use cosmwasm_std::{
    to_binary, Addr, BankMsg, BlockInfo, Coin, CosmosMsg, Timestamp, Uint128, WasmMsg,
};
use cw_multi_test::Executor;



use crate::tests_helpers::{
    get_code_ids, instantiate_contracts, mock_app, use_contract, CodeIds, ContractAddresses,
};

const YELLOW_UNDERLINE: &str = "\x1b[1;33;4m";
const GREEN: &str = "\x1b[1;32m";
const WHITE: &str = "\x1b[0m";
const BLUE: &str = "\x1b[1;34m";
const FORCED_WHITE: &str = "\x1b[1;37m";

#[test]
fn user_account_multi_test() {
    // Create the owner account
    let legacy_owner = Addr::unchecked("owner");

    // Create a mock App to handle state
    let mut router = mock_app();

    // Helper function stores the code for the contracts we will be using
    let code_ids: CodeIds = get_code_ids(&mut router);

    // Helper function instantiates the contracts we will be using
    let contract_addresses: ContractAddresses =
        instantiate_contracts(&mut router, code_ids, legacy_owner.clone());

    // An authorized spend we'll be using for spend limit testing
    let authorized_spender = "alice".to_string();

    // To test resets on recurring spend limits, we advance block_info's time
    let block_info: BlockInfo = router.block_info();

    println!();
    println!("{} ██████╗ ██████╗ ██╗", BLUE);
    println!("{}██╔═══██╗██╔══██╗██║", BLUE);
    println!("{}██║   ██║██████╔╝██║", BLUE);
    println!("{}██║   ██║██╔══██╗██║", BLUE);
    println!("{}╚██████╔╝██████╔╝██║", BLUE);
    println!("{} ╚═════╝ ╚═════╝ ╚═╝", BLUE);
    println!();
    println!("{} User Account Integration Multi-Test", FORCED_WHITE);

    println!(
        "{}*** Contracts Instantiated Successfully ***{}",
        GREEN, WHITE
    );

    println!(
        "{}*** Test 1: Non-Owner cannot update legacy owner ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let update_owner_msg = andromeda_modules::user_account::ExecuteMsg::UpdateLegacyOwner {
        new_owner: "alice".to_string(),
    };
    let _ = router
        .execute_contract(
            Addr::unchecked(authorized_spender.clone()),
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Execute".to_string(),
            ),
            &update_owner_msg,
            &[],
        )
        .unwrap_err();
    println!("{}...success{}", GREEN, WHITE);
    println!();

    println!(
        "{}*** Test 2: Add a permissioned user with a $100 daily spend limit ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    // Let's have alice added as a permissioned user
    let msg = andromeda_modules::gatekeeper_spendlimit::ExecuteMsg::UpsertPermissionedAddress {
        new_permissioned_address: PermissionedAddressParams {
            address: authorized_spender.clone(),
            cooldown: block_info.time.seconds().checked_add(86400).unwrap(),
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![andromeda_modules::permissioned_address::CoinLimit {
                denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
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
            use_contract(
                contract_addresses.spendlimit_gatekeeper.clone(),
                contract_addresses.clone(),
                "Execute".to_string(),
            ),
            &msg,
            &[],
        )
        .unwrap();
    println!("{}...success{}", GREEN, WHITE);
    println!();

    // Query the contract to verify we now have a permissioned address
    let query_msg = andromeda_modules::gatekeeper_spendlimit::QueryMsg::PermissionedAddresss {};
    let permissioned_address_response: PermissionedAddresssResponse = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.spendlimit_gatekeeper.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .unwrap();
    assert_eq!(
        permissioned_address_response.permissioned_addresses.len(),
        1
    );

    // we have a $100 USDC spend limit, so we should be able to spend $99...
    // we could query with andromeda_modules::gatekeeper_spendlimit::QueryMsg::CanSpend,
    // but this is an integration test
    println!(
        "{}*** Test 3a: Check that permissioned user can spend $99 ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        funds: vec![],
        msg: UniversalMsg::Legacy(CosmosMsg::Bank(BankMsg::Send {
            to_address: "bob".to_string(),
            amount: vec![Coin {
                denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
                amount: Uint128::from(99_000_000u128),
            }],
        })),
    };

    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("{}...success{}", GREEN, WHITE);
    println!();

    // spending it should update the spend limit (not implemented here; called by the account module)
    // so let's manually update
    // note that only limit remaining changes (safer implementation todo)
    println!(
        "{}*** Test 3b: Manually reduce today's spending limit to $1 ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let msg =
        andromeda_modules::gatekeeper_spendlimit::ExecuteMsg::UpdatePermissionedAddressSpendLimit {
            permissioned_address: authorized_spender.clone(),
            new_spend_limits: andromeda_modules::permissioned_address::CoinLimit {
                denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
                amount: 100_000_000u64,
                limit_remaining: 1_000_000u64,
            },
            is_beneficiary: "false".to_string(),
        };
    let _ = router
        .execute_contract(
            legacy_owner.clone(),
            use_contract(
                contract_addresses.spendlimit_gatekeeper.clone(),
                contract_addresses.clone(),
                "Execute".to_string(),
            ),
            &msg,
            &[],
        )
        .unwrap();
    println!("{}...success{}", GREEN, WHITE);
    println!();

    // now we should NOT be able to spend even $2
    println!(
        "{}*** Test 3c: Try (and fail) to send $2 ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        msg: {
            UniversalMsg::Legacy(CosmosMsg::Bank(BankMsg::Send {
                to_address: "bob".to_string(),
                amount: vec![Coin {
                    denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
                    amount: Uint128::from(2_000_000u128),
                }],
            }))
        },
        funds: vec![],
    };
    let can_spend_response: Result<CanSpendResponse, ContractError> = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .map_err(ContractError::Std);
    can_spend_response.unwrap_err();
    // note that the above errors instead of returning false. Maybe a todo
    println!("{}...failed as expected{}", GREEN, WHITE);
    println!();

    // nor can we spend 2 "ujunox"
    println!(
        "{}*** Test 3d: Try (and fail) to send 2 Juno (valued by dummy dex at $4.56 each) ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        msg: {
            UniversalMsg::Legacy(CosmosMsg::Bank(BankMsg::Send {
                to_address: "bob".to_string(),
                amount: vec![Coin {
                    denom: "ujunox".to_string(),
                    amount: Uint128::from(2_000_000u128),
                }],
            }))
        },
        funds: vec![],
    };
    let can_spend_response: Result<CanSpendResponse, ContractError> = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .map_err(ContractError::Std);
    can_spend_response.unwrap_err();
    println!("{}...failed as expected{}", GREEN, WHITE);
    println!();

    // but we can spend $1
    println!(
        "{}*** Test 3e: Check we can spend $1 ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        msg: {
            UniversalMsg::Legacy(CosmosMsg::Bank(BankMsg::Send {
                to_address: "bob".to_string(),
                amount: vec![Coin {
                    denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
            }))
        },
        funds: vec![],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("{}...success{}", GREEN, WHITE);
    println!();

    // or 0.1 JUNO
    println!(
        "{}*** Test 3f: Check we can spend 0.1 Juno ($0.45) ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        msg: {
            UniversalMsg::Legacy(CosmosMsg::Bank(BankMsg::Send {
                to_address: "bob".to_string(),
                amount: vec![Coin {
                    denom: "ujunox".to_string(),
                    amount: Uint128::from(100_000u128),
                }],
            }))
        },
        funds: vec![],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("{}...success{}", GREEN, WHITE);
    println!();

    println!(
        "{}*** Test 3g: Go forward 1 day, and now we can spend $2 since limit has reset ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let old_block_info = router.block_info();
    router.set_block(BlockInfo {
        height: old_block_info.height + 17280,
        time: Timestamp::from_seconds(old_block_info.time.seconds() + 86400),
        chain_id: old_block_info.chain_id,
    });

    // and we can spend $2 now
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        msg: {
            UniversalMsg::Legacy(CosmosMsg::Bank(BankMsg::Send {
                to_address: "bob".to_string(),
                amount: vec![Coin {
                    denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
                    amount: Uint128::from(2_000_000u128),
                }],
            }))
        },
        funds: vec![],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("{}...success{}", GREEN, WHITE);
    println!();

    println!(
        "{}*** Test 3h: We can spend 2 Juno now as well ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        msg: {
            UniversalMsg::Legacy(CosmosMsg::Bank(BankMsg::Send {
                to_address: "bob".to_string(),
                amount: vec![Coin {
                    denom: "ujunox".to_string(),
                    amount: Uint128::from(2_000_000u128),
                }],
            }))
        },
        funds: vec![],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!("{}...success{}", GREEN, WHITE);
    println!();

    println!(
        "{}*** Test 4a: Non-owner cannot execute the Kobayashi Maru action, not even without funds ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let execute_msg = dummy_counter_executable::msg::ExecuteMsg::KobayashiMaru {
        captain: "kirk".to_string(),
        strategy: "cheat".to_string(),
    };
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addresses.dummy_enterprise.to_string(),
            msg: to_binary(&execute_msg).unwrap(),
            funds: vec![],
        })),
        funds: vec![],
    };
    let can_spend_response: Result<CanSpendResponse, ContractError> = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .map_err(ContractError::Std);
    can_spend_response.unwrap_err();
    println!("{}...failed as expected{}", GREEN, WHITE);
    println!();

    println!(
        "{}*** Test 4b: Add authorization for alice to KobayashiMaru, with 'kirk' and 'cheat'  ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let add_authorization_msg =
        andromeda_modules::gatekeeper_message::ExecuteMsg::AddAuthorization {
            new_authorization: Authorization {
                identifier: 0u16, // no effect for adding
                actor: Some(Addr::unchecked(authorized_spender.clone())),
                contract: Some(contract_addresses.dummy_enterprise.clone()),
                message_name: Some("MsgExecuteContract".to_string()),
                // remember in direct cases, this should be snake_case
                wasmaction_name: Some("kobayashi_maru".to_string()),
                fields: Some(vec![
                    (String::from("captain"), String::from("kirk")),
                    (String::from("strategy"), String::from("cheat")),
                ]),
            },
        };
    let _ = router
        .execute_contract(
            legacy_owner,
            use_contract(
                contract_addresses.message_gatekeeper.clone(),
                contract_addresses.clone(),
                "Execute".to_string(),
            ),
            &add_authorization_msg,
            &[],
        )
        .unwrap();
    println!("{}...success{}", GREEN, WHITE);
    println!();

    // print out our authorizations
    println!("Current authorizations:");
    let query_msg = andromeda_modules::gatekeeper_message::QueryMsg::Authorizations {
        identifier: None,
        actor: None,
        target_contract: Some(contract_addresses.dummy_enterprise.to_string()),
        message_name: None,
        wasmaction_name: None,
        fields: None,
        limit: None,
        start_after: None,
    };
    let authorizations_response: AuthorizationsResponse = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.message_gatekeeper.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .unwrap();
    println!("authorizations_response: {:?}", authorizations_response);
    println!();

    println!(
        "{}*** Test 4c: Can the authorized actor execute Kobayashi Maru with the wrong fields? ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    use dummy_counter_executable::msg::ExecuteMsg::KobayashiMaru;
    let execute_msg = KobayashiMaru {
        captain: "picard".to_string(),
        strategy: "engage".to_string(),
    };
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addresses.dummy_enterprise.to_string(),
            msg: to_binary(&execute_msg).unwrap(),
            funds: vec![],
        })),
        funds: vec![],
    };
    let can_spend_response: Result<CanSpendResponse, ContractError> = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .map_err(ContractError::Std);
    can_spend_response.unwrap_err();
    println!("{}...of course not, it's impossible{}", GREEN, WHITE);
    println!();

    println!(
        "{}*** Test 4d: What about the right captain, but the wrong strategy? ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let execute_msg = KobayashiMaru {
        captain: "kirk".to_string(),
        strategy: "seduce".to_string(),
    };
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender.clone(),
        msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addresses.dummy_enterprise.to_string(),
            msg: to_binary(&execute_msg).unwrap(),
            funds: vec![],
        })),
        funds: vec![],
    };
    let can_spend_response: Result<CanSpendResponse, ContractError> = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses.clone(),
                "Query".to_string(),
            ),
            &query_msg,
        )
        .map_err(ContractError::Std);
    can_spend_response.unwrap_err();
    println!(
        "{}...nope. One too many Priceline commercials.{}",
        GREEN, WHITE
    );
    println!();

    println!(
        "{}*** Test 4e: But if both fields match the authorization... ***{}",
        YELLOW_UNDERLINE, WHITE
    );
    let execute_msg = KobayashiMaru {
        captain: "kirk".to_string(),
        strategy: "cheat".to_string(),
    };
    let query_msg = andromeda_modules::user_account::QueryMsg::CanExecute {
        address: authorized_spender,
        msg: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addresses.dummy_enterprise.to_string(),
            msg: to_binary(&execute_msg).unwrap(),
            funds: vec![],
        })),
        funds: vec![],
    };
    let can_spend_response: CanSpendResponse = router
        .wrap()
        .query_wasm_smart(
            use_contract(
                contract_addresses.user_account.clone(),
                contract_addresses,
                "Query".to_string(),
            ),
            &query_msg,
        )
        .unwrap();
    assert!(can_spend_response.can_spend);
    println!(
        "{}...success. Unlike Jimmy T, Alice can't cheat.{}",
        GREEN, WHITE
    );
    println!();
}
