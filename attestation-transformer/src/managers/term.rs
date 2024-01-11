use proto_buf::transformer::{TermBatch, TermObject};
use rocksdb::{Options, WriteBatch, DB};

use crate::{error::AttTrError, term::Term};

struct TermManager {
	db: DB,
}

impl TermManager {
	pub fn new(db: &str) -> Result<Self, AttTrError> {
		let opts = Options::default();
		let db = DB::open_cf(&opts, &db, vec!["term"]).map_err(|e| AttTrError::DbError(e))?;
		Ok(Self { db })
	}
	pub fn read_terms(&self, batch: TermBatch) -> Result<Vec<TermObject>, AttTrError> {
		let mut terms = Vec::new();
		for i in batch.start..batch.size {
			let id_bytes = i.to_be_bytes();
			let res_opt = self.db.get(id_bytes).map_err(|e| AttTrError::DbError(e))?;
			if let Some(res) = res_opt {
				if let Ok(term) = Term::from_bytes(res) {
					let term_obj: TermObject = term.into();
					terms.push(term_obj);
				}
			}
		}
		Ok(terms)
	}
	pub fn write_terms(&self, terms: Vec<(u32, Term)>) -> Result<(), AttTrError> {
		let mut batch = WriteBatch::default();
		for (id, term) in terms {
			let term_bytes = term.into_bytes()?;
			let id = id.to_be_bytes();
			batch.put(id, term_bytes);
		}
		self.db.write(batch).map_err(|e| AttTrError::DbError(e))
	}
}
