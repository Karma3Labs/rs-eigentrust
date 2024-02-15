* generate a restorable unique seq id for source data
* basic verification
* healthcheck / monitoring
* error handling

```bash
cargo build && cargo run
```

2 folders will be created:
* *db* - key value db to store indexing state
* *cache* - results as csv files