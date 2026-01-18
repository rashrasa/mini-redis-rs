# mini-redis-rs

![mini redis gif](https://github.com/user-attachments/assets/4b2c2a77-94b4-4bbc-a768-ff1e93c74b54)

*Demo: Sending requests to server over TCP. Read data -> replaced field with new data -> re-opened server -> old data is still there.*

Hosted in-memory key-value store accepting requests through TCP.

Uses serde_json for request serialization, tokio for async IO.

**Completed**:

- Accepts TCP connections and spawns a tokio task for each
- Handles requests (serde_json utf-8 format) and sends back responses (Insert, Read, Delete requests)
- Persists values in a json file (only syncs when closing the server, for now)
- Toy stress test

**To Be Completed**:

- Currently relying on shared memory and tokio locks, causing performance overhead. Also, the server can be easily overwhelmed. The goal is to switch to using channels
- Brittle sync policy, any crash means all of the unsaved data is lost
- Many .unwrap() calls in places they do not need to be (Error handling)
- Commit ids and timestamps for performance tests, flamegraphs, charts, etc., for comparing performance across implementations (Basically, implementing a benchmark suite)

## Additional

### Stress Test

Located at `./mini-redis-server-rs/tests/integration_test.rs`.

1. Creates N connections and sends requests at a constant rate
2. Waits until the response rate stabilizes, according to an evaluation time frame (e.g., checks performance for the last 5 seconds, every 5 seconds)
3. If the server is able to handle requests at the rate they are being sent, the request rate is doubled and the process repeats. If not, the average performance across "evaluation windows" is output to stdout.
