use common::app::AndrAddress;

use cw_storage_plus::Item;

// The target ADO we want to querry
pub const TARGET_ADO_ADDRESS: Item<AndrAddress> = Item::new("target_ado_address");

// Query message of the target ADO, converted into binary and supplied by the frontend

pub const QUERY_MSG: Item<String> = Item::new("query_message");
