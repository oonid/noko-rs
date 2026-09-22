pub mod model;
pub mod repository;

pub use model::{CreateCustomerAddress, Customer, CustomerAddress};
pub use repository::{create_address, get_addresses, get_customer_by_actor_id, get_customer_by_id};
