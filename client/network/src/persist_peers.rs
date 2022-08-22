// This file is part of Substrate.

// Copyright (C) 2017-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::{
	collections::{HashMap, HashSet},
	fmt,
	future::Future,
	io,
	path::{Path, PathBuf},
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
	time::{Duration, Instant},
};

use futures::FutureExt;
use lru::LruCache;

use sc_peerset::PeersetHandle;

use crate::{Multiaddr, PeerId};

type BoxedFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
type Never = std::convert::Infallible;
type ProtocolType = String;

const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const PEER_ADDRS_CACHE_SIZE: usize = 100;

pub struct PersistPeerAddrs {
	paths: Arc<Paths>,
	flushed_at: Instant,
	protocols: HashMap<ProtocolType, LruCache<PeerId, HashSet<Multiaddr>>>,
	busy: Option<BoxedFuture<Result<(), io::Error>>>,
}

impl PersistPeerAddrs {
	pub fn load(dir: impl AsRef<Path>) -> Self {
		let paths = Paths::new(dir, "peer-addrs");

		let protocols = match persist_peer_addrs::load(&paths.path) {
			Ok(restored) => restored,
			Err(reason) => {
				log::warn!("Failed to load peer addresses: {:?}", reason);
				Default::default()
			},
		};

		let protocols = protocols
			.into_iter()
			.map(|(protocol, entries)| {
				let cache = entries.into_iter().rev().fold(
					LruCache::new(PEER_ADDRS_CACHE_SIZE),
					|mut acc, persist_peer_addrs::PeerEntry { peer_id, addrs }| {
						if let Ok(peer_id) = peer_id.parse() {
							acc.push(peer_id, addrs.into_iter().collect::<HashSet<_>>());
						}
						acc
					},
				);
				(protocol, cache)
			})
			.collect();

		Self { paths: Arc::new(paths), flushed_at: Instant::now(), protocols, busy: None }
	}

	pub fn report_peer_addr(
		&mut self,
		peer_id: &PeerId,
		protocol: impl AsRef<[u8]>,
		addr: &Multiaddr,
	) {
		let protocol = String::from_utf8(protocol.as_ref().to_owned()).expect(
			"According to the `crate::discovery::protocol_name_from_protocol_id` \
					and `<ProtocolId as AsRef<str>>` it's a correct UTF-8 string",
		);

		let entries = self
			.protocols
			.entry(protocol)
			.or_insert_with(|| LruCache::new(PEER_ADDRS_CACHE_SIZE));
		if let Some(peer_addrs) = entries.get_mut(peer_id) {
			peer_addrs.insert(addr.to_owned());
		} else {
			entries.push(peer_id.to_owned(), [addr.to_owned()].into_iter().collect());
		}
	}

	pub fn peer_addrs<'a>(
		&'a mut self,
		peer_id: &'a PeerId,
		protocols: impl IntoIterator<Item = impl AsRef<[u8]> + 'a>,
	) -> impl Iterator<Item = &'a Multiaddr> {
		let protocols = protocols.into_iter().collect::<Vec<_>>();

		self.protocols
			.iter_mut()
			.filter_map(move |(protocol, entries)| {
				if protocols.iter().any(|p| p.as_ref() == protocol.as_bytes()) {
					Some(entries)
				} else {
					None
				}
			})
			.flat_map(|entries| entries.get(peer_id).into_iter())
			.flat_map(IntoIterator::into_iter)
	}

	pub fn poll(&mut self, cx: &mut Context) -> Poll<Never> {
		if let Some(busy_future) = self.busy.as_mut() {
			if let Poll::Ready(result) = busy_future.poll_unpin(cx) {
				self.busy = None;
				self.flushed_at = Instant::now();

				if let Err(reason) = result {
					log::warn!("Failed to persist peer addresses: {}", reason);
				}
			}
		} else if self.flushed_at.elapsed() > FLUSH_INTERVAL {
			let entries = self
				.protocols
				.iter()
				.map(|(protocol, entries)| {
					let entries = entries
						.iter()
						.map(|(peer_id, addrs)| {
							let peer_id = peer_id.to_base58();
							let addrs = addrs.into_iter().cloned().collect();

							persist_peer_addrs::PeerEntry { peer_id, addrs }
						})
						.collect::<Vec<_>>();
					(protocol.to_owned(), entries)
				})
				.collect();

			let busy_future = persist_peer_addrs::persist(Arc::clone(&self.paths), entries).boxed();
			self.busy = Some(busy_future);
		}
		Poll::Pending
	}
}

mod persist_peer_addrs {
	use super::*;
	use tokio::io::AsyncWriteExt;

	#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	pub(super) struct PeerEntry {
		pub peer_id: String,
		pub addrs: Vec<Multiaddr>,
	}

	pub(super) async fn persist(
		paths: Arc<Paths>,
		protocols: HashMap<ProtocolType, Vec<PeerEntry>>,
	) -> Result<(), io::Error> {
		let mut tmp_file = tokio::fs::OpenOptions::new()
			.create(true)
			.write(true)
			.truncate(true)
			.open(&paths.tmp_path)
			.await?;
		let serialized = serde_json::to_vec_pretty(&protocols)?;

		tmp_file.write_all(&serialized).await?;
		tmp_file.flush().await?;
		std::mem::drop(tmp_file);

		tokio::fs::rename(&paths.tmp_path, &paths.path).await?;

		Ok(())
	}

	pub(super) fn load(
		path: impl AsRef<Path>,
	) -> Result<HashMap<ProtocolType, Vec<PeerEntry>>, io::Error> {
		let file = match std::fs::OpenOptions::new().read(true).open(path.as_ref()) {
			Ok(file) => file,
			Err(not_found) if not_found.kind() == std::io::ErrorKind::NotFound =>
				return Ok(Default::default()),
			Err(reason) => return Err(reason),
		};
		let entries = serde_json::from_reader(file)?;
		Ok(entries)
	}
}

pub struct PersistPeersets(BoxedFuture<Never>);
pub use peersets::load as peersets_load;

impl PersistPeersets {
	pub fn new(dir: impl AsRef<Path>, peerset_handle: PeersetHandle) -> Self {
		let paths = Paths::new(dir, "peer-sets");
		let busy_future = async move {
			let mut ticks = tokio::time::interval(FLUSH_INTERVAL);
			ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			loop {
				let _ = ticks.tick().await;
				if let Err(reason) = peersets::persist(&paths, &peerset_handle).await {
					log::warn!("Error persisting peer sets: {}", reason);
				}
			}
		};
		Self(busy_future.boxed())
	}

	pub fn poll(&mut self, cx: &mut Context) -> Poll<Never> {
		self.0.poll_unpin(cx)
	}
}

mod peersets {
	use super::*;

	#[derive(Debug, serde::Serialize, serde::Deserialize)]
	pub struct PeerInfo {
		pub peer_id: String,
		pub reputation: i32,
		pub sets: Vec<usize>,
	}

	pub(super) async fn persist(
		paths: &Paths,
		peerset_handle: &PeersetHandle,
	) -> Result<(), io::Error> {
		use tokio::io::AsyncWriteExt;

		let peersets_dumped = peerset_handle
			.dump_state()
			.await
			.map_err(|()| io::Error::new(io::ErrorKind::BrokenPipe, "oneshot channel failure"))?
			.into_iter()
			.map(|(peer_id, reputation, sets)| PeerInfo {
				peer_id: peer_id.to_base58(),
				reputation,
				sets,
			})
			.collect::<Vec<_>>();

		let mut tmp_file = tokio::fs::OpenOptions::new()
			.create(true)
			.write(true)
			.truncate(true)
			.open(&paths.tmp_path)
			.await?;
		let serialized = serde_json::to_vec_pretty(&peersets_dumped)?;
		tmp_file.write_all(&serialized).await?;
		tmp_file.flush().await?;
		std::mem::drop(tmp_file);

		tokio::fs::rename(&paths.tmp_path, &paths.path).await?;

		Ok(())
	}

	pub fn load(dir: impl AsRef<Path>) -> Result<Vec<(PeerId, i32, Vec<usize>)>, io::Error> {
		let path = dir.as_ref().join("peer-sets.json");

		match std::fs::OpenOptions::new().read(true).open(&path) {
			Ok(f) => {
				let peersets: Vec<PeerInfo> = serde_json::from_reader(f)?;

				Ok(peersets
					.into_iter()
					.filter_map(|peer_info| {
						if let Ok(peer_id) = peer_info.peer_id.parse::<PeerId>() {
							Some((peer_id, peer_info.reputation, peer_info.sets))
						} else {
							None
						}
					})
					.collect())
			},
			Err(not_found) if not_found.kind() == io::ErrorKind::NotFound => Ok(vec![]),
			Err(reason) => Err(reason),
		}
	}

	impl fmt::Debug for PersistPeersets {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			f.debug_struct("PersistPeersets").finish()
		}
	}
}

#[derive(Debug)]
struct Paths {
	path: PathBuf,
	tmp_path: PathBuf,
}

impl Paths {
	pub fn new(net_config_path: impl AsRef<Path>, name: impl AsRef<str>) -> Self {
		let mut p = net_config_path.as_ref().to_owned();
		p.push(name.as_ref());

		let path = p.with_extension("json");
		let tmp_path = p.with_extension("tmp");

		Self { path, tmp_path }
	}
}

#[test]
fn test_paths() {
	let p = Paths::new("/tmp", "test");
	assert_eq!(p.path.to_str(), Some("/tmp/test.json"));
	assert_eq!(p.tmp_path.to_str(), Some("/tmp/test.tmp"));
}
