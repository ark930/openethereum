// Copyright 2015-2020 Parity Technologies (UK) Ltd.
// This file is part of Open Ethereum.

// Open Ethereum is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Open Ethereum is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Open Ethereum.  If not, see <http://www.gnu.org/licenses/>.

//! Set of different helpers for client tests

extern crate tempfile;
extern crate trie_db as trie;
extern crate ethcore_db as db;

mod evm_test_client;

/// Re-export for tests only
pub use evm::CreateContractAddress;
/// Re-export for tests only
pub use trie::TrieSpec;
/// Re-export for tests only
pub use self::evm_test_client::{EvmTestClient, EvmTestError, TransactErr, TransactSuccess};

use std::path::Path;
use std::sync::Arc;
use std::{fs, io};

use ethcore_blockchain::{BlockChainDB, BlockChainDBHandler, Config as BlockChainConfig};
use blooms_db;
use ethereum_types::{H256, U256, Address};
use evm::Factory as EvmFactory;
use keccak_hash::keccak;
use kvdb::KeyValueDB;
use kvdb_rocksdb::{self, Database, DatabaseConfig};
use self::tempfile::TempDir;
use common_types::{
	chain_notify::ChainMessageType,
	transaction::{Action, Transaction, SignedTransaction},
	encoded,
	engines::ForkChoice,
	header::Header,
	view,
	views::BlockView,
	verification::Unverified,
};

use trie_vm_factories::Factories;
use spec::{Spec, self};
use account_state::*;
use state_db::StateDB;

use crate::db as rockdb;

struct TestBlockChainDB {
	_blooms_dir: TempDir,
	_trace_blooms_dir: TempDir,
	blooms: blooms_db::Database,
	trace_blooms: blooms_db::Database,
	key_value: Arc<dyn KeyValueDB>,
}

impl BlockChainDB for TestBlockChainDB {
	fn key_value(&self) -> &Arc<dyn KeyValueDB> {
		&self.key_value
	}

	fn blooms(&self) -> &blooms_db::Database {
		&self.blooms
	}

	fn trace_blooms(&self) -> &blooms_db::Database {
		&self.trace_blooms
	}
}

/// Creates new test instance of `BlockChainDB`
pub fn new_db() -> Arc<dyn BlockChainDB> {
	let blooms_dir = TempDir::new().unwrap();
	let trace_blooms_dir = TempDir::new().unwrap();

	let db = TestBlockChainDB {
		blooms: blooms_db::Database::open(blooms_dir.path()).unwrap(),
		trace_blooms: blooms_db::Database::open(trace_blooms_dir.path()).unwrap(),
		_blooms_dir: blooms_dir,
		_trace_blooms_dir: trace_blooms_dir,
		key_value: Arc::new(::kvdb_memorydb::create(db::NUM_COLUMNS))
	};

	Arc::new(db)
}

/// Creates a new temporary `BlockChainDB` on FS
pub fn new_temp_db(tempdir: &Path) -> Arc<dyn BlockChainDB> {
	let blooms_dir = TempDir::new().unwrap();
	let trace_blooms_dir = TempDir::new().unwrap();
	let key_value_dir = tempdir.join("key_value");

	let db_config = DatabaseConfig::with_columns(db::NUM_COLUMNS);
	let key_value_db = Database::open(&db_config, key_value_dir.to_str().unwrap()).unwrap();

	let db = TestBlockChainDB {
		blooms: blooms_db::Database::open(blooms_dir.path()).unwrap(),
		trace_blooms: blooms_db::Database::open(trace_blooms_dir.path()).unwrap(),
		_blooms_dir: blooms_dir,
		_trace_blooms_dir: trace_blooms_dir,
		key_value: Arc::new(key_value_db)
	};

	Arc::new(db)
}

/// Creates new instance of KeyValueDBHandler
pub fn restoration_db_handler(config: kvdb_rocksdb::DatabaseConfig) -> Box<dyn BlockChainDBHandler> {
	struct RestorationDBHandler {
		config: kvdb_rocksdb::DatabaseConfig,
	}

	struct RestorationDB {
		blooms: blooms_db::Database,
		trace_blooms: blooms_db::Database,
		key_value: Arc<dyn KeyValueDB>,
	}

	impl BlockChainDB for RestorationDB {
		fn key_value(&self) -> &Arc<dyn KeyValueDB> {
			&self.key_value
		}

		fn blooms(&self) -> &blooms_db::Database {
			&self.blooms
		}

		fn trace_blooms(&self) -> &blooms_db::Database {
			&self.trace_blooms
		}
	}

	impl BlockChainDBHandler for RestorationDBHandler {
		fn open(&self, db_path: &Path) -> io::Result<Arc<dyn BlockChainDB>> {
			let key_value = Arc::new(kvdb_rocksdb::Database::open(&self.config, &db_path.to_string_lossy())?);
			let blooms_path = db_path.join("blooms");
			let trace_blooms_path = db_path.join("trace_blooms");
			fs::create_dir_all(&blooms_path)?;
			fs::create_dir_all(&trace_blooms_path)?;
			let blooms = blooms_db::Database::open(blooms_path).unwrap();
			let trace_blooms = blooms_db::Database::open(trace_blooms_path).unwrap();
			let db = RestorationDB {
				blooms,
				trace_blooms,
				key_value,
			};
			Ok(Arc::new(db))
		}
	}

	Box::new(RestorationDBHandler { config })
}

/// Returns temp state
pub fn get_temp_state() -> State<::state_db::StateDB> {
	let journal_db = get_temp_state_db();
	State::new(journal_db, U256::from(0), Default::default())
}

/// Returns temp state using coresponding factory
pub fn get_temp_state_with_factory(factory: EvmFactory) -> State<::state_db::StateDB> {
	let journal_db = get_temp_state_db();
	let mut factories = Factories::default();
	factories.vm = factory.into();
	State::new(journal_db, U256::from(0), factories)
}

/// Returns temp state db
pub fn get_temp_state_db() -> StateDB {
	let db = new_db();
	let journal_db = ::journaldb::new(db.key_value().clone(), ::journaldb::Algorithm::EarlyMerge, db::COL_STATE);
	StateDB::new(journal_db, 5 * 1024 * 1024)
}


pub fn get_temp_state_rockdb(path: &Path) -> State<::state_db::StateDB> {
	let mut state_db = get_temp_state_db_rockdb(path);
	let account_start_nonce: U256 = U256::from(0);
	let factories: Factories = Default::default();
	// let mut hash_db = state_db.as_hash_db_mut();
	let mut LatestStateRootKey = keccak("evm:LatestStateRootKey");

	// let bytes = hex::decode("d7f8974fb5ac78d9ac099b9ad5018bedc2ce0a72dad1827a1709da30580f0544").expect("hex::decode failed");
	// let mut array = [0u8; 32];
	// let bytes = &bytes[..array.len()]; // panics if not enough data
	// array.copy_from_slice(bytes);
	// let mut root = H256::from(&array);
	let mut root = H256::zero();
	// println!("root ==> {}", root);
	// State::new(state_db, account_start_nonce, factories)
	match factories.trie.from_existing(state_db.clone().as_hash_db_mut(), &mut root.clone()) {
		Ok(_) => {
			State::from_existing(state_db, root, account_start_nonce, factories).expect("state not exist")
		},
		Err(_) => {
			// init trie and reset root to null
			let _ = factories.trie.create(state_db.clone().as_hash_db_mut(), &mut root.clone());
			State::new(state_db, account_start_nonce, factories)
		},
	}


	// match factories.trie.from_existing(hash_db, &mut LatestStateRootKey) {
	// 	Ok(_) => {
	// 		// println!("root ==> {}", LatestStateRootKey);
	// 		println!("root ==> contains");
	//
	// 		let mut state_db = get_temp_state_db_rockdb(path);
	//
	// 		let mut LatestStateRootKey = keccak("evm:LatestStateRootKey");
	//
	// 		State::from_existing(state_db, LatestStateRootKey, account_start_nonce, factories).expect("state not exist")
	// 	},
	// 	Err(_) => {
	// 		// println!("root ==> {}", LatestStateRootKey);
	// 		println!("root ==> not contains");
	// 		let mut state_db = get_temp_state_db_rockdb(path);
	// 		let mut hash_db = state_db.as_hash_db_mut();
	//
	// 		let mut LatestStateRootKey = keccak("evm:LatestStateRootKey");
	// 		factories.trie.create(hash_db, &mut LatestStateRootKey);
	//
	// 		State::new(state_db, account_start_nonce, factories)
	// 	},
	// }

	// if hash_db.contains(&LatestStateRootKey, hash_db::EMPTY_PREFIX) {
	// 	let bytes = hash_db.get(&LatestStateRootKey, hash_db::EMPTY_PREFIX).unwrap();
	// 	let mut array = [0u8; 32];
	// 	let bytes = &bytes[..array.len()]; // panics if not enough data
	// 	array.copy_from_slice(bytes);
	// 	let root = H256::from(&array);
	// 	println!("root ==> {}", root);
	// 	println!("root ==> contains");
	//
	// 	State::from_existing(state_db, root, account_start_nonce, factories).expect("state not exist")
	// } else {
	// 	let root = H256::zero();
	// 	println!("root ==> {}", root);
	// 	println!("root ==> not contains");
	//
	// 	hash_db.emplace(LatestStateRootKey, hash_db::EMPTY_PREFIX, root.as_ref().to_vec());
	// 	println!("exists? {:?}", hash_db.contains(&LatestStateRootKey, hash_db::EMPTY_PREFIX));
	// 	State::new(state_db, account_start_nonce, factories)
	// }
}

pub fn get_temp_state_db_rockdb(path: &Path) -> StateDB {
	let db = new_db_rockdb(path);
	let journal_db = ::journaldb::new(db.key_value().clone(), ::journaldb::Algorithm::Archive, ::ethcore_db::COL_STATE);
	StateDB::new(journal_db, 5 * 1024 * 1024)
}

pub fn new_db_rockdb(path: &Path) -> Arc<dyn BlockChainDB> {
	let client_config = Default::default();
	let restoration_db_handler = rockdb::restoration_db_handler(&path, &client_config);
	let db = restoration_db_handler.open(&path).expect(format!("Failed to open database").as_str());

	db
}