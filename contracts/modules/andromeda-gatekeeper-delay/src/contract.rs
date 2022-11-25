use std::convert::TryInto;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, ensure, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError,
};
use cw2::{get_contract_version, set_contract_version};

use crate::state::{next_id, COUNTER, QUEUE};
use ado_base::ADOContract;
use andromeda_modules::gatekeeper_common::{
    is_legacy_owner, update_legacy_owner, InstantiateMsg, LEGACY_OWNER,
};
use andromeda_modules::gatekeeper_delay::{
    AllTransactionsResponse, DelayedMsg, ExecuteMsg, MigrateMsg, QueryMsg, TransactionResponse,
};
use common::{
    ado_base::{hooks::AndromedaHook, AndromedaQuery, InstantiateMsg as BaseInstantiateMsg},
    encode_binary,
    error::ContractError,
};

use semver::Version;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-gatekeeper-delay";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    LEGACY_OWNER.save(deps.storage, &msg.legacy_owner)?;
    COUNTER.save(deps.storage, &0u64)?;
    ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "address-list".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            modules: None,
            primitive_contract: None,
        },
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AndrReceive(msg) => {
            ADOContract::default().execute(deps, env, info, msg, execute)
        }
        ExecuteMsg::BeginTransaction {
            message,
            delay_seconds,
        } => begin_transaction(deps, info, env, message, delay_seconds),
        ExecuteMsg::CancelTransaction { txnumber } => cancel_transaction(deps, info, txnumber),
        ExecuteMsg::CompleteTransaction { txnumber } => {
            complete_transaction(deps, info, env, txnumber)
        }
        ExecuteMsg::UpdateLegacyOwner { new_owner } => {
            let valid_new_owner = deps.api.addr_validate(&new_owner)?;
            update_legacy_owner(deps, info, valid_new_owner)
        }
    }
}

/// Creates a `DelayedMsg` which will only be executable by `complete_transaction`
/// once the delay has passed. Until then, it can be cancelled.
fn begin_transaction(
    mut deps: DepsMut,
    info: MessageInfo,
    env: Env,
    message: CosmosMsg,
    delay: u64,
) -> Result<Response, ContractError> {
    // nonpayable(&info)?;
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.as_ref().storage, info.sender.as_str())?
            || is_legacy_owner(deps.as_ref(), info.sender)?,
        ContractError::Unauthorized {}
    );
    let txnumber = next_id(deps.branch())?;
    QUEUE.save(
        deps.storage,
        &txnumber.to_ne_bytes(),
        &DelayedMsg::new(env.block.time.seconds() + delay, message.clone()),
    )?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "begin_transaction"),
        attr("transaction", format!("{:?}", message)),
        attr("pending_txnumber", format!("{}", txnumber)),
    ]))
}

/// Deletes a `DelayedMsg` which has been begun but not completed.
fn cancel_transaction(
    deps: DepsMut,
    info: MessageInfo,
    txnumber: u64,
) -> Result<Response, ContractError> {
    // nonpayable(&info)?;
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.as_ref().storage, info.sender.as_str())?
            || is_legacy_owner(deps.as_ref(), info.sender)?,
        ContractError::Unauthorized {}
    );
    QUEUE.remove(deps.storage, &txnumber.to_ne_bytes());

    Ok(Response::new().add_attributes(vec![
        attr("action", "remove_transaction"),
        attr("removed_txnumber", txnumber.to_string()),
    ]))
}

/// Completes a `DelayedMsg` if its delay has expired. Note that currently this
/// can be done by any party; it does not require owner authorization, as they
/// have already authorized that the transaction begin.
fn complete_transaction(
    deps: DepsMut,
    _info: MessageInfo,
    env: Env,
    txnumber: u64,
) -> Result<Response, ContractError> {
    // nonpayable(&info)?;
    let msg_to_add: DelayedMsg = QUEUE.load(deps.storage, &txnumber.to_ne_bytes())?;
    ensure!(
        msg_to_add.check_expiration(env.block.time)?,
        ContractError::Std(StdError::GenericErr {
            msg: format!("Delay still in progress for tx number {}", txnumber)
        })
    );
    QUEUE.remove(deps.storage, &txnumber.to_ne_bytes());
    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "complete_transaction"),
            attr("completed_txnumber", format!("{}", txnumber)),
        ])
        .add_message(msg_to_add.get_message()))
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

fn from_semver(err: semver::Error) -> StdError {
    StdError::generic_err(format!("Semver: {}", err))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrHook(msg) => handle_andr_hook(deps, msg),
        QueryMsg::AndrQuery(msg) => handle_andromeda_query(deps, env, msg),
        QueryMsg::TransactionInProgress { txnumber } => {
            Ok(to_binary(&query_transaction_in_progress(deps, txnumber)?)?)
        }
        QueryMsg::AllTransactionsInProgress {} => {
            Ok(to_binary(&query_all_transactions_in_progress(deps)?)?)
        }
    }
}

fn handle_andr_hook(_deps: Deps, msg: AndromedaHook) -> Result<Binary, ContractError> {
    match msg {
        AndromedaHook::OnExecute { sender: _, .. } => {
            /* let is_included = includes_address(deps.storage, &sender)?;
            let is_inclusive = IS_INCLUSIVE.load(deps.storage)?;
            if is_included != is_inclusive {
                Err(ContractError::Unauthorized {})
            } else { */
            Ok(to_binary(&None::<Response>)?)
            // }
        }
        _ => Ok(to_binary(&None::<Response>)?),
    }
}

fn handle_andromeda_query(
    deps: Deps,
    env: Env,
    msg: AndromedaQuery,
) -> Result<Binary, ContractError> {
    match msg {
        AndromedaQuery::Get(_data) => {
            /*let address: String = parse_message(&data)?;
            encode_binary(&query_address(deps, &address)?)*/
            encode_binary(&cosmwasm_std::Empty {})
        }
        _ => ADOContract::default().query(deps, env, msg, query),
    }
}

fn query_transaction_in_progress(
    deps: Deps,
    txnumber: u64,
) -> Result<TransactionResponse, ContractError> {
    Ok(TransactionResponse {
        delayed_transaction: QUEUE.load(deps.storage, &txnumber.to_ne_bytes())?,
    })
}

// no pagination yet
fn query_all_transactions_in_progress(
    deps: Deps,
) -> Result<AllTransactionsResponse, ContractError> {
    let txs = QUEUE
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .into_iter()
        .map(|i| {
            // bad unwraps here, fix is todo
            let j = i.unwrap();
            (u64::from_ne_bytes(j.0.try_into().unwrap()), j.1)
        })
        .collect::<Vec<(u64, DelayedMsg)>>();
    Ok(AllTransactionsResponse {
        transactions_with_ids: txs,
    })
}
