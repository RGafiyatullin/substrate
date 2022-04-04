use codec::{Codec, Decode, Encode};

use crate::{Constraints, Hash, MetaDb, PruningMode, StateDb};

const PRUNING_MODE: &[u8] = b"mode";
const PRUNING_MODE_ARG: &[u8] = b"mode_arg";

const PRUNING_MODE_ARCHIVE: &[u8] = b"archive";
const PRUNING_MODE_ARCHIVE_CANON: &[u8] = b"archive_canonical";
const PRUNING_MODE_CONSTRAINED: &[u8] = b"constrained";

fn to_meta_key<S: Codec>(suffix: &[u8], data: &S) -> Vec<u8> {
	let mut buffer = data.encode();
	buffer.extend(suffix);
	buffer
}

impl<BlockHash: Hash, Key: Hash> StateDb<BlockHash, Key> {
	///
	pub fn meta_data_fetch_pruning_mode<D: MetaDb>(
		meta_db: &D,
	) -> Result<Option<PruningMode>, D::Error> {
		let mode = meta_db.get_meta(&to_meta_key(PRUNING_MODE, &()))?;
		let mode_arg = meta_db.get_meta(&to_meta_key(PRUNING_MODE_ARG, &()))?;

		eprintln!("mode:\t{:?}", mode);
		eprintln!("mode-arg:\t{:?}", mode_arg);

		let pruning_mode_opt = match mode.as_ref().map(AsRef::as_ref) {
			None => None,
			Some(PRUNING_MODE_ARCHIVE) => Some(PruningMode::ArchiveAll),
			Some(PRUNING_MODE_ARCHIVE_CANON) => Some(PruningMode::ArchiveCanonical),
			Some(PRUNING_MODE_CONSTRAINED) =>
				if let Some(mod_arg) = mode_arg {
					let mut decode_input = mod_arg.as_ref();
					let constraints = Constraints::decode(&mut decode_input).expect(
						"FIXME: Should wrap D::Error into something able to contain a DecodeError",
					);
					Some(PruningMode::Constrained(constraints))
				} else {
					Some(PruningMode::Constrained(Default::default()))
				},
			_ => unimplemented!("Unexpected pruning-mode in the meta-data"),
		};

		Ok(pruning_mode_opt)
	}

	pub fn meta_data_write_pruning_mode<D: MetaDb>(
		meta_db: &mut D,
		pruning_mode: PruningMode,
	) -> Result<(), D::Error> {
		let (mode_value, mode_arg_value) = match pruning_mode {
			PruningMode::ArchiveAll => (PRUNING_MODE_ARCHIVE, None),
			PruningMode::ArchiveCanonical => (PRUNING_MODE_ARCHIVE_CANON, None),
			PruningMode::Constrained(constraints) => {
				let mode_arg_value = constraints.encode();
				(PRUNING_MODE_CONSTRAINED, Some(mode_arg_value))
			},
		};
		meta_db.set_meta(&to_meta_key(PRUNING_MODE, &()), Some(mode_value))?;
		meta_db.set_meta(&to_meta_key(PRUNING_MODE_ARG, &()), mode_arg_value.as_ref())?;

		Ok(())
	}
}
