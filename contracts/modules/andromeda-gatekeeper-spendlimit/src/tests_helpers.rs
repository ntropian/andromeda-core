use andromeda_modules::gatekeeper_spendlimit::{CanSpendResponse, ExecuteMsg, InstantiateMsg};
use andromeda_modules::permissioned_address::{CoinLimit, PeriodType, PermissionedAddressParams};
use cosmwasm_std::{BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response};

use crate::contract::{can_spend, execute, query_permissioned_addresses};
use crate::error::ContractError;
use crate::tests_contract::PERMISSIONED_ADDRESS;
pub const LEGACY_OWNER_STR: &str = "alice";

const ASSET_UNIFIER_CONTRACT_ADDRESS: &str = "asset_unifier_contract_address";

pub fn get_test_instantiate_message(env: Env) -> InstantiateMsg {
    // instantiate the contract

    InstantiateMsg {
        legacy_owner: Some(LEGACY_OWNER_STR.to_string()),
        permissioned_addresses: vec![PermissionedAddressParams {
            address: PERMISSIONED_ADDRESS.to_string(),
            cooldown: env.block.time.seconds() as u64, // this is fine since it will calc on first spend
            period_type: PeriodType::DAYS,
            period_multiple: 1,
            spend_limits: vec![CoinLimit {
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
                amount: 1_000_000u64,
                limit_remaining: 1_000_000u64,
            }],
            usdc_denom: Some("true".to_string()),
            default: Some(true),
        }],
    }
}

#[allow(clippy::too_many_arguments)]
pub fn add_test_permissioned_address(
    mut deps: DepsMut,
    address: String,
    current_env: Env,
    info: MessageInfo,
    period_multiple: u16,
    period_type: PeriodType,
    limit: u64,
) -> Result<Response, ContractError> {
    let res = query_permissioned_addresses(deps.as_ref()).unwrap();
    let old_length = res.permissioned_addresses.len();
    let execute_msg = ExecuteMsg::UpsertPermissionedAddress {
        new_permissioned_address: PermissionedAddressParams {
            address,
            cooldown: current_env.block.time.seconds() as u64,
            period_type,
            period_multiple,
            spend_limits: vec![CoinLimit {
                denom: "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034"
                    .to_string(),
                amount: limit,
                limit_remaining: limit,
            }],
            usdc_denom: Some("true".to_string()),
            default: Some(true),
        },
    };

    let _res = execute(deps.branch(), current_env, info, execute_msg).unwrap();
    let res = query_permissioned_addresses(deps.as_ref()).unwrap();
    assert!(res.permissioned_addresses.len() == old_length + 1);
    Ok(Response::new())
}

pub fn test_spend_bank(
    deps: DepsMut,
    current_env: Env,
    to_address: String,
    amount: Vec<Coin>,
    info: MessageInfo,
) -> Result<CanSpendResponse, ContractError> {
    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address,
        amount: amount.clone(),
    });
    let res = can_spend(
        deps.as_ref(),
        current_env,
        info.sender.to_string(),
        amount,
        vec![send_msg],
        ASSET_UNIFIER_CONTRACT_ADDRESS.to_string(),
    );
    let unwrapped_res = match res {
        Ok(res) => res,
        Err(e) => {
            return Err(e);
        }
    };
    assert!(unwrapped_res.0.can_spend);
    Ok(unwrapped_res.0)
}
