use itertools::Itertools;
use rocksdb::{WriteBatch, DB};

use proto_buf::transformer::{TermBatch, TermObject};

use crate::error::AttTrError;
use crate::term::Term;

#[derive(Debug)]
pub struct TermManager;

impl TermManager {
	pub fn read_terms(db: &DB, batch: TermBatch) -> Result<Vec<TermObject>, AttTrError> {
		let cf = db.cf_handle("term").ok_or_else(|| AttTrError::NotFoundError)?;

		let mut terms = Vec::new();
		for i in batch.start..batch.start + batch.size {
			let id_bytes = i.to_be_bytes();
			let res_opt = db.get_cf(&cf, id_bytes).map_err(AttTrError::DbError)?;
			if let Some(res) = res_opt {
				let term = Term::from_bytes(res)?;
				let term_obj: TermObject = term.into();
				terms.push(term_obj);
			}
		}
		Ok(terms)
	}

	pub fn write_terms(db: &DB, terms: Vec<(u32, Term)>) -> Result<(), AttTrError> {
		let cf = db.cf_handle("term").ok_or_else(|| AttTrError::NotFoundError)?;

		let mut batch = WriteBatch::default();
		for (id, term) in terms {
			let term_bytes = term.into_bytes()?;
			let id = id.to_be_bytes();
			batch.put_cf(&cf, id, term_bytes);
		}
		db.write(batch).map_err(AttTrError::DbError)
	}

	pub fn get_indexed_terms(
		start: u32, terms: Vec<Vec<Term>>,
	) -> Result<(u32, Vec<(u32, Term)>), AttTrError> {
		let new_items =
			terms.iter().fold((start, Vec::new()), |(mut acc, mut new_items), items| {
				let indexed_items = items
					.iter()
					.map(|x| {
						let indexed_item = (acc, x.clone());
						acc += 1;
						indexed_item
					})
					.collect_vec();
				new_items.extend(indexed_items);
				(acc, new_items)
			});

		Ok(new_items)
	}
}

#[cfg(test)]
mod test {
	use itertools::Itertools;
	use rocksdb::{Options, DB};

	use proto_buf::transformer::{TermBatch, TermObject};

	use crate::schemas::Domain;
	use crate::term::{Term, TermForm};

	use super::*;

	#[test]
	fn should_write_read_term() {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		opts.create_if_missing(true);
		let db = DB::open_cf(&opts, "att-wrt-test-storage", vec!["term"]).unwrap();

		let org_terms = vec![Term::new(
			"did:pkh:eth:0x90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_string(),
			"did:pkh:eth:0x90f8bf6a479f320ead074411a4b0e7944ea8c9c3".to_string(),
			25.,
			Domain::SoftwareSecurity.into(),
			TermForm::Trust,
			0,
		)];
		let indexed_terms =
			org_terms.clone().into_iter().enumerate().map(|(i, x)| (i as u32, x)).collect_vec();

		TermManager::write_terms(&db, indexed_terms).unwrap();

		let term_batch = TermBatch { start: 0, size: 1 };
		let terms = TermManager::read_terms(&db, term_batch).unwrap();

		let term_objs: Vec<TermObject> = org_terms.into_iter().map(|x| x.into()).collect_vec();
		assert_eq!(terms, term_objs);
	}
}
