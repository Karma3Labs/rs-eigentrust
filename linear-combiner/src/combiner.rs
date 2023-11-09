struct LocalTrustItem {
	x: u32,
	y: u32,
	value: u32,
}

struct LinearCombiner {
	index_map: HashMap<String, u32>,
	matrix: HashMap<(u32, u32), LtItem>,
}
