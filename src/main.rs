enum DeliveryStatus {
    Pending,
    Succeeded,
    Failed,
}

struct Delivery {
    event: String,
    attempts: i32,
    status: DeliveryStatus,
}

fn main() {
    let mut successful_delivery = Delivery {
        event: String::from("order.created"),
        attempts: 2,
        status: DeliveryStatus::Pending,
    };

    print_attempt(&successful_delivery);
    successful_delivery.attempts += 1;

    match mark_succeeded(&mut successful_delivery) {
        Ok(_) => print_attempt(&successful_delivery),
        Err(error) => println!("{error}"),
    };

    match mark_succeeded(&mut successful_delivery) {
        Ok(_) => print_attempt(&successful_delivery),
        Err(error) => println!("{error}"),
    };

    print_attempt(&successful_delivery);

    let mut failed_delivery = Delivery {
        event: String::from("order.canceled"),
        attempts: 1,
        status: DeliveryStatus::Pending,
    };

    print_attempt(&failed_delivery);
    failed_delivery.attempts += 1;

    match mark_failed(&mut failed_delivery) {
        Ok(_) => print_attempt(&failed_delivery),
        Err(error) => println!("{error}"),
    };

    match mark_succeeded(&mut failed_delivery) {
        Ok(_) => print_attempt(&failed_delivery),
        Err(error) => println!("{error}"),
    };

    print_attempt(&failed_delivery);
}

fn mark_succeeded(delivery: &mut Delivery) -> Result<(), &'static str> {
    match delivery.status {
        DeliveryStatus::Pending => {
            delivery.status = DeliveryStatus::Succeeded;
            Ok(())
        }
        _ => Err("cannot change delivery status from terminal state"),
    }
}

fn mark_failed(delivery: &mut Delivery) -> Result<(), &'static str> {
    match delivery.status {
        DeliveryStatus::Pending => {
            delivery.status = DeliveryStatus::Failed;
            Ok(())
        }
        _ => Err("cannot change delivery status from terminal state"),
    }
}

fn print_attempt(delivery: &Delivery) {
    let status = match delivery.status {
        DeliveryStatus::Pending => "pending",
        DeliveryStatus::Succeeded => "succeeded",
        DeliveryStatus::Failed => "failed",
    };

    println!(
        "Delivery {}, attempt {} ({status})",
        delivery.event, delivery.attempts
    );
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
