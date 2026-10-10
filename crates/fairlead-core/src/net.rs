//! The only module allowed to reach the network (R-ARCH-124, R-EDN-025): `upgrade --fetch`, to
//! retrieve edition migration data, never a binary. Behind the `fetch` feature, off by default.
//! Owner question Q-2: 0.2 publishes no remote migration data, so there is no client here yet;
//! every supported migration is embedded in the binary.

/// What `upgrade --fetch` reports while no remote data exists.
pub const NO_REMOTE_DATA: &str = "no remote migration data is published for this binary";
