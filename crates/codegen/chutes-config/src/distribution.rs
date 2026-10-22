//! What a build of this CLI may do. Call sites ask [`Distribution::allows`] instead of testing Cargo
//! features, so each distribution's answers are testable in any build.
//!
//! A withheld capability stays off whatever config, requirements or the environment say. Keeping a
//! service's code out of the binary is the Cargo feature set's job.
/// The capabilities a build withholds. [`Distribution::current`] is this build's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Distribution {
    withheld: u32,
}
impl Default for Distribution {
    /// A fresh value is always this build's distribution; other
    /// distributions only exist where a caller names one explicitly.
    fn default() -> Self {
        Self::current()
    }
}
/// Something a distribution may withhold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::EnumIter)]
pub enum Capability {
    /// Product telemetry and trace upload.
    Telemetry,
    /// Crash and error reports.
    ErrorReporting,
    /// `/v1/models` and `/v1/settings` fetches and the managed-config sync.
    RemoteFetch,
    /// A login kept in `auth.json` or `CHUTES_BUILD_AUTH`, an auth provider command, and signing in to or
    /// out of an account. Without it the only credential is the API key the build was given.
    AccountLogin,
    /// Controlling this machine from other devices.
    RemoteControl,
    /// Voice dictation, whose speech goes to a speech-to-text service.
    Voice,
}
impl Capability {
    const fn bit(self) -> u32 {
        1 << self as u32
    }
}
impl Distribution {
    /// Every capability, subject to config, requirements and the environment.
    pub const STOCK: Self = Self { withheld: 0 };
    /// A distribution that withholds `capabilities` and allows the rest.
    pub const fn withholding(capabilities: &[Capability]) -> Self {
        let mut withheld = 0;
        let mut rest = capabilities;
        while let [capability, tail @ ..] = rest {
            withheld |= capability.bit();
            rest = tail;
        }
        Self { withheld }
    }
    /// This build's distribution: Chutes Build withholds every capability
    /// that reports to, or is driven from, anyone but the Chutes ecosystem.
    /// Product telemetry, crash/error reports, remote control of this
    /// machine, and voice dictation to a third-party STT service stay off
    /// whatever config, requirements or the environment say. Account login
    /// stays off too: the only credential is the `CHUTES_API_KEY` the build
    /// was given, so no sign-in screen, session token, or provider command
    /// is ever offered. `RemoteFetch` stays allowed: it also gates the
    /// Chutes model-catalog fetch, whose targets are Chutes-only by
    /// endpoint default.
    pub const CHUTES: Self = Self::withholding(&[
        Capability::Telemetry,
        Capability::ErrorReporting,
        Capability::RemoteControl,
        Capability::Voice,
        Capability::AccountLogin,
    ]);
    /// This build's distribution.
    pub const fn current() -> Self {
        Self::CHUTES
    }
    pub const fn allows(self, capability: Capability) -> bool {
        self.withheld & capability.bit() == 0
    }
    /// What a surface that offers `capability` tells the user when this distribution withholds it.
    /// `None` where the distribution allows it, or where no surface offers it to refuse.
    pub const fn refusal(self, capability: Capability) -> Option<&'static str> {
        if self.allows(capability) {
            return None;
        }
        match capability {
            Capability::AccountLogin => Some(
                "This build has no account sign-in: run `chutes-build login` to save your Chutes API key, \
                 or set the CHUTES_API_KEY environment variable.",
            ),
            Capability::RemoteControl => Some("Remote Control is off in this build."),
            Capability::Voice => Some("Voice dictation is off in this build."),
            Capability::Telemetry | Capability::ErrorReporting | Capability::RemoteFetch => None,
        }
    }
}
#[cfg(test)]
#[path = "distribution_tests.rs"]
mod tests;
