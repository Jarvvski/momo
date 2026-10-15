//! The stable error-code vocabulary.
//!
//! Every error a user can act on carries a code — a short, namespaced string
//! (`momo.version.stale-candidate`) that is stable enough to grep, document and
//! allow-list in CI. The code lives on the error type, beside its `Display`, so
//! a variant and its code move together.
//!
//! Only actionable variants get a real code. A wrapper or internal variant
//! reports [`UNCLASSIFIED`], which the CLI's boundary skips so a more specific
//! error deeper in the chain still wins; an error with no code anywhere in its
//! chain falls through to the same code.
//!
//! Returning `&'static str` keeps this pure: no I/O, no environment, no CLI.

/// The code an error carries when momo cannot classify it.
pub const UNCLASSIFIED: &str = "momo.error.unclassified";

/// A stable, greppable code for an error a user can act on.
pub trait Coded {
    /// The stable code, namespaced `momo.<area>.<variant>`.
    fn code(&self) -> &'static str;

    /// What the user can do about it, when there is something they can do.
    fn fix(&self) -> Option<String> {
        None
    }

    /// Documentation for this code, when the generic `docs/errors.md` anchor is
    /// not where the user should land.
    fn url(&self) -> Option<&'static str> {
        None
    }
}

/// Implement [`Coded`] for an error enum, one line per variant.
///
/// Each arm pairs a variant pattern with its code, and optionally a `fix`
/// expression and a `url`; the pattern binds whatever the fix needs. A variant
/// that is not user-actionable is given [`UNCLASSIFIED`], so the CLI's chain
/// scan keeps looking for a more specific error.
macro_rules! impl_coded {
    (
        $ty:ty {
            $(
                $pattern:pat => ($code:literal
                    $(, fix = $fix:expr)?
                    $(, url = $url:expr)?
                    $(,)?
                )
            ),* $(,)?
        }
    ) => {
        impl $ty {
            /// The code, fix and documentation URL, resolved in one match so
            /// each variant's bindings are used exactly where the fix needs
            /// them.
            fn coded_parts(&self) -> (&'static str, Option<String>, Option<&'static str>) {
                match self {
                    $(
                        $pattern => (
                            $code,
                            $crate::domain::diagnostic::impl_coded!(@fix $($fix)?),
                            $crate::domain::diagnostic::impl_coded!(@url $($url)?),
                        ),
                    )*
                }
            }
        }

        impl $crate::domain::diagnostic::Coded for $ty {
            fn code(&self) -> &'static str {
                self.coded_parts().0
            }

            fn fix(&self) -> Option<String> {
                self.coded_parts().1
            }

            fn url(&self) -> Option<&'static str> {
                self.coded_parts().2
            }
        }

        #[cfg(test)]
        impl $ty {
            /// Every code this type reports, for the docs drift test.
            pub(crate) const CODES: &'static [&'static str] = &[ $( $code ),* ];
        }
    };

    (@fix $fix:expr) => { Some($fix.to_string()) };
    (@fix) => { None };
    (@url $url:expr) => { Some($url) };
    (@url) => { None };
}

pub(crate) use impl_coded;
