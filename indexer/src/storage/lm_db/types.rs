#[derive(Clone, Debug)]
pub struct LMDBClientConfig {
    pub db_name: String,
    pub path: String,
    pub max_dbs: u32,
    pub map_size: usize,
}
