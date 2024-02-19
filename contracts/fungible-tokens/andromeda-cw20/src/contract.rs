use andromeda_fungible_tokens::cw20::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{AndromedaMsg, AndromedaQuery, InstantiateMsg as BaseInstantiateMsg},
    ado_contract::ADOContract,
    common::{context::ExecuteContext, encode_binary},
    error::{from_semver, ContractError},
};
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, from_json, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, Uint128,
};

use cw2::{get_contract_version, set_contract_version};
use cw20::Cw20ExecuteMsg;
use cw20_base::contract::{
    execute as execute_cw20, instantiate as cw20_instantiate, query as cw20_query,
};
use semver::Version;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-cw20";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let contract = ADOContract::default();
    let resp = contract.instantiate(
        deps.storage,
        env.clone(),
        deps.api,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: "cw20".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            kernel_address: msg.clone().kernel_address,
            owner: msg.clone().owner,
        },
    )?;

    let cw20_resp = cw20_instantiate(deps, env, info, msg.into())?;

    Ok(resp
        .add_submessages(cw20_resp.messages)
        .add_attributes(cw20_resp.attributes))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);

    match msg {
        ExecuteMsg::AMPReceive(pkt) => {
            ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
        }
        _ => handle_execute(ctx, msg),
    }
}

pub fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer { recipient, amount } => execute_transfer(ctx, recipient, amount),
        ExecuteMsg::Burn { amount } => execute_burn(ctx, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(ctx, contract, amount, msg),
        ExecuteMsg::Mint { recipient, amount } => execute_mint(ctx, recipient, amount),
        _ => {
            let serialized = encode_binary(&msg)?;
            match from_json::<AndromedaMsg>(&serialized) {
                Ok(msg) => ADOContract::default().execute(ctx, msg),
                _ => Ok(execute_cw20(ctx.deps, ctx.env, ctx.info, msg.into())?),
            }
        }
    }
}

fn execute_transfer(
    ctx: ExecuteContext,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    // Continue with standard cw20 operation
    let cw20_resp = execute_cw20(
        deps,
        env,
        info,
        Cw20ExecuteMsg::Transfer { recipient, amount },
    )?;
    Ok(cw20_resp)
}

fn execute_burn(ctx: ExecuteContext, amount: Uint128) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    Ok(execute_cw20(
        deps,
        env,
        info,
        Cw20ExecuteMsg::Burn { amount },
    )?)
}

fn execute_send(
    ctx: ExecuteContext,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let cw20_resp = execute_cw20(
        deps,
        env,
        info,
        Cw20ExecuteMsg::Send {
            contract,
            amount,
            msg,
        },
    )?;

    Ok(cw20_resp)
}

fn execute_mint(
    ctx: ExecuteContext,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    Ok(execute_cw20(
        deps,
        env,
        info,
        Cw20ExecuteMsg::Mint { recipient, amount },
    )?)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // New version
    let version: Version = CONTRACT_VERSION.parse().map_err(from_semver)?;

    // Old version
    let stored = get_contract_version(deps.storage)?;
    let storage_version: Version = stored.version.parse().map_err(from_semver)?;

    let contract = ADOContract::default();

    ensure!(
        stored.contract == CONTRACT_NAME,
        ContractError::CannotMigrate {
            previous_contract: stored.contract,
        }
    );

    // New version has to be newer/greater than the old version
    ensure!(
        storage_version < version,
        ContractError::CannotMigrate {
            previous_contract: stored.version,
        }
    );

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Update the ADOContract's version
    contract.execute_update_version(deps)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    let serialized = to_json_binary(&msg)?;
    match from_json::<AndromedaQuery>(&serialized) {
        Ok(msg) => ADOContract::default().query(deps, env, msg),
        _ => Ok(cw20_query(deps, env, msg.into())?),
    }
}
