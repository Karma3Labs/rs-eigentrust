#[derive(Clone)]
pub struct LMDBClientConfig {
    pub path: String,
    max_dbs: u32,
    map_size: u32,
}
