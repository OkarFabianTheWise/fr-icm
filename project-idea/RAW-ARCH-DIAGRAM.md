## 🧱 Fiatrouter System Architecture (High-Level Sketch)

```plaintext
                          +------------------------------+
                          |      User Interface (Web)    |
                          |  - Widget-Based UI           |
                          |  - Portfolio Selection       |
                          |  - Leaderboard + Feed        |
                          +-------------+----------------+
                                        |
                      +----------------v----------------+
                      |      API Gateway (Rust Axum)    |
                      |  Auth | Request Routing | Logs   |
                      +--------+---------+--------------+
                               |         |
           +------------------+         +-------------------------+
           |                                            |
+----------v----------+                      +----------v----------+
|   Swap Engine       |                      |  Portfolio Tracker   |
|  - Naira <-> USDC   |                      |  - PnL calc engine   |
|  - USDC <-> ETFs    |                      |  - Position state    |
|  - Fee extraction   |                      |  - Strategy metadata |
+----------+----------+                      +----------+-----------+
           |                                            |
           |                                            |
+----------v----------+                      +----------v-----------+
|    On/Off Ramp      |                      |   Agent Executor     |
| - Integrated APIs   |                      | - Rust async workers |
| - Manual fallback   |                      | - Executes logic     |
+---------------------+                      | - Runs in intervals  |
                                             +----------+-----------+
                                                        |
                                       +----------------v----------------+
                                       |   Asset Routers / Data Feeds    |
                                       | - Price Feeds (Oracles, APIs)   |
                                       | - Market Depth                  |
                                       | - Asset Allocation updates      |
                                       +----------------------------------+

                🧾 DB Layer: Postgres or ScyllaDB (for event + state tracking)
                - Portfolios, User History, Swap Logs, Agent States
```

---


## 🧮 Data Flow Summary

```plaintext
User Action (UI) -->
    API Gateway -->
        Swap / Portfolio / Agent Trigger -->
            Agent fetches prices, evaluates portfolio -->
                If rebalance needed -->
                    Executes swap via swap engine -->
                        Logs transaction + fees -->
                            Updates leaderboard & user PnL
```
