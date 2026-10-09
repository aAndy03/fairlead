use serde::{Deserialize, Serialize};

/// Who may write a file and what lines it may hold (D-19, R-FILE-001).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum FileClass {
    Authored,
    Generated,
    Reference,
    VendorOwned,
    Ledger,
    ExternalGenerated,
}
