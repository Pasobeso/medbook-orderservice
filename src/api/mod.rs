pub mod deliveries;
pub mod products;

pub struct ApiUrls {
    pub delivery_service_url: String,
    pub inventory_service_url: String,
}

impl ApiUrls {
    pub fn init() -> Self {
        Self {
            delivery_service_url: Self::get_delivery_service_url(),
            inventory_service_url: Self::get_inventory_service_url(),
        }
    }

    pub fn get_delivery_service_url() -> String {
        std::env::var("DELIVERY_SERVICE_URL")
            .unwrap_or("http://localhost:3000/deliveries-service".to_string())
    }

    pub fn get_inventory_service_url() -> String {
        std::env::var("INVENTORY_SERVICE_URL")
            .unwrap_or("http://localhost:3000/inventory-service".to_string())
    }
}
