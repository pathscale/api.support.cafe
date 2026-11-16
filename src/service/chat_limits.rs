use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use honey_id_types::id_entities::UserPublicId;
use parking_lot::Mutex;

use crate::id_types::AppPublicId;

const WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Default)]
struct Writes {
    sessions: u32,
    messages: u32,
}

impl Writes {
    const fn new(sessions: u32, messages: u32) -> Self {
        Self { sessions, messages }
    }

    fn can_add(self, charge: Self, limit: Self) -> bool {
        self.sessions + charge.sessions <= limit.sessions
            && self.messages + charge.messages <= limit.messages
    }

    fn add(&mut self, charge: Self) {
        self.sessions += charge.sessions;
        self.messages += charge.messages;
    }
}

const PROCESS_LIMIT: Writes = Writes::new(600, 6_000);
const APP_LIMIT: Writes = Writes::new(60, 600);
const USER_LIMIT: Writes = Writes::new(5, 60);

pub(super) enum ChatWrite {
    Session,
    Message,
    Contact,
}

#[derive(Debug)]
pub(crate) enum ChatWriteError {
    RateExceeded,
    MessageTooLarge,
}

impl fmt::Display for ChatWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RateExceeded => "Chat write rate exceeded; try again shortly",
            Self::MessageTooLarge => "Message must be at most 5000 characters",
        })
    }
}

impl std::error::Error for ChatWriteError {}

struct Window {
    started: Instant,
    total: Writes,
    apps: HashMap<AppPublicId, Writes>,
    users: HashMap<(AppPublicId, UserPublicId), Writes>,
}

impl Window {
    fn new(started: Instant) -> Self {
        Self {
            started,
            total: Writes::default(),
            apps: HashMap::new(),
            users: HashMap::new(),
        }
    }

    fn charge(
        &mut self,
        now: Instant,
        app: AppPublicId,
        user: UserPublicId,
        write: ChatWrite,
    ) -> Result<(), ChatWriteError> {
        if now.duration_since(self.started) >= WINDOW {
            self.started = now;
            self.total = Writes::default();
            self.apps.clear();
            self.users.clear();
        }

        let charge = match write {
            ChatWrite::Session => Writes::new(1, 0),
            ChatWrite::Message => Writes::new(0, 1),
            ChatWrite::Contact => Writes::new(1, 1),
        };
        let user_key = (app, user);
        if !self.total.can_add(charge, PROCESS_LIMIT)
            || !self
                .apps
                .get(&app)
                .copied()
                .unwrap_or_default()
                .can_add(charge, APP_LIMIT)
            || !self
                .users
                .get(&user_key)
                .copied()
                .unwrap_or_default()
                .can_add(charge, USER_LIMIT)
        {
            return Err(ChatWriteError::RateExceeded);
        }

        // Only admitted writes create keys, and every admission costs at
        // least one of the process's units. Both maps are therefore bounded.
        self.total.add(charge);
        self.apps.entry(app).or_default().add(charge);
        self.users.entry(user_key).or_default().add(charge);
        Ok(())
    }
}

pub(super) struct ChatWriteLimiter(Mutex<Window>);

impl ChatWriteLimiter {
    pub(super) fn new() -> Self {
        Self(Mutex::new(Window::new(Instant::now())))
    }

    pub(super) fn charge(
        &self,
        app: AppPublicId,
        user: UserPublicId,
        write: ChatWrite,
    ) -> Result<(), ChatWriteError> {
        let mut window = self.0.lock();
        window.charge(Instant::now(), app, user, write)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id_types::NanoId;

    #[test]
    fn resource_admission_contact_charges_both_budgets_without_partial_debit() {
        let now = Instant::now();
        let mut window = Window::new(now);
        let app = AppPublicId::from(NanoId::new());
        let user = UserPublicId::default();
        for _ in 0..USER_LIMIT.messages {
            window.charge(now, app, user, ChatWrite::Message).unwrap();
        }
        assert!(window.charge(now, app, user, ChatWrite::Contact).is_err());
        for _ in 0..USER_LIMIT.sessions {
            window.charge(now, app, user, ChatWrite::Session).unwrap();
        }
        assert!(window.charge(now, app, user, ChatWrite::Session).is_err());

        let later = now + WINDOW;
        for _ in 0..USER_LIMIT.sessions {
            window.charge(later, app, user, ChatWrite::Contact).unwrap();
        }
        assert!(window.charge(later, app, user, ChatWrite::Session).is_err());
        for _ in USER_LIMIT.sessions..USER_LIMIT.messages {
            window.charge(later, app, user, ChatWrite::Message).unwrap();
        }
        assert!(window.charge(later, app, user, ChatWrite::Message).is_err());
    }

    #[test]
    fn resource_admission_rotating_identities_cannot_bypass_app_or_process_limits() {
        let now = Instant::now();
        let mut window = Window::new(now);
        for _ in 0..PROCESS_LIMIT.sessions / APP_LIMIT.sessions {
            let app = AppPublicId::from(NanoId::new());
            for _ in 0..APP_LIMIT.sessions {
                window
                    .charge(now, app, UserPublicId::default(), ChatWrite::Session)
                    .unwrap();
            }
            assert!(
                window
                    .charge(now, app, UserPublicId::default(), ChatWrite::Session)
                    .is_err()
            );
        }
        let other_app = AppPublicId::from(NanoId::new());
        let user = UserPublicId::default();
        assert!(
            window
                .charge(now, other_app, user, ChatWrite::Session)
                .is_err()
        );
        assert!(
            window
                .charge(now, other_app, user, ChatWrite::Message)
                .is_ok()
        );
        assert!(
            window
                .charge(now + WINDOW, other_app, user, ChatWrite::Session)
                .is_ok()
        );
    }
}
