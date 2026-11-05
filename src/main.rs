mod delivery;
use delivery::{Delivery, DeliveryStatus, find_delivery, mark_failed, mark_succeeded};

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

    let deliveries = vec![successful_delivery, failed_delivery];

    println!("Looking for deliveries...");

    for event_name in ["order.canceled", "order.refunded"] {
        match find_delivery(&deliveries, event_name) {
            Some(delivery) => print_attempt(delivery),
            None => println!("cannot find event {event_name}"),
        };
    }
}

fn print_attempt(delivery: &Delivery) {
    println!(
        "Delivery {}, attempt {} ({})",
        delivery.event, delivery.attempts, delivery.status
    );
}
