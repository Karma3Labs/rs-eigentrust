// todo rename to clique specific
#[derive(Clone)]
pub struct EVMIndexerConfig {
    pub rpc_url: String,
    pub master_registry_contract: String,
    pub from_block: u64,
}