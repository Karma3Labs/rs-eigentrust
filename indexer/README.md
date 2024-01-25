* generate a restorable unique seq id for source data
* basic verification
* healthcheck / monitoring
* error handling

```bash
cargo build && cargo run
```

2 folders will be created in indexer/
*db* - key value db to store indexers state
*cache* - cache indexing results in csv files