generate abi

example
https://www.gakonst.com/ethers-rs/contracts/abigen.html

```
 const CONTRACT_ABI: &str = include_str!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/clique/clique_master_registry_abi.json")
        );
    
        Abigen::new("CLIQUE", CONTRACT_ABI)?.generate()?.write_to_file("bindings.rs")?;
```