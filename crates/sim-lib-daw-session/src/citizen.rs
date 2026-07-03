use sim_citizen_derive::Citizen;
use sim_kernel::{Expr, Result, Symbol};

use crate::DawSession;

/// Citizen descriptor that carries a [`DawSession`] as a runtime object.
///
/// The session is stored in its [`Expr`] encoding so it can round-trip through
/// the citizen protocol; [`DawSessionDescriptor::session`] decodes it back.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "daw-session/DawSession", version = 1)]
pub struct DawSessionDescriptor {
    #[citizen(with = "session_expr")]
    session: Expr,
}

impl DawSessionDescriptor {
    /// Wraps a session as a citizen descriptor, encoding it to its expression
    /// form.
    pub fn new(session: DawSession) -> Self {
        Self {
            session: session.to_expr(),
        }
    }

    /// Builds a descriptor from a session expression, validating that it decodes
    /// to a [`DawSession`].
    pub fn from_expr(expr: Expr) -> Result<Self> {
        session_expr::decode(&expr)?;
        Ok(Self { session: expr })
    }

    /// Decodes and returns the wrapped [`DawSession`].
    pub fn session(&self) -> Result<DawSession> {
        DawSession::from_expr(&self.session)
    }

    /// Returns the underlying session expression without decoding it.
    pub fn as_expr(&self) -> &Expr {
        &self.session
    }
}

impl Default for DawSessionDescriptor {
    fn default() -> Self {
        Self::new(
            DawSession::new("citizen-session", "Citizen Session", 48_000)
                .expect("default DAW session descriptor should be valid"),
        )
    }
}

/// Returns the class symbol under which DAW sessions register as citizens.
pub fn daw_session_class_symbol() -> Symbol {
    Symbol::qualified("daw-session", "DawSession")
}

pub(crate) mod session_expr {
    use sim_kernel::{Expr, Result};

    use crate::DawSession;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        DawSession::from_expr(expr)?;
        Ok(expr.clone())
    }
}
