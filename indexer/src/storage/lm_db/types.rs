#[derive(Clone)]
pub struct LMDBClientConfig {
    pub path: String,
    pub max_dbs: u32,
    pub map_size: u32,
}
