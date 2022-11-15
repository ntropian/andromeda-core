use ado_base::state::ADOContract;
use andromeda_fungible_tokens::cw20_exchange::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, Sale,
};
use common::{
    ado_base::{AndromedaQuery, InstantiateMsg as BaseInstantiateMsg},
    // encode_binary,
    error::ContractError,
    // parse_message,
};
use cosmwasm_std::{
    attr, ensure, entry_point, from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env,
    MessageInfo, Reply, Response, StdError, Uint128,
};
use cw2::{get_contract_version, set_contract_version};
use cw20::Cw20ReceiveMsg;
use cw_asset::AssetInfo;
use cw_utils::one_coin;
use semver::Version;

use crate::state::{SALE, TOKEN_ADDRESS};

pub struct ExecuteEnv<'a> {
    deps: DepsMut<'a>,
    env: Env,
    info: MessageInfo,
}

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-cw20-exchange";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    TOKEN_ADDRESS.save(deps.storage, &msg.token_address)?;

    ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "cw20-exchange".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            modules: None,
            primitive_contract: None,
        },
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        return Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        )));
    }

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let execute_env = ExecuteEnv { deps, env, info };
    match msg {
        ExecuteMsg::CancelSale { asset } => execute_cancel_sale(execute_env, asset),
        ExecuteMsg::Purchase { recipient } => execute_purchase_native(execute_env, recipient),
        ExecuteMsg::Receive(cw20_msg) => execute_receive(execute_env, cw20_msg),
        ExecuteMsg::AndrReceive(msg) => ADOContract::default().execute(
            execute_env.deps,
            execute_env.env,
            execute_env.info,
            msg,
            execute,
        ),
    }
}

pub fn execute_receive(
    execute_env: ExecuteEnv,
    receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let sent_asset = AssetInfo::Cw20(execute_env.info.sender.clone());
    let amount_sent = receive_msg.amount;

    match from_binary(&receive_msg.msg)? {
        Cw20HookMsg::StartSale {
            asset,
            exchange_rate,
        } => execute_start_sale(
            execute_env,
            amount_sent,
            asset,
            exchange_rate,
            receive_msg.sender,
        ),
        Cw20HookMsg::Purchase { recipient } => execute_purchase(
            execute_env,
            amount_sent,
            sent_asset,
            recipient.unwrap_or(receive_msg.sender).as_str(),
        ),
    }
}

pub fn execute_start_sale(
    execute_env: ExecuteEnv,
    amount: Uint128,
    asset: AssetInfo,
    exchange_rate: Uint128,
    sender: String,
) -> Result<Response, ContractError> {
    let app_contract = ADOContract::default().get_app_contract(execute_env.deps.storage)?;
    let token_addr = TOKEN_ADDRESS.load(execute_env.deps.storage)?.get_address(
        execute_env.deps.api,
        &execute_env.deps.querier,
        app_contract,
    )?;

    ensure!(
        ADOContract::default().is_contract_owner(execute_env.deps.storage, &sender)?,
        ContractError::Unauthorized {}
    );
    ensure!(
        execute_env.info.sender == token_addr,
        ContractError::InvalidFunds {
            msg: "Incorrect CW20 provided for sale".to_string()
        }
    );

    // Do not allow duplicate sales
    let current_sale = SALE.may_load(execute_env.deps.storage, asset.to_string())?;
    ensure!(current_sale.is_none(), ContractError::SaleNotEnded {});

    let sale = Sale {
        amount,
        exchange_rate,
    };
    SALE.save(execute_env.deps.storage, asset.to_string(), &sale)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "start_sale"),
        attr("asset", asset.to_string()),
        attr("rate", exchange_rate),
        attr("amount", amount),
    ]))
}

pub fn execute_purchase(
    execute_env: ExecuteEnv,
    amount_sent: Uint128,
    asset_sent: AssetInfo,
    recipient: &str,
) -> Result<Response, ContractError> {
    execute_env.deps.api.addr_validate(recipient)?;
    let resp = Response::default();

    Ok(resp)
}

pub fn execute_purchase_native(
    execute_env: ExecuteEnv,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    // Default to sender as recipient
    let recipient = recipient.unwrap_or(execute_env.info.sender.to_string());
    execute_env.deps.api.addr_validate(&recipient)?;

    // Only allow one coin for purchasing
    one_coin(&execute_env.info)?;

    let payment = execute_env.info.funds.first().unwrap();
    let asset = AssetInfo::Native(payment.denom.to_string());
    let amount = payment.amount;

    execute_purchase(execute_env, amount, asset, &recipient)
}

pub fn execute_cancel_sale(
    execute_env: ExecuteEnv,
    asset: AssetInfo,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(execute_env.deps.storage, execute_env.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let resp = Response::default();

    Ok(resp)
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
    // match msg {
    //     QueryMsg::AndrQuery(msg) => handle_andromeda_query(deps, env, msg),
    // }
    Ok(to_binary(&true)?)
}

// fn handle_andromeda_query(
//     deps: Deps,
//     env: Env,
//     msg: AndromedaQuery,
// ) -> Result<Binary, ContractError> {
//     match msg {
//         AndromedaQuery::Get(data) => {
//         }
//         _ => ADOContract::default().query(deps, env, msg, query),
//     }
// }
