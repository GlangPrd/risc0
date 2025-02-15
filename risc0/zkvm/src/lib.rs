// Copyright 2024 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(missing_docs)]

//! The RISC Zero zkVM is a RISC-V virtual machine that produces [zero-knowledge
//! proofs] of code it executes. By using the zkVM, a cryptographic [receipt] is
//! produced which anyone can [verify][receipt-verify] was produced by the
//! zkVM's guest code. No additional information about the code execution (such
//! as, for example, the inputs provided) is revealed by publishing the
//! [receipt].
//!
//! Additional (non-reference) resources for using our zkVM that you may also
//! find helpful, especially if you're new to the RISC Zero zkVM. These include:
//!
//! * Our [zkVM Tutorial], which walks you through writing your first zkVM
//!   project.
//! * The [`cargo risczero` tool]. It includes a `new` command which generates
//!   code for building and launching a zkVM guest and guidance on where
//!   projects most commonly modify host and guest code.
//! * The [examples], which contains various examples using our zkVM.
//! * [This clip][zkHack] from our presentation at ZK Hack III gives an overview
//!   of the RISC Zero zkVM. [Our YouTube channel][YouTube] has many more videos
//!   as well.
//! * We track zkVM issues with known workarounds using the [rust guest
//!   workarounds] GitHub tag. If you're having problems running your code in
//!   the zkVM, you can see if there's a workaround, and if you're using a
//!   workaround, you can track when it gets resolved to a permanent solution.
//! * And more on [the RISC Zero developer website][dev-docs]!
//!
//! # Crate Feature Flags
//!
//! The following feature flags are supported.
//!
//! Note that in order to use `risc0-zkvm` in the guest, you must disable the
//! "prove" feature by setting `default-features = false`.
//!
//! | Feature          | Target(s)         | Implies    | Description                                                                                                                                                  |
//! | ---------------- | ----------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
//! | client           | all except rv32im | std        | Enables the client API.                                                                                                                                      |
//! | cuda             |                   | prove, std | Enables CUDA GPU acceleration for the prover. Requires CUDA toolkit to be installed.                                                                         |
//! | disable-dev-mode | all except rv32im |            | Disables dev mode so that proving and verifying may not be faked. Used to prevent a misplaced `RISC0_DEV_MODE` from breaking security in production systems. |
//! | metal            | macos             | prove, std | Enables Metal GPU acceleration for the prover.                                                                                                               |
//! | prove            | all except rv32im | std        | Enables the prover, incompatible within the zkvm guest.                                                                                                      |
//! | std              | all               |            | Support for the Rust stdlib.                                                                                                                                 |
//!
//! [`cargo risczero` tool]: https://crates.io/crates/cargo-risczero
//! [dev-docs]: https://dev.risczero.com
//! [examples]: https://dev.risczero.com/api/zkvm/examples
//! [receipt]: crate::host::receipt::Receipt
//! [receipt-verify]: crate::host::receipt::Receipt::verify
//! [rust guest workarounds]:
//!     https://github.com/risc0/risc0/issues?q=is%3Aissue+is%3Aopen+label%3A%22rust+guest+workarounds%22
//! [YouTube]: https://www.youtube.com/@risczero
//! [zero-knowledge proofs]: https://en.wikipedia.org/wiki/Zero-knowledge_proof
//! [zkHack]: https://youtu.be/cLqFvhmXiD0
//! [zkVM Tutorial]: https://dev.risczero.com/api/zkvm/tutorials/hello-world

extern crate alloc;

pub mod guest;
#[cfg(not(target_os = "zkvm"))]
mod host;
mod receipt_claim;
pub mod serde;
pub mod sha;

/// Re-exports for recursion
#[cfg(all(not(target_os = "zkvm"), feature = "prove"))]
pub mod recursion {
    pub use super::host::recursion::*;
}

pub use anyhow::Result;
#[cfg(not(target_os = "zkvm"))]
#[cfg(any(feature = "client", feature = "prove"))]
pub use bytes::Bytes;
pub use risc0_binfmt::SystemState;
pub use risc0_zkvm_platform::{declare_syscall, memory::GUEST_MAX_MEM, PAGE_SIZE};

#[cfg(all(not(target_os = "zkvm"), feature = "prove"))]
pub use self::host::{
    api::server::Server as ApiServer,
    client::prove::local::LocalProver,
    server::{
        exec::executor::ExecutorImpl,
        prove::{get_prover_server, loader::Loader, HalPair, ProverServer},
        session::{FileSegmentRef, Segment, SegmentRef, Session, SessionEvents, SimpleSegmentRef},
    },
};
#[cfg(all(not(target_os = "zkvm"), feature = "client"))]
pub use self::host::{
    api::{client::Client as ApiClient, Asset, AssetRequest, Connector, SegmentInfo, SessionInfo},
    client::{
        env::{ExecutorEnv, ExecutorEnvBuilder},
        exec::TraceEvent,
        prove::{
            bonsai::BonsaiProver, default_executor, default_prover, external::ExternalProver,
            Executor, Prover, ProverOpts,
        },
    },
};
pub use self::receipt_claim::{
    Assumptions, ExitCode, InvalidExitCodeError, MaybePruned, Output, PrunedValueError,
    ReceiptClaim,
};
#[cfg(not(target_os = "zkvm"))]
pub use {
    self::host::{
        control_id::POSEIDON_CONTROL_ID,
        receipt::{
            Assumption, CompactReceipt, CompositeReceipt, InnerReceipt, Journal, Receipt,
            SegmentReceipt, SuccinctReceipt, VerifierContext,
        },
        recursion::ALLOWED_IDS_ROOT,
    },
    risc0_binfmt::compute_image_id,
};

use semver::Version;

/// Reports the current version of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Reports the current version of this crate as represented by a
/// [semver::Version].
pub fn get_version() -> Result<Version, semver::Error> {
    Version::parse(VERSION)
}

/// Align the given address `addr` upwards to alignment `align`.
///
/// Requires that `align` is a power of two.
pub const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Returns `true` if dev mode is enabled.
#[cfg(feature = "std")]
pub fn is_dev_mode() -> bool {
    let is_env_set = std::env::var("RISC0_DEV_MODE")
        .ok()
        .map(|x| x.to_lowercase())
        .filter(|x| x == "1" || x == "true" || x == "yes")
        .is_some();

    if cfg!(feature = "disable-dev-mode") && is_env_set {
        panic!("zkVM: Inconsistent settings -- please resolve. \
            The RISC0_DEV_MODE environment variable is set but dev mode has been disabled by feature flag.");
    }

    cfg!(not(feature = "disable-dev-mode")) && is_env_set
}
