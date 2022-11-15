use cosmwasm_std::{
    testing::{mock_dependencies, mock_env, mock_info},
    Addr,
};

use crate::contract::{execute, instantiate, query};

#[test]
pub fn test_start_sale() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let info = mock_info(owner.as_str(), &[]);
}
