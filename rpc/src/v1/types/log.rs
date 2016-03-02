// Copyright 2015, 2016 Ethcore (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use util::numbers::*;
use ethcore::log_entry::LocalizedLogEntry;
use v1::types::Bytes;

#[derive(Debug, Serialize)]
pub struct Log {
	address: Address,
	topics: Vec<H256>,
	data: Bytes,
	#[serde(rename="blockHash")]
	block_hash: H256,
	#[serde(rename="blockNumber")]
	block_number: U256,
	#[serde(rename="transactionHash")]
	transaction_hash: H256,
	#[serde(rename="transactionIndex")]
	transaction_index: U256,
	#[serde(rename="logIndex")]
	log_index: U256,
}

impl From<LocalizedLogEntry> for Log {
	fn from(e: LocalizedLogEntry) -> Log {
		Log {
			address: e.entry.address,
			topics: e.entry.topics,
			data: Bytes::new(e.entry.data),
			block_hash: e.block_hash,
			block_number: From::from(e.block_number),
			transaction_hash: e.transaction_hash,
			transaction_index: From::from(e.transaction_index),
			log_index: From::from(e.log_index)
		}
	}
}

#[cfg(test)]
mod tests {
	use serde_json;
	use std::str::FromStr;
	use util::numbers::*;
	use v1::types::{Bytes, Log};

	#[test]
	fn log_serialization() {
		let s = r#"{"address":"0x33990122638b9132ca29c723bdf037f1a891a70c","topics":["0xa6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc","0x4861736852656700000000000000000000000000000000000000000000000000"],"data":"0x","blockHash":"0xed76641c68a1c641aee09a94b3b471f4dc0316efe5ac19cf488e2674cf8d05b5","blockNumber":"0x04510c","transactionHash":"0x0000000000000000000000000000000000000000000000000000000000000000","transactionIndex":"0x00","logIndex":"0x01"}"#;

		let log = Log {
			address: Address::from_str("33990122638b9132ca29c723bdf037f1a891a70c").unwrap(),
			topics: vec![
				H256::from_str("a6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc").unwrap(),
				H256::from_str("4861736852656700000000000000000000000000000000000000000000000000").unwrap()
			],
			data: Bytes::new(vec![]),
			block_hash: H256::from_str("ed76641c68a1c641aee09a94b3b471f4dc0316efe5ac19cf488e2674cf8d05b5").unwrap(),
			block_number: U256::from(0x4510c),
			transaction_hash: H256::new(),
			transaction_index: U256::zero(),
			log_index: U256::one()
		};

		let serialized = serde_json::to_string(&log).unwrap();
		assert_eq!(serialized, s);
	}
}
