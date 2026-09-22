use crate::actor::{model::Actor, repository::create_human_actor};
use crate::customer::{model::Customer, repository::create_customer};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProvisionRegisteredCustomerResult {
    pub actor: Actor,
    pub customer: Customer,
}

pub async fn provision_registered_customer(
    pool: &PgPool,
    auth_subject: &str,
    display_name: &str,
    email: &str,
    phone: Option<&str>,
    first_name: &str,
    last_name: &str,
) -> Result<ProvisionRegisteredCustomerResult, AppError> {
    let mut tx = pool.begin().await?;

    let actor = create_human_actor(&mut tx, auth_subject, display_name).await?;

    let customer = create_customer(&mut tx, actor.id, email, phone, first_name, last_name).await?;

    tx.commit().await?;

    Ok(ProvisionRegisteredCustomerResult { actor, customer })
}
