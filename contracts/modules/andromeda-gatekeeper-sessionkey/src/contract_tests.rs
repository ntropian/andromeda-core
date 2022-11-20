#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{execute, instantiate, query};
    use ado_base::ADOContract;
    use andromeda_modules::gatekeeper_common::{
        InstantiateMsg, TestExecuteMsg, TestMsg, UniversalMsg,
    };
    use andromeda_modules::gatekeeper_sessionkey::{CanExecuteResponse, ExecuteMsg, QueryMsg};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{
        attr, from_binary, to_binary, CosmosMsg, DepsMut, MessageInfo, Response, Timestamp, WasmMsg,
    };

    fn init(deps: DepsMut, info: MessageInfo) {
        instantiate(
            deps,
            mock_env(),
            info,
            InstantiateMsg {
                legacy_owner: Some("creator".to_string()),
            },
        )
        .unwrap();
    }

    #[test]
    fn test_instantiate() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            legacy_owner: Some("creator".to_string()),
        };
        let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn test_delayed_transaction() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let operator = "creator";
        let info = mock_info(operator, &[]);

        init(deps.as_mut(), info.clone());

        ADOContract::default()
            .execute_update_operators(deps.as_mut(), info.clone(), vec![operator.to_owned()])
            .unwrap();

        // create a sessionkey for 60 seconds

        let msg = ExecuteMsg::CreateSessionKey {
            address: "firstsession".to_string(),
            max_duration: 60,
            admin_permissions: true,
        };

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(res.messages.len(), 0);
        assert_eq!(res.attributes.len(), 2);

        // can execute a message 6 seconds later
        let mut future_env = env.clone();
        future_env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 6u64);

        let query_msg = QueryMsg::CanExecute {
            sender: "firstsession".to_string(),
            message: UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "dummycontract".to_string(),
                msg: to_binary(&TestMsg::KesselRun(TestExecuteMsg {
                    parsecs: "14".to_string(),
                }))
                .unwrap(),
                funds: vec![],
            })),
        };
        let res: CanExecuteResponse =
            from_binary(&query(deps.as_ref(), future_env.clone(), query_msg.clone()).unwrap())
                .unwrap();
        assert!(res.can_execute);

        // but not 66 seconds later
        future_env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 66u64);
        // currently errors if expired, rather than returning false
        let _res = query(deps.as_ref(), future_env.clone(), query_msg.clone()).unwrap_err();

        // let's destroy the sessionkey, and check that we can't execute, not even 6 seconds later
        future_env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 6u64);
        let msg = ExecuteMsg::DestroySessionKey {
            address: "firstsession".to_string(),
        };
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // won't return false but will error as the sessionkey doesn't exist anymore
        let _res = query(deps.as_ref(), future_env.clone(), query_msg.clone()).unwrap_err();
    }
}
