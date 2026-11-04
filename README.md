# hookrun

A small Rust CLI that models event delivery status in memory. The demo starts two deliveries as pending, records one as succeeded and the other as failed, then shows how further status changes are rejected.

## Run the demo

With Rust and Cargo installed, run this from the directory containing `Cargo.toml`:

```sh
cargo run
```

The output shows each event name, attempt count, and status before and after the transition. After each rejection, the demo prints the delivery again so you can see that its status was preserved.

## Run the tests

```sh
cargo test
```

Four unit tests check that pending deliveries can succeed or fail and that rejected transitions preserve the original terminal status.
