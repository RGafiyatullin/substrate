use std::{
	fmt,
	future::Future,
	io::Error as IoError,
	path::Path,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
	time::{Duration, Instant},
};

use futures::FutureExt;
use tokio::io::AsyncWriteExt;

use crate::Multiaddr;

const MIN_WRITE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub(super) enum PeerAddressesPersistence {
	Disabled,
	Enabled(Enabled),
}

type IoResult<T> = Result<T, IoError>;
type BusyFuture = dyn Future<Output = IoResult<()>> + Send;
type BusyFutureBoxed = Pin<Box<BusyFuture>>;

pub(super) struct Enabled {
	path: Arc<Path>,
	tmp_path: Arc<Path>,
	flushed_at: Instant,
	entries: Arc<[Multiaddr]>,
	busy: Option<BusyFutureBoxed>,
}

impl PeerAddressesPersistence {
	pub fn init<P: AsRef<Path>>(path: Option<P>) -> Self {
		if let Some(path) = path {
			let entries = if let Ok(file_ro) = std::fs::OpenOptions::new().read(true).open(&path) {
				let entries: Vec<Multiaddr> = serde_json::from_reader(file_ro).unwrap_or_default();
				Arc::from(entries)
			} else {
				Arc::from(vec![])
			};

			let path = Arc::<Path>::from(path.as_ref().to_owned());
			let tmp_path = Arc::<Path>::from(path.with_extension("tmp"));

			let enabled =
				Enabled { path, tmp_path, flushed_at: Instant::now(), entries, busy: None };

			Self::Enabled(enabled)
		} else {
			Self::Disabled
		}
	}

	pub fn entries(&self) -> &[Multiaddr] {
		if let Self::Enabled(enabled) = self {
			&enabled.entries
		} else {
			&[]
		}
	}

	pub fn poll<'a, I>(&mut self, cx: &mut Context, latest: I)
	where
		I: IntoIterator<Item = &'a Multiaddr> + 'a,
	{
		if let Self::Enabled(enabled) = self {
			enabled.poll(cx, latest)
		}
	}
}

impl Enabled {
	fn can_flush(&self) -> bool {
		self.flushed_at.elapsed() > MIN_WRITE_INTERVAL
	}

	fn poll<'a, I>(&mut self, cx: &mut Context, latest: I)
	where
		I: IntoIterator<Item = &'a Multiaddr> + 'a,
	{
		if let Some(busy) = self.busy.as_mut() {
			match busy.as_mut().poll(cx) {
				Poll::Ready(result) => {
					if let Err(reason) = result {
						log::warn!(target: "sub-libp2p", "Discovery persistence error: {}", reason);
					}

					self.flushed_at = Instant::now();
					self.busy = None;
				},
				Poll::Pending => (),
			}
		} else if self.can_flush() {
			let latest: Vec<_> = latest.into_iter().cloned().collect();

			if &latest[..] != &self.entries[..] {
				self.entries = Arc::from(latest);
				let busy = persist(
					self.path.to_owned(),
					self.tmp_path.to_owned(),
					self.entries.to_owned(),
				)
				.boxed();
				self.busy = Some(busy);
			}
		}
	}
}

async fn persist(
	path: Arc<Path>,
	tmp_path: Arc<Path>,
	entries: Arc<[Multiaddr]>,
) -> Result<(), IoError> {
	eprintln!("!!! SAVING [path: {:?}; tmp-path: {:?}; entries: {:?}]", path, tmp_path, entries);

	let mut tmp_file_rw = tokio::fs::OpenOptions::new()
		.create(true)
		.write(true)
		.truncate(true)
		.open(&tmp_path)
		.await?;
	{
		let entries_json = serde_json::to_vec_pretty(&entries[..])?;
		tmp_file_rw.write_all(&entries_json[..]).await?;
		tmp_file_rw.sync_data().await?;
	}
	tokio::fs::rename(tmp_path, path).await?;

	Ok(())
}

impl fmt::Debug for Enabled {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Enabled")
			.field("path", &self.path)
			.field("flused_at", &self.flushed_at)
			.field("entries", &self.entries)
			.finish()
	}
}
