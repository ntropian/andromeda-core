use std::env;

use crate::state::COUNT;
use ado_base::state::ADOContract;
use andromeda_automation::counter::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use common::{
    ado_base::InstantiateMsg as BaseInstantiateMsg, encode_binary, error::ContractError, require,
};
use cosmwasm_std::{
    entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError, Uint128,
};
use cw2::{get_contract_version, set_contract_version};
use semver::Version;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-counter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    COUNT.save(deps.storage, &Uint128::zero())?;

    ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "counter".to_string(),
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
    let contract = ADOContract::default();
    match msg {
        ExecuteMsg::AndrReceive(msg) => contract.execute(deps, env, info, msg, execute),
        ExecuteMsg::Increment {} => execute_increment(deps, env, info),
    }
}

fn execute_increment(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut count = COUNT.load(deps.storage)?;
    count += Uint128::new(1);
    COUNT.save(deps.storage, &count)?;
    Ok(Response::new()
        .add_attribute("action", "increment_count")
        .add_attribute("new_count", count))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // New version
    let version: Version = CONTRACT_VERSION.parse().map_err(from_semver)?;

    // Old version
    let stored = get_contract_version(deps.storage)?;
    let storage_version: Version = stored.version.parse().map_err(from_semver)?;

    let contract = ADOContract::default();

    require(
        stored.contract == CONTRACT_NAME,
        ContractError::CannotMigrate {
            previous_contract: stored.contract,
        },
    )?;

    // New version has to be newer/greater than the old version
    require(
        storage_version < version,
        ContractError::CannotMigrate {
            previous_contract: stored.version,
        },
    )?;

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
        QueryMsg::AndrQuery(msg) => ADOContract::default().query(deps, env, msg, query),
        QueryMsg::Count {} => encode_binary(&query_count(deps)?),
    }
}

fn query_count(deps: Deps) -> Result<Uint128, ContractError> {
    let count = COUNT.load(deps.storage)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

    #[test]
    fn test_initialization() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // make sure address was saved correctly
        let count = COUNT.load(&deps.storage).unwrap();
        assert_eq!(count, Uint128::zero())
    }

    #[test]
    fn test_increment() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // make sure address was saved correctly
        let count = COUNT.load(&deps.storage).unwrap();
        assert_eq!(count, Uint128::zero());

        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let expected_count = Uint128::new(1);
        let count = COUNT.load(&deps.storage).unwrap();
        assert_eq!(count, expected_count)
    }
}