// This file is part of Substrate.

// Copyright (C) 2021-2022 Parity Technologies (UK) Ltd.
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

//! Provides mpsc notification channel that can be instantiated
//! _after_ it's been shared to the consumer and producers entities.
//!
//! Useful when building RPC extensions where, at service definition time, we
//! don't know whether the specific interface where the RPC extension will be
//! exposed is safe or not and we want to lazily build the RPC extension
//! whenever we bind the service to an interface.
//!
//! See [`sc-service::builder::RpcExtensionBuilder`] for more details.

use crate::pubsub::{channels::TracingUnbounded, Hub, Receiver};

mod impl_traits;

mod registry;
use registry::Registry;

/// Trait used to define the "tracing key" string used to tag
/// and identify the mpsc channels.
pub trait TracingKeyStr {
	/// Const `str` representing the "tracing key" used to tag and identify
	/// the mpsc channels owned by the object implemeting this trait.
	const TRACING_KEY: &'static str;
}

/// The receiving half of the notifications channel.
///
/// The `NotificationStream` entity stores the `SharedSenders` so it can be
/// used to add more subscriptions.
#[derive(Clone)]
pub struct NotificationStream<Payload, TK: TracingKeyStr> {
	hub: Hub<TracingUnbounded<Payload>, Registry>,
	_pd: std::marker::PhantomData<TK>,
}

/// The receiving half of the notifications channel(s).
#[derive(Debug)]
pub struct NotificationReceiver<Payload> {
	receiver: Receiver<TracingUnbounded<Payload>, Registry>,
}

/// The sending half of the notifications channel(s).
///
/// Used to send notifications from the BEEFY gadget side.
pub struct NotificationSender<Payload> {
	hub: Hub<TracingUnbounded<Payload>, Registry>,
}

impl<Payload, TK: TracingKeyStr> NotificationStream<Payload, TK> {
	/// Creates a new pair of receiver and sender of `Payload` notifications.
	pub fn channel() -> (NotificationSender<Payload>, Self) {
		let channels = TracingUnbounded::new(TK::TRACING_KEY);
		let hub = Hub::new(channels);
		let sender = NotificationSender { hub: hub.clone() };
		let receiver = NotificationStream { hub, _pd: Default::default() };
		(sender, receiver)
	}

	/// Subscribe to a channel through which the generic payload can be received.
	pub fn subscribe(&self) -> NotificationReceiver<Payload> {
		let receiver = self.hub.subscribe(());
		NotificationReceiver { receiver }
	}
}

impl<Payload> NotificationSender<Payload> {
	/// Send out a notification to all subscribers that a new payload is available for a
	/// block.
	pub fn notify<Error>(
		&self,
		payload: impl FnOnce() -> Result<Payload, Error>,
	) -> Result<(), Error>
	where
		Payload: Clone,
	{
		// The subscribers collection used to be cleaned upon send previously.
		// The set used to be cleaned up twice:
		// - once before sending: filter on `!tx.is_closed()`;
		// - once while sending: filter on `!tx.unbounded_send().is_err()`.
		//
		// Since there is no `close` or `disconnect` operation defined on the
		// `NotificationReceiver<Payload>`, the only way to close the `rx` is to drop it.
		// Upon being dropped the `NotificationReceiver<Payload>` unregisters its `rx`
		// from the registry using its `_subs_guard`.
		//
		// So there's no need to clean up the subscribers set upon sending another message.

		let payload = payload()?; // FIXME: Did it have to be lazily instantiated?
		self.hub.dispatch(payload);

		Ok(())
	}
}

#[cfg(test)]
mod tests;
