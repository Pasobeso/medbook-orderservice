use anyhow::{Context, Result};
use medbook_core::app_error::{AppError, StdResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::ApiUrls;

#[derive(Serialize, Deserialize)]
struct DeliveryAddress {
    patient_id: i32,
}

pub async fn get_delivery_address_as_value(client: Client, id: i32) -> Result<Value> {
    let url = ApiUrls::get_delivery_service_url();
    let delivery_address: StdResponse<Value, String> = client
        .get(format!("{}/delivery-addresses/{}", url, id))
        .send()
        .await
        .map_err(|_| AppError::ServiceUnreachable("DeliveryService".into()))?
        .json()
        .await
        .context("Failed to parse JSON")?;

    match delivery_address.data {
        Some(delivery_address) => Ok(delivery_address),
        None => Err(anyhow::anyhow!("Delivery address not found")),
    }
}

pub async fn get_delivery_address_as_value_with_ownership_check(
    client: Client,
    id: i32,
    patient_id: i32,
) -> Result<Value> {
    let url = ApiUrls::get_delivery_service_url();
    let delivery_address: StdResponse<Value, String> = client
        .get(format!("{}/delivery-addresses/{}", url, id))
        .send()
        .await
        .map_err(|_| AppError::ServiceUnreachable("DeliveryService".into()))?
        .json()
        .await
        .context("Failed to parse JSON")?;

    match delivery_address.data {
        Some(delivery_address) => {
            let delivery_address_with_patient_id: DeliveryAddress =
                serde_json::from_value(delivery_address.clone())
                    .context("Failed to deserialize delivery address")?;

            if delivery_address_with_patient_id.patient_id != patient_id {
                return Err(anyhow::anyhow!("Delivery address patient IDs do not match"));
            }

            Ok(delivery_address)
        }
        None => Err(anyhow::anyhow!("Delivery address not found")),
    }
}
