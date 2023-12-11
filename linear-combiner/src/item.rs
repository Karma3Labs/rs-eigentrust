use proto_buf::combiner::LtObject;

struct LtItem {
	x: u32,
	y: u32,
	value: u32,
}

impl LtItem {
	pub fn new(x: u32, y: u32, value: u32) -> Self {
		LtItem { x, y, value }
	}
}

impl Into<LtObject> for LtItem {
	fn into(self) -> LtObject {
		LtObject { x: self.x, y: self.y, value: self.value }
	}
}
