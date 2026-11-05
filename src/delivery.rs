use std::fmt::Display;

pub(super) enum DeliveryStatus {
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

pub(super) struct Delivery {
    pub(super) event: String,
    pub(super) attempts: i32,
    pub(super) status: DeliveryStatus,
}

pub(super) fn find_delivery<'a>(
    deliveries: &'a [Delivery],
    event_name: &str,
) -> Option<&'a Delivery> {
    deliveries
        .iter()
        .find(|&delivery| delivery.event == event_name)
}

pub(super) fn mark_succeeded(delivery: &mut Delivery) -> Result<(), &'static str> {
    match delivery.status {
        DeliveryStatus::Pending => {
            delivery.status = DeliveryStatus::Succeeded;
            Ok(())
        }
        _ => Err("cannot change delivery status from terminal state"),
    }
}

pub(super) fn mark_failed(delivery: &mut Delivery) -> Result<(), &'static str> {
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
            event: String::from("order.created"),
            attempts: 1,
            status: DeliveryStatus::Succeeded,
        };

        let result = mark_failed(&mut delivery);

        assert!(result.is_err());
        assert!(matches!(delivery.status, DeliveryStatus::Succeeded));
    }
}
