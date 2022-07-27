use std::{
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

const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const PEER_ADDRS_CACHE_SIZE: usize = 100;

// #[derive(Debug)]
// pub struct PersistPeerAddrs {
// 	paths: Arc<Paths>,
// 	flush_valve: FlushValve,
// 	entries: LruCache<PeerId, Vec<Multiaddr>>,
// }

// impl PersistPeerAddrs {
// 	pub fn new(dir: impl AsRef<Path>) -> Self {
// 		let paths = Paths::new(dir, "peer-sets");
// 		Self {
// 			paths: Arc::new(paths),
// 			flush_valve: FlushValve::new(MIN_FLUSH_INTERVAL),
// 			entries: LruCache::new(PEER_ADDRS_CACHE_SIZE),
// 		}
// 	}

// 	pub fn report_peer_addrs(&mut self, peer_id: &PeerId, addrs: &[Multiaddr]) {
// 		let addrs = addrs.into_iter().cloned().collect::<Vec<_>>();
// 		self.entries.push(peer_id.to_owned(), addrs);
// 		self.flush_valve.report_change();
// 	}
// }

pub struct PersistPeersets(BoxedFuture<Never>);

impl PersistPeersets {
	pub fn new(dir: impl AsRef<Path>, peerset_handle: PeersetHandle) -> Self {
		let paths = Paths::new(dir, "peer-sets");
		let busy_future = async move {
			let mut ticks = tokio::time::interval(FLUSH_INTERVAL);
			ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			loop {
				let _ = ticks.tick().await;
				if let Err(reason) = persist_peersets(&paths, &peerset_handle).await {
					log::warn!("Error persisting peer sets: {}", reason);
				}
			}
		};
		Self(busy_future.boxed())
	}

	pub fn poll(&mut self, cx: &mut Context) -> Poll<std::convert::Infallible> {
		self.0.poll_unpin(cx)
	}
}

impl fmt::Debug for PersistPeersets {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.debug_struct("PersistPeersets").finish()
	}
}

async fn persist_peersets(paths: &Paths, peerset_handle: &PeersetHandle) -> Result<(), io::Error> {
	Err(io::Error::new(io::ErrorKind::Other, "Not implemented (persist peersets)"))
}

pub fn load_peersets(dir: impl AsRef<Path>) -> Result<Vec<()>, io::Error> {
	let mut path = dir.as_ref().to_owned();
	path.push("peer-sets.json");

	match std::fs::OpenOptions::new().read(true).open(&path) {
		Ok(f) => {
			let peersets = serde_json::from_reader(f)?;
			Ok(peersets)
		},
		Err(not_found) if not_found.kind() == io::ErrorKind::NotFound => Ok(vec![]),
		Err(reason) => Err(reason),
	}
}

// #[derive(Debug)]
// struct FlushValve {
// 	min_flush_interval: Duration,
// 	last_flush: std::time::Instant,
// 	has_changes: bool,
// }

// impl FlushValve {
// 	pub fn new(min_flush_interval: Duration) -> Self {
// 		Self { min_flush_interval, last_flush: Instant::now(), has_changes: false }
// 	}
// 	pub fn report_change(&mut self) {
// 		self.has_changes = true;
// 	}
// 	pub fn should_flush(&self) -> bool {
// 		self.has_changes && self.last_flush.elapsed() > self.min_flush_interval
// 	}
// 	pub fn report_flushed(&mut self) {
// 		self.last_flush = Instant::now();
// 	}
// }

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
