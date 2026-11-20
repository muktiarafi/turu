//! A model's state that belongs to a tab but isn't the bot's (what a popup picked, what was read
//! for it): one value per bot, dropped with the bot. Turu Mas has several bots in one process.

use std::sync::{Arc, Mutex, Weak};
use turu_bot::Bot;

/// One tab's value, shared with the bot threads that fill it in.
pub type Shared<T> = Arc<Mutex<T>>;

/// One `T` per bot.
pub struct PerTab<T> {
    entries: Mutex<Vec<(Weak<Bot>, Shared<T>)>>,
}

impl<T: Default> Default for PerTab<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default> PerTab<T> {
    pub const fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// `bot`'s value (made the first time), shared: bot threads fill it in.
    pub fn of(&self, bot: &Arc<Bot>) -> Shared<T> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|(weak, _)| weak.strong_count() > 0);
        let me = Arc::downgrade(bot);
        if let Some((_, value)) = entries.iter().find(|(weak, _)| weak.ptr_eq(&me)) {
            return value.clone();
        }
        let value = Arc::new(Mutex::new(T::default()));
        entries.push((me, value.clone()));
        value
    }
}

/// Locks, through a poisoned lock.
pub fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
