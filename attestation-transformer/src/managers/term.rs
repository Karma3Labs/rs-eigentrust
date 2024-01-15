use itertools::Itertools;
use proto_buf::transformer::{TermBatch, TermObject};
use rocksdb::{Options, WriteBatch, DB};

use crate::{error::AttTrError, term::Term};

#[derive(Debug)]
pub struct TermManager {
	db: DB,
}

impl TermManager {
	pub fn new(db: &str) -> Result<Self, AttTrError> {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
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

	pub fn get_indexed_terms(start: u32, terms: Vec<Vec<Term>>) -> (u32, Vec<(u32, Term)>) {
		terms.iter().fold((start, Vec::new()), |(mut acc, mut new_items), items| {
			let indexed_items = items
				.into_iter()
				.map(|x| {
					let indexed_item = (acc, x.clone());
					acc += 1;
					indexed_item
				})
				.collect_vec();
			new_items.extend(indexed_items);
			(acc, new_items)
		})
	}

	pub fn drop(self) -> Result<(), AttTrError> {
		self.db.drop_cf("term").map_err(|e| AttTrError::DbError(e))?;
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use itertools::Itertools;
	use proto_buf::transformer::{TermBatch, TermObject};

	use crate::{managers::term::TermManager, schemas::Domain, term::Term};

	#[test]
	fn should_write_read_term() {
		let org_terms = vec![Term::new(
			"did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_string(),
			"did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c3".to_string(),
			25.,
			Domain::SoftwareSecurity.into(),
			true,
		)];
		let indexed_terms =
			org_terms.clone().into_iter().enumerate().map(|(i, x)| (i as u32, x)).collect_vec();

		let term_manager = TermManager::new("att-tr-terms-test-storage").unwrap();
		term_manager.write_terms(indexed_terms).unwrap();

		let term_batch = TermBatch { start: 0, size: 1 };
		let terms = term_manager.read_terms(term_batch).unwrap();

		let term_objs: Vec<TermObject> = org_terms.into_iter().map(|x| x.into()).collect_vec();
		assert_eq!(terms, term_objs);
	}
}
