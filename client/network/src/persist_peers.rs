use std::path::Path;
use std::path::PathBuf;

use crate::PeerId;

#[derive(Debug)]
pub enum PersistPeers {
	Disabled,
	Enabled(PersistPeersEnabled),
}

#[derive(Debug)]
pub struct PersistPeersEnabled {
    net_config_path: PathBuf,
}

impl PersistPeers {
	pub fn init(enabled: bool, net_config_path: Option<&Path>) -> Self {
        match (enabled, net_config_path) {
            (true, Some(net_config_path)) => PersistPeers::Enabled(PersistPeersEnabled {
                net_config_path: net_config_path.to_owned(),
            }),
            _ =>
		        Self::Disabled
        }
	}

	pub fn on_sync_peer_connected(&mut self, peer_id: &PeerId) {
		if let Self::Enabled(enabled) = self {
            enabled.on_sync_peer_connected(peer_id)
        }
	}

	pub fn on_sync_peer_disconnected(&mut self, peer_id: &PeerId) {
		if let Self::Enabled(enabled) = self {
            enabled.on_sync_peer_disconnected(peer_id)
        }
	}
}

impl PersistPeersEnabled {
    pub fn on_sync_peer_connected(&mut self, peer_id: &PeerId) {
		eprintln!("!!! sc-network::PersistPeersEnabled::on_sync_peer_connected: [peer_id: {:?}", peer_id);
	}

	pub fn on_sync_peer_disconnected(&mut self, peer_id: &PeerId) {
		eprintln!(
			"!!! sc-network::PersistPeersEnabled::on_sync_peer_disconnected: [peer_id: {:?}",
			peer_id
		);
	}
}
