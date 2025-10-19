use std::collections::HashMap;

use anyhow::{Context, Result};
use medbook_core::app_error::AppError;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Product {
    pub id: i32,
    pub unit_price: f32,
}

pub async fn get_product_unit_prices(client: Client, ids: Vec<i32>) -> Result<HashMap<i32, f32>> {
    let ids_query = ids
        .into_iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let products: Vec<Product> = client
        .get("http://localhost:3000/products")
        .query(&[("ids", ids_query)])
        .send()
        .await
        .map_err(|_| AppError::ServiceUnreachable("InventoryService".into()))?
        .json()
        .await
        .context("Failed to parse JSON")?;

    let unit_prices: HashMap<i32, f32> =
        products.into_iter().map(|p| (p.id, p.unit_price)).collect();

    Ok(unit_prices)
}
