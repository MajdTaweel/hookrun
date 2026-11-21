# hookrun

🦀 A little crab learning to deliver webhooks in Rust.

Hookrun currently provides an Axum HTTP API for creating, listing, and inspecting deliveries stored in PostgreSQL. Optional idempotency keys protect creation requests from duplicate submissions, including concurrent retries. Sending webhooks and running a delivery worker come next.

## Local setup

You need Rust and Cargo, a running PostgreSQL server, and `psql`. Run shell commands from the directory containing `Cargo.toml`.

Through a PostgreSQL administrator connection, create the local role and databases:

```sql
CREATE ROLE hookrun LOGIN PASSWORD '<choose-a-local-password>';
CREATE DATABASE hookrun_dev OWNER hookrun;
CREATE DATABASE hookrun_test OWNER hookrun;
ALTER ROLE hookrun CREATEDB;
```

Replace the password placeholder before running the SQL. `CREATEDB` lets SQLx create isolated test databases; it is a local test requirement, not an application runtime requirement.

Apply migrations to the fresh development database in order. These commands prompt for the role's password:

```sh
psql -h localhost -U hookrun -d hookrun_dev -v ON_ERROR_STOP=1 \
  -f migrations/0001_create_deliveries.sql
psql -h localhost -U hookrun -d hookrun_dev -v ON_ERROR_STOP=1 \
  -f migrations/0002_create_idempotency_keys.sql
```

Migrations are currently applied manually, once each per development database. If the first is already applied, run only the second. SQLx applies migrations automatically to isolated test databases.

Set the development URL, replacing the password placeholder and URL-encoding any special characters in it:

```sh
export DATABASE_URL='postgresql://hookrun:<url-encoded-password>@localhost:5432/hookrun_dev'
cargo run
```

Hookrun connects to PostgreSQL before listening on `127.0.0.1:3000`. Missing configuration or a failed initial connection prevents startup. Startup does not apply migrations or insert seed data. The original in-memory learning demo still prints before the server starts.

## Try the API

Check the service and create a delivery:

```sh
curl -i http://127.0.0.1:3000/health

curl -i http://127.0.0.1:3000/deliveries \
  -H 'Content-Type: application/json' \
  -d '{"event":"order.created"}'
```

Creation returns `201` with a generated ID, the event, `attempts: 0`, and `status: "pending"`. The health endpoint reports the service status; it is not a database readiness check.

List deliveries, then replace `42` with an ID returned by creation to inspect one:

```sh
curl -i http://127.0.0.1:3000/deliveries
curl -i http://127.0.0.1:3000/deliveries/42
```

A fresh database returns an empty list. A missing delivery returns JSON `404`. Nonnumeric IDs are rejected by Axum with `400`; IDs outside PostgreSQL's supported range return JSON `400`.

The learning endpoint `POST /echo` accepts an `event` JSON field and returns it without storing a delivery.

## Idempotent creation

Choose a key before the first request, then reuse it when retrying that same request:

```sh
curl -i http://127.0.0.1:3000/deliveries \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: create-order-123' \
  -d '{"event":"order.created"}'
```

Repeating this request returns the same delivery ID without storing another delivery. Both creation and replay currently return `201`. The delivery and its key mapping are committed in one transaction.

| Request | Behavior |
| --- | --- |
| Same key and same event | Return the existing delivery |
| Same key and different event | JSON `409 Conflict` |
| Blank or invalid key header | JSON `400 Bad Request` |
| No key header | Create a new delivery on each successful request |

Keys are currently global, with no caller or operation scope and no expiration. Matching compares the event string; there is no webhook payload yet. A replay reads the stored delivery rather than a saved copy of the original HTTP response. This protects delivery creation, not downstream webhook execution or exactly-once delivery.

Blank events return JSON `400`. Database and stored-data failures return generic JSON `500` without exposing internal details. Missing fields and malformed JSON are handled by Axum's extractors and may have plain-text error bodies.

## Tests and checks

Use the test connection database when running tests:

```sh
DATABASE_URL='postgresql://hookrun:<url-encoded-password>@localhost:5432/hookrun_test' \
  cargo test
```

SQLx creates a fresh database for each annotated router or database test and applies the migrations. Tests insert their own fixtures. Successful test databases are cleaned up automatically; failed tests may leave databases available for debugging. The role needs `CREATEDB`. Tests do not require resetting development data.

Coverage includes domain transitions and conversions, HTTP validation and errors, PostgreSQL reads and writes, transactional rollback, idempotent replay and conflicts, and concurrent creation.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The application currently stores delivery records only. Webhook destinations, payloads, execution, retries, and deployment are still ahead in the learning project.
