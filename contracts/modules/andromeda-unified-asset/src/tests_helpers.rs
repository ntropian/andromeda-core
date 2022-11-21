use andromeda_modules::unified_asset::InstantiateMsg;
use cosmwasm_std::{BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response};

use crate::error::ContractError;

use crate::tests_contract::{LEGACY_OWNER, PERMISSIONED_ADDRESS};

pub fn get_test_instantiate_message(
    env: Env,
    starting_debt: Coin,
    obi_is_signer: bool,
) -> InstantiateMsg {
    let signer2: String = if obi_is_signer {
        "juno17w77rnps59cnallfskg42s3ntnlhrzu2mjkr3e".to_string()
    } else {
        "signer2".to_string()
    };
    // instantiate the contract

    InstantiateMsg {
        legacy_owner: Some(LEGACY_OWNER.to_string()),
        home_network: "local".to_string(),
    }
}
