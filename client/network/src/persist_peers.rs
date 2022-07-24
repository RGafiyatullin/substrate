use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	task::{Context, Poll},
	time::{Duration, Instant},
};

use crate::PeerId;
use crate::Multiaddr;
use libp2p::core::connection::ConnectionId;

const INIT_COOLDOWN: Duration = Duration::from_secs(10);
const DUMP_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum PersistPeers {
	Disabled,
	Enabled(PersistPeersEnabled),
}

#[derive(Debug)]
pub struct PersistPeersEnabled {
	net_config_path: PathBuf,
	time: Time,

	connected_peers: HashMap<PeerId, HashSet<ConnectionId>>,
	// recently_disconnected_peers: HashMap<PeerId, Instant>,
}

#[derive(Debug, Clone, Copy)]
enum Time {
	CreatedAt(Instant),
	LastSavedAt(Instant),
}

impl PersistPeers {
	pub fn init(enabled: bool, net_config_path: Option<&Path>) -> Self {
		match (enabled, net_config_path) {
			(true, Some(net_config_path)) => PersistPeers::Enabled(PersistPeersEnabled {
				net_config_path: net_config_path.to_owned(),
				time: Time::CreatedAt(Instant::now()),

				connected_peers: Default::default(),
				// recently_disconnected_peers: Default::default(),
			}),
			_ => Self::Disabled,
		}
	}

	pub fn on_connected(&mut self, conn_id: ConnectionId, peer_id: &PeerId, address: &Multiaddr) {
		if let Self::Enabled(enabled) = self {
			enabled.on_connected(conn_id, peer_id, address)
		}
	}

	pub fn on_disconnected(&mut self, conn_id: ConnectionId, peer_id: &PeerId) {
		if let Self::Enabled(enabled) = self {
			enabled.on_disconnected(conn_id, peer_id)
		}
	}

	pub fn poll(&mut self, cx: &mut Context) -> Poll<()> {
		if let Self::Enabled(enabled) = self {
			enabled.poll(cx)
		} else {
			Poll::Ready(())
		}
	}
}

impl PersistPeersEnabled {
	pub fn on_connected(&mut self, conn_id: ConnectionId, peer_id: &PeerId, address: &Multiaddr) {
		eprintln!(
			"!!! sc-network::PersistPeersEnabled::on_connected: [conn-id: {:?}, peer_id: {:?}, address: {:?}]",
			conn_id, peer_id, address,
		);

		let conn_ids = self.connected_peers.entry(peer_id.to_owned()).or_default();
		assert!(conn_ids.insert(conn_id), "Duplicate on_connected for the same pair of (peer-id: {:?}, conn-id: {:?})", peer_id, conn_id);
	}

	pub fn on_disconnected(&mut self, conn_id: ConnectionId, peer_id: &PeerId) {
		eprintln!(
			"!!! sc-network::PersistPeersEnabled::on_disconnected: [peer_id: {:?}",
			peer_id
		);

		if let Some(conn_ids) = self.connected_peers.get_mut(peer_id) {
			conn_ids.remove(&conn_id);
			if conn_ids.is_empty() {
				self.connected_peers.remove(peer_id);
			}
		} else {
			log::warn!(
				target: "persist-peers",
				"a stray sync-peer-disconnected event [peer-id: {:?}]",
				peer_id,
			)
		}
	}

	pub fn poll(&mut self, _cx: &mut Context) -> Poll<()> {
		let should_dump = match self.time {
			Time::CreatedAt(at) => at.elapsed() > INIT_COOLDOWN,
			Time::LastSavedAt(at) => at.elapsed() > DUMP_INTERVAL,
		};

		if should_dump {
			eprintln!("!!! sc-network::PersistPeersEnabled: DUMPING...");
			eprintln!("!!! connected_peers: {:?}", self.connected_peers);

			for (peer_id, conn_ids) in &self.connected_peers {
				eprintln!("!!!  - peer-id: {:?}, conn-ids: {:?}", peer_id, conn_ids);
			}
			
			self.time = Time::LastSavedAt(Instant::now());
		}

		Poll::Ready(())
	}
}
