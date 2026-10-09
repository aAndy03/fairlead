//! Typed errors; the library never exits the process (R-ARCH-100). The bin maps `class()` to codes.
use crate::config::ConfigFailure;
use crate::diag::Diagnostic;
use crate::discover::PinRefusal;
use std::fmt;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(
        "no root managerial folder found searching up from {searched_from}; run `fairlead init` to create one"
    )]
    NoRoot { searched_from: String },
    #[error("path is outside the root: {path}")]
    PathOutsideRoot { path: String },
    #[error("{0}")]
    Usage(String),
    #[error("{verb} arrives in {milestone}")]
    Reserved {
        verb: &'static str,
        milestone: &'static str,
    },
    #[error("control file {} is invalid", .0.path)]
    Config(Box<ConfigFailure>),
    #[error("refused by pin: {}", .0.reason)]
    Pin(PinRefusal),
    #[error("refused: {}", .0.message)]
    Refused(Refusal),
    #[error("{0}")]
    Io(IoFailure),
    #[error("cannot determine: {0}")]
    CannotDetermine(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ErrorClass {
    Usage,
    Refused,
    Internal,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RefusalKind {
    PinMismatch,
    LockHeld,
    NoClobber,
    HandEditedGenerated,
    PlanNotConfirmed,
    NotATty,
    CiSet,
    UnbalancedBlock,
    Floor,
    /// A root exists above (`init`) (A-22).
    Nested,
    /// The owner is not in `owners`, or an authority-family entry is unapproved (A-22).
    Authority,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Refusal {
    pub kind: RefusalKind,
    pub message: String,
    pub hint: String,
}

/// A failed file-system operation (C-10).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IoFailure {
    pub path: String,
    pub op: &'static str,
    pub kind: std::io::ErrorKind,
    pub detail: String,
    /// True when the path came from the user (flag, argument), so NotFound is a usage error.
    pub user_path: bool,
}

impl fmt::Display for IoFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot {} {}: {}", self.op, self.path, self.detail)
    }
}

impl Error {
    pub fn io(
        path: impl Into<String>,
        op: &'static str,
        err: &std::io::Error,
        user_path: bool,
    ) -> Error {
        Error::Io(IoFailure {
            path: path.into(),
            op,
            kind: err.kind(),
            detail: err.to_string(),
            user_path,
        })
    }

    pub fn class(&self) -> ErrorClass {
        match self {
            Error::NoRoot { .. }
            | Error::PathOutsideRoot { .. }
            | Error::Usage(_)
            | Error::Reserved { .. }
            | Error::Config(_) => ErrorClass::Usage,
            Error::Pin(_) | Error::Refused(_) => ErrorClass::Refused,
            Error::Io(f) => {
                if f.kind == std::io::ErrorKind::NotFound && f.user_path {
                    ErrorClass::Usage
                } else {
                    ErrorClass::Internal
                }
            }
            Error::CannotDetermine(_) | Error::Internal(_) => ErrorClass::Internal,
        }
    }

    /// Diagnostics printed like findings (R-ARCH-103); empty for errors that carry none.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Error::Config(c) => &c.diags,
            Error::Pin(p) => &p.diags,
            _ => &[],
        }
    }
}

impl From<ConfigFailure> for Error {
    fn from(f: ConfigFailure) -> Self {
        Error::Config(Box::new(f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigFailure;
    use crate::diag::{Diagnostic, Location, Severity, codes};
    use crate::discover::PinRefusal;
    use crate::path::RepoPath;

    fn io(kind: std::io::ErrorKind, user_path: bool) -> Error {
        Error::io(
            "some/file",
            "read",
            &std::io::Error::new(kind, "boom"),
            user_path,
        )
    }

    #[test]
    fn r_arch_100__class_mapping() {
        use ErrorClass::*;
        assert_eq!(
            Error::NoRoot {
                searched_from: "x".into()
            }
            .class(),
            Usage
        );
        assert_eq!(Error::PathOutsideRoot { path: "x".into() }.class(), Usage);
        assert_eq!(Error::Usage("x".into()).class(), Usage);
        assert_eq!(
            Error::Reserved {
                verb: "seal",
                milestone: "0.2"
            }
            .class(),
            Usage
        );
        let cf = ConfigFailure {
            path: RepoPath::root(),
            diags: vec![],
        };
        assert_eq!(Error::Config(Box::new(cf)).class(), Usage);
        let pin = PinRefusal {
            install_line: "i".into(),
            reason: "r".into(),
            diags: Vec::new(),
        };
        assert_eq!(Error::Pin(pin).class(), Refused);
        let r = crate::error::Refusal {
            kind: RefusalKind::LockHeld,
            message: "m".into(),
            hint: "h".into(),
        };
        assert_eq!(Error::Refused(r).class(), Refused);
        assert_eq!(Error::CannotDetermine("x".into()).class(), Internal);
        assert_eq!(Error::Internal("x".into()).class(), Internal);
    }

    #[test]
    fn r_arch_100__io_not_found_on_user_path_is_usage_else_internal() {
        assert_eq!(
            io(std::io::ErrorKind::NotFound, true).class(),
            ErrorClass::Usage
        );
        assert_eq!(
            io(std::io::ErrorKind::NotFound, false).class(),
            ErrorClass::Internal
        );
        assert_eq!(
            io(std::io::ErrorKind::PermissionDenied, true).class(),
            ErrorClass::Internal
        );
    }

    #[test]
    fn r_arch_102__cannot_determine_is_internal_and_says_so() {
        let e = Error::CannotDetermine("git state unreadable".into());
        assert_eq!(e.class(), ErrorClass::Internal);
        assert!(e.to_string().contains("git state unreadable"));
    }

    #[test]
    fn r_arch_100__config_error_exposes_diagnostics() {
        let d = Diagnostic::new(
            codes::HN_C001,
            Severity::Error,
            Location::file(RepoPath::new("fairlead/control.toml").unwrap()),
            "bad",
        );
        let e = Error::from(ConfigFailure {
            path: RepoPath::new("fairlead/control.toml").unwrap(),
            diags: vec![d.clone()],
        });
        assert_eq!(e.diagnostics(), &[d]);
        assert!(Error::Usage("x".into()).diagnostics().is_empty());
        assert!(e.to_string().contains("fairlead/control.toml"));
    }

    #[test]
    fn r_arch_100__reserved_verb_message_names_the_milestone() {
        assert_eq!(
            Error::Reserved {
                verb: "seal",
                milestone: "0.2"
            }
            .to_string(),
            "seal arrives in 0.2"
        );
    }

    #[test]
    fn r_arch_100__core_never_calls_process_exit() {
        // Built from two literals so this file does not contain the needle itself.
        let needle = concat!("process", "::exit");
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        let mut checked = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let p = entry.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    assert!(
                        !text.contains(needle),
                        "{} calls the process-exit function",
                        p.display()
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 5);
    }
}
