use crate::primitive_keys::{ADDRESSES_TO_CACHE, WORMHOLE_CORE_BRIDGE, WORMHOLE_TOKEN_BRIDGE};
use ado_base::ADOContract;
use andromeda_protocol::wormhole::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use common::{
    ado_base::{recipient::Recipient, InstantiateMsg as BaseInstantiateMsg},
    encode_binary,
    error::ContractError,
    require,
};
use cosmwasm_std::{
    entry_point, from_binary, Addr, Api, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QuerierWrapper, Response, SubMsg, Uint128, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};
use terraswap::asset::{Asset, AssetInfo};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda_wormhole";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let contract = ADOContract::default();
    let resp = contract.instantiate(
        deps.storage,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "wormhole".to_string(),
            operators: None,
            modules: None,
            primitive_contract: Some(msg.primitive_contract),
        },
    )?;
    for address in ADDRESSES_TO_CACHE {
        contract.cache_address(deps.storage, &deps.querier, address)?;
    }
    Ok(resp)
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let wormhole_token_bridge_contract =
        contract.get_cached_address(deps.storage, WORMHOLE_TOKEN_BRIDGE)?;
    match msg {
        ExecuteMsg::AndrReceive(msg) => {
            ADOContract::default().execute(deps, env, info, msg, execute)
        }
        ExecuteMsg::RegisterAssetHook { asset_id } => {
            execute_register_asset_hook(deps, env, info, &asset_id.as_slice())
        }
        ExecuteMsg::InitiateTransfer {
            asset,
            recipient_chain,
            recipient,
            fee,
            nonce,
        } => execute_initiate_transfer(
            deps,
            env,
            info,
            asset,
            recipient_chain,
            recipient.as_slice().to_vec(),
            fee,
            nonce,
        ),
        ExecuteMsg::DepositTokens {} => execute_deposit_tokens(deps, env, info),
        ExecuteMsg::WithdrawTokens { asset } => execute_withdraw_tokens(deps, env, info, asset),
        ExecuteMsg::SubmitVaa { data } => execute_submit_vaa(deps, env, info, data),
    }
}

fn execute_submit_vaa(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    data: Binary,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let token_bridge = contract.get_cached_address(deps.storage, WORMHOLE_TOKEN_BRIDGE)?;
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_bridge,
        msg: encode_binary(&ExecuteMsg::SubmitVaa { data })?,
        funds: vec![],
    });
    let sub_msg = SubMsg::reply_on_success(msg, 0);
    Ok(Response::new()
        .add_attribute("action", "submitted_vaa")
        .add_submessage(sub_msg))
}

fn execute_register_asset_hook(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset_id: &[u8],
) -> Result<Response, ContractError> {
    let binary_asset_id = encode_binary(&asset_id)?;
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: WORMHOLE_TOKEN_BRIDGE.to_string(),
        msg: encode_binary(&ExecuteMsg::RegisterAssetHook {
            asset_id: binary_asset_id,
        })?,
        funds: vec![],
    });
    Ok(Response::new()
        .add_attribute("action", "registered_asset")
        .add_message(msg))
}

fn execute_initiate_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: Asset,
    recipient_chain: u16,
    recipient: Vec<u8>,
    mut fee: Uint128,
    nonce: u32,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let token_bridge = contract.get_cached_address(deps.storage, WORMHOLE_TOKEN_BRIDGE)?;
    let binary_recipient = encode_binary(&recipient)?;
    let msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: token_bridge,
        msg: encode_binary(&ExecuteMsg::InitiateTransfer {
            asset,
            recipient_chain,
            recipient: binary_recipient,
            fee,
            nonce,
        })?,
        funds: vec![],
    });
    Ok(Response::new().add_submessage(msg))
}

fn execute_deposit_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    require(
        !info.funds.is_empty(),
        ContractError::InvalidFunds {
            msg: "No funds detected".to_string(),
        },
    )?;
    let contract = ADOContract::default();
    let token_bridge = contract.get_cached_address(deps.storage, WORMHOLE_TOKEN_BRIDGE)?;
    let funds = info.funds;
    let msg = WasmMsg::Execute {
        contract_addr: token_bridge,
        msg: encode_binary(&ExecuteMsg::DepositTokens {})?,
        funds,
    };
    let sub_msg = SubMsg::reply_on_success(msg, 0);

    Ok(Response::new()
        .add_attribute("action", "deposit_tokens")
        .add_submessage(sub_msg))
}

fn execute_withdraw_tokens(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    asset: AssetInfo,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let token_bridge = contract.get_cached_address(deps.storage, WORMHOLE_TOKEN_BRIDGE)?;
    let msg = WasmMsg::Execute {
        contract_addr: token_bridge,
        msg: encode_binary(&ExecuteMsg::WithdrawTokens { asset })?,
        funds: vec![],
    };
    let sub_msg = SubMsg::reply_on_success(msg, 0);
    Ok(Response::new()
        .add_attribute("action", "withdraw_tokens")
        .add_submessage(sub_msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrQuery(msg) => ADOContract::default().query(deps, env, msg, query),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(ContractError::CannotMigrate {
            previous_contract: version.contract,
        });
    }
    Ok(Response::default())
}
