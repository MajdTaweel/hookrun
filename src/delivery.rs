use std::fmt::Display;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Succeeded,
    Failed,
}

impl Display for DeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status_str = match self {
            DeliveryStatus::Pending => "pending",
            DeliveryStatus::Succeeded => "succeeded",
            DeliveryStatus::Failed => "failed",
        };

        f.write_str(status_str)
    }
}

impl std::str::FromStr for DeliveryStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(DeliveryStatus::Pending),
            "succeeded" => Ok(DeliveryStatus::Succeeded),
            "failed" => Ok(DeliveryStatus::Failed),
            unknown => Err(format!("unknown delivery status: {unknown}")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Delivery {
    pub id: u64,
    pub event: String,
    pub attempts: i32,
    pub status: DeliveryStatus,
}

pub fn find_delivery<'a>(deliveries: &'a [Delivery], event_name: &str) -> Option<&'a Delivery> {
    deliveries
        .iter()
        .find(|delivery| delivery.event == event_name)
}

pub fn mark_succeeded(delivery: &mut Delivery) -> Result<(), &'static str> {
    match delivery.status {
        DeliveryStatus::Pending => {
            delivery.status = DeliveryStatus::Succeeded;
            Ok(())
        }
        _ => Err("cannot change delivery status from terminal state"),
    }
}

pub fn mark_failed(delivery: &mut Delivery) -> Result<(), &'static str> {
    match delivery.status {
        DeliveryStatus::Pending => {
            delivery.status = DeliveryStatus::Failed;
            Ok(())
        }
        _ => Err("cannot change delivery status from terminal state"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_delivery_can_succeed() {
        let mut delivery = Delivery {
            id: 1,
            event: String::from("order.created"),
            attempts: 1,
            status: DeliveryStatus::Pending,
        };

        let result = mark_succeeded(&mut delivery);

        assert!(result.is_ok());
        assert!(matches!(delivery.status, DeliveryStatus::Succeeded));
    }

    #[test]
    fn failed_delivery_cannot_succeed() {
        let mut delivery = Delivery {
            id: 1,
            event: String::from("order.created"),
            attempts: 1,
            status: DeliveryStatus::Failed,
        };

        let result = mark_succeeded(&mut delivery);

        assert!(result.is_err());
        assert!(matches!(delivery.status, DeliveryStatus::Failed));
    }

    #[test]
    fn pending_delivery_can_fail() {
        let mut delivery = Delivery {
            id: 1,
            event: String::from("order.created"),
            attempts: 1,
            status: DeliveryStatus::Pending,
        };

        let result = mark_failed(&mut delivery);

        assert!(result.is_ok());
        assert!(matches!(delivery.status, DeliveryStatus::Failed));
    }

    #[test]
    fn succeeded_delivery_cannot_fail() {
        let mut delivery = Delivery {
            id: 1,
            event: String::from("order.created"),
            attempts: 1,
            status: DeliveryStatus::Succeeded,
        };

        let result = mark_failed(&mut delivery);

        assert!(result.is_err());
        assert!(matches!(delivery.status, DeliveryStatus::Succeeded));
    }

    #[test]
    fn delivery_status_parses_known_values_and_rejects_unknown_values() {
        let pending: DeliveryStatus = "pending".parse().unwrap();
        assert!(matches!(pending, DeliveryStatus::Pending));

        let succeeded: DeliveryStatus = "succeeded".parse().unwrap();
        assert!(matches!(succeeded, DeliveryStatus::Succeeded));

        let failed: DeliveryStatus = "failed".parse().unwrap();
        assert!(matches!(failed, DeliveryStatus::Failed));

        let queued: Result<DeliveryStatus, String> = "queued".parse();
        assert_eq!(queued.unwrap_err(), "unknown delivery status: queued");
    }
}
