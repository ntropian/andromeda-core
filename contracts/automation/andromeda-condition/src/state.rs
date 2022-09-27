use andromeda_automation::condition::{EvalDetails, LogicGate};
use common::app::AndrAddress;
use cw_storage_plus::Item;

// Logic gate setting
pub const LOGIC_GATE: Item<LogicGate> = Item::new("logic_gate");

// Results from evalutation ADOs
pub const RESULTS: Item<Vec<bool>> = Item::new("results_from_evaluation_ado");

// List of contracts you want to query results from
pub const WHITELIST: Item<Vec<EvalDetails>> = Item::new("whitelist");

// Execute ADO's address
pub const EXECUTE_ADO: Item<AndrAddress> = Item::new("execute_ado");
