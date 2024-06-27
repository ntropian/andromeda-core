use andromeda_fungible_tokens::cw20::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{AndromedaMsg, AndromedaQuery, InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    amp::AndrAddr,
    common::{
        actions::call_action, call_action::get_action_name, context::ExecuteContext, encode_binary,
        Funds,
    },
    error::ContractError,
};
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_json, to_json_binary, Addr, Api, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Storage, SubMsg, Uint128, WasmMsg,
};

use cw20::{Cw20Coin, Cw20ExecuteMsg};
use cw20_base::{
    contract::{execute as execute_cw20, instantiate as cw20_instantiate, query as cw20_query},
    state::BALANCES,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-cw20";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let cw20_resp = cw20_instantiate(deps.branch(), env.clone(), info.clone(), msg.clone().into())?;
    let resp = contract.instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address.clone(),
            owner: msg.clone().owner,
        },
    )?;

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

pub fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action = get_action_name(CONTRACT_NAME, msg.as_ref());

    let _contract = ADOContract::default();
    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;

    let res = match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_transfer(ctx, recipient, amount, action)
        }
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => execute_transfer_from(ctx, recipient, owner, amount, action),
        ExecuteMsg::Burn { amount } => execute_burn(ctx, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(ctx, contract, amount, msg, action),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => {
            let contract = contract.get_raw_address(&ctx.deps.as_ref())?.into_string();
            let msg = Cw20ExecuteMsg::SendFrom {
                owner,
                contract,
                amount,
                msg,
            };
            Ok(execute_cw20(ctx.deps, ctx.env, ctx.info, msg)?)
        }
        ExecuteMsg::Mint { recipient, amount } => execute_mint(ctx, recipient, amount),
        _ => {
            let serialized = encode_binary(&msg)?;
            match from_json::<AndromedaMsg>(&serialized) {
                Ok(msg) => ADOContract::default().execute(ctx, msg),
                _ => Ok(execute_cw20(ctx.deps, ctx.env, ctx.info, msg.into())?),
            }
        }
    }?;
    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

fn execute_transfer(
    ctx: ExecuteContext,
    recipient: AndrAddr,
    amount: Uint128,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let transfer_response = ADOContract::default().query_deducted_funds(
        deps.as_ref(),
        action,
        Funds::Cw20(Cw20Coin {
            address: env.contract.address.to_string(),
            amount,
        }),
    )?;
    match transfer_response {
        Some(transfer_response) => {
            let remaining_amount = match transfer_response.leftover_funds {
                Funds::Native(..) => amount, //What do we do in the case that the rates returns remaining amount as native funds?
                Funds::Cw20(coin) => coin.amount,
            };

            let mut resp = filter_out_cw20_messages(
                transfer_response.msgs,
                deps.storage,
                deps.api,
                &info.sender,
            )?;

            let recipient = recipient.get_raw_address(&deps.as_ref())?.into_string();
            // Continue with standard cw20 operation
            let cw20_resp = execute_cw20(
                deps,
                env,
                info,
                Cw20ExecuteMsg::Transfer {
                    recipient,
                    amount: remaining_amount,
                },
            )?;
            resp = resp
                .add_submessages(cw20_resp.messages)
                .add_attributes(cw20_resp.attributes)
                .add_events(transfer_response.events);
            Ok(resp)
        }
        None => {
            let recipient = recipient.get_raw_address(&deps.as_ref())?.into_string();
            // Continue with standard cw20 operation
            let cw20_resp = execute_cw20(
                deps,
                env,
                info,
                Cw20ExecuteMsg::Transfer { recipient, amount },
            )?;
            Ok(cw20_resp)
        }
    }
}
fn execute_transfer_from(
    ctx: ExecuteContext,
    recipient: AndrAddr,
    owner: String,
    amount: Uint128,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let transfer_response = ADOContract::default().query_deducted_funds(
        deps.as_ref(),
        action,
        Funds::Cw20(Cw20Coin {
            address: env.contract.address.to_string(),
            amount,
        }),
    )?;
    match transfer_response {
        Some(transfer_response) => {
            let remaining_amount = match transfer_response.leftover_funds {
                Funds::Native(..) => amount, //What do we do in the case that the rates returns remaining amount as native funds?
                Funds::Cw20(coin) => coin.amount,
            };

            let mut resp = filter_out_cw20_messages(
                transfer_response.msgs,
                deps.storage,
                deps.api,
                &info.sender,
            )?;

            let recipient = recipient.get_raw_address(&deps.as_ref())?.into_string();
            // Continue with standard cw20 operation
            let cw20_resp = execute_cw20(
                deps,
                env,
                info,
                Cw20ExecuteMsg::TransferFrom {
                    recipient,
                    owner,
                    amount: remaining_amount,
                },
            )?;
            resp = resp
                .add_submessages(cw20_resp.messages)
                .add_attributes(cw20_resp.attributes)
                .add_events(transfer_response.events);
            Ok(resp)
        }
        None => {
            let recipient = recipient.get_raw_address(&deps.as_ref())?.into_string();
            // Continue with standard cw20 operation
            let cw20_resp = execute_cw20(
                deps,
                env,
                info,
                Cw20ExecuteMsg::TransferFrom {
                    recipient,
                    owner,
                    amount,
                },
            )?;
            Ok(cw20_resp)
        }
    }
}

fn transfer_tokens(
    storage: &mut dyn Storage,
    sender: &Addr,
    recipient: &Addr,
    amount: Uint128,
) -> Result<(), ContractError> {
    BALANCES.update(
        storage,
        sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        storage,
        recipient,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_add(amount)?)
        },
    )?;
    Ok(())
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
    contract: AndrAddr,
    amount: Uint128,
    msg: Binary,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let rates_response = ADOContract::default().query_deducted_funds(
        deps.as_ref(),
        action,
        Funds::Cw20(Cw20Coin {
            address: env.contract.address.to_string(),
            amount,
        }),
    )?;
    match rates_response {
        Some(rates_response) => {
            let remaining_amount = match rates_response.leftover_funds {
                Funds::Native(..) => amount, //What do we do in the case that the rates returns remaining amount as native funds?
                Funds::Cw20(coin) => coin.amount,
            };

            let mut resp = filter_out_cw20_messages(
                rates_response.msgs,
                deps.storage,
                deps.api,
                &info.sender,
            )?;
            let contract = contract.get_raw_address(&deps.as_ref())?.to_string();
            let cw20_resp = execute_cw20(
                deps,
                env,
                info,
                Cw20ExecuteMsg::Send {
                    contract,
                    amount: remaining_amount,
                    msg,
                },
            )?;
            resp = resp
                .add_submessages(cw20_resp.messages)
                .add_attributes(cw20_resp.attributes)
                .add_events(rates_response.events);

            Ok(resp)
        }
        None => {
            let contract = contract.get_raw_address(&deps.as_ref())?.to_string();
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
    }
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

fn filter_out_cw20_messages(
    msgs: Vec<SubMsg>,
    storage: &mut dyn Storage,
    api: &dyn Api,
    sender: &Addr,
) -> Result<Response, ContractError> {
    let mut resp: Response = Response::new();
    // Filter through payment messages to extract cw20 transfer messages to avoid looping
    for sub_msg in msgs {
        // Transfer messages are CosmosMsg::Wasm type
        if let CosmosMsg::Wasm(WasmMsg::Execute { msg: exec_msg, .. }) = sub_msg.msg.clone() {
            // If binary deserializes to a Cw20ExecuteMsg check the message type
            if let Ok(Cw20ExecuteMsg::Transfer { recipient, amount }) =
                from_json::<Cw20ExecuteMsg>(&exec_msg)
            {
                transfer_tokens(storage, sender, &api.addr_validate(&recipient)?, amount)?;
            } else {
                resp = resp.add_submessage(sub_msg);
            }
        } else {
            resp = resp.add_submessage(sub_msg);
        }
    }
    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    let serialized = to_json_binary(&msg)?;
    match from_json::<AndromedaQuery>(&serialized) {
        Ok(msg) => ADOContract::default().query(deps, env, msg),
        _ => Ok(cw20_query(deps, env, msg.into())?),
    }
}
