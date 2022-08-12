use crate::{ado_base::query_get, encode_binary, error::ContractError};
use cosmwasm_std::{Addr, Api, QuerierWrapper};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AndrAddress {
    /// Can be either an address or identifier of an ADO in a app.
    pub identifier: String,
}

impl AndrAddress {
    /// Gets the address from the given identifier by attempting to validate it into an [`Addr`] and
    /// then querying the app contract if it fails.
    pub fn get_address(
        &self,
        api: &dyn Api,
        querier: &QuerierWrapper,
        app_contract: Option<Addr>,
    ) -> Result<String, ContractError> {
        let addr = api.addr_validate(&self.identifier);
        match addr {
            Ok(addr) => Ok(addr.to_string()),
            Err(_) => match app_contract {
                Some(app_contract) => query_get::<String>(
                    Some(encode_binary(&self.identifier)?),
                    app_contract.to_string(),
                    querier,
                ),
                // TODO: Make error more descriptive.
                None => Err(ContractError::InvalidAddress {}),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock_querier::{mock_dependencies_custom, MOCK_APP_CONTRACT};
    use cosmwasm_std::testing::mock_dependencies;

    #[test]
    fn test_get_address_not_app() {
        let deps = mock_dependencies();
        let andr_address = AndrAddress {
            identifier: "address".to_string(),
        };
        assert_eq!(
            "address",
            andr_address
                .get_address(deps.as_ref().api, &deps.as_ref().querier, None)
                .unwrap()
        );
    }

    #[test]
    fn test_get_address_app() {
        let deps = mock_dependencies_custom(&[]);
        let andr_address = AndrAddress {
            identifier: "ab".to_string(),
        };
        assert_eq!(
            "actual_address",
            andr_address
                .get_address(
                    deps.as_ref().api,
                    &deps.as_ref().querier,
                    Some(Addr::unchecked(MOCK_APP_CONTRACT)),
                )
                .unwrap()
        );
    }
}
