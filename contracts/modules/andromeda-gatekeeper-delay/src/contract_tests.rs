#[cfg(test)]
mod tests {
    
    use crate::contract::{execute, instantiate};
    
    use ado_base::ADOContract;
    use andromeda_modules::gatekeeper_common::InstantiateMsg;
    use andromeda_modules::gatekeeper_delay::ExecuteMsg;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{
        to_binary, CosmosMsg, DepsMut, MessageInfo, Timestamp, WasmMsg,
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

        let _address = "whitelistee";

        init(deps.as_mut(), info.clone());

        ADOContract::default()
            .execute_update_operators(deps.as_mut(), info.clone(), vec![operator.to_owned()])
            .unwrap();

        // create a simple transaction

        let msg = ExecuteMsg::BeginTransaction {
            message: CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "dummycontract".to_string(),
                msg: to_binary(&ExecuteMsg::BeginTransaction {
                    message: CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: "dummycontract".to_string(),
                        msg: to_binary(&ExecuteMsg::CancelTransaction { txnumber: 1 }).unwrap(),
                        funds: vec![],
                    }),
                    delay_seconds: 3600,
                })
                .unwrap(),
                funds: vec![],
            }),
            delay_seconds: 10,
        };

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(res.messages.len(), 0);
        assert_eq!(res.attributes.len(), 3);

        // message is not executable 6 seconds later
        let mut future_env = env.clone();
        future_env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 6u64);

        let msg = ExecuteMsg::CompleteTransaction { txnumber: 1u64 };
        let _res =
            execute(deps.as_mut(), future_env.clone(), info.clone(), msg.clone()).unwrap_err();

        // but is executable 10 seconds later
        future_env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 10u64);
        let _res = execute(deps.as_mut(), future_env.clone(), info.clone(), msg.clone()).unwrap();

        // and now no longer exists
        future_env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 16u64);
        let _res = execute(deps.as_mut(), future_env, info, msg).unwrap_err();
    }
}
