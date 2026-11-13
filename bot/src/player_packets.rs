//! Packets from the player (docs/HOST_SWF_PROPOSAL.md, R5): on the Ruffle builds ruffle-aqw tells
//! the bot what the game's sockets send and, once the client has handled them, what they
//! received ([`Bot::on_socket`]). They become the bot's `packet` and `pext`, as the host SWF's
//! events were, in the same shapes, so nothing above them changes.
//!
//! SmartFox ends every message with a null byte; a message can be split across reads, so each
//! socket keeps what's left of its last one. A received message becomes `pext` as the client's
//! SmartFox (`ExtHandler.handleMessage`) builds `onExtensionResponse`'s `params`: for a string
//! message (`%xt%cmd%-1%...%`) `dataObj` is its fields after `xt`; for a JSON one
//! (`{"t":"xt","b":{"o":{...}}}`) it's `b.o`. Other messages (XML, system ones) aren't the bot's.
//! A compressed message (`Z` and zlib data in Base64, [`crate::capture::unpack`]) is unpacked
//! first, as the client does.

use crate::Bot;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

/// Which way a socket's bytes went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Sent,
    Received,
}

/// What's left of each socket's last message, each way, until its null byte comes.
#[derive(Default)]
pub struct SocketBuffers {
    partial: Mutex<HashMap<(u64, bool), Vec<u8>>>,
}

/// A message longer than this without a null byte isn't SmartFox's: dropped.
const MAX_MESSAGE: usize = 8 * 1024 * 1024;

impl SocketBuffers {
    /// `data` added to `socket`'s buffer; the messages it completed, as text.
    pub fn messages(&self, socket: u64, direction: Direction, data: &[u8]) -> Vec<String> {
        let mut partial = self.partial.lock();
        let buffer = partial
            .entry((socket, direction == Direction::Sent))
            .or_default();
        buffer.extend_from_slice(data);
        let mut messages = Vec::new();
        while let Some(end) = buffer.iter().position(|&b| b == 0) {
            let message: Vec<u8> = buffer.drain(..=end).collect();
            let text = String::from_utf8_lossy(&message[..message.len() - 1]).into_owned();
            if !text.is_empty() {
                messages.push(text);
            }
        }
        if buffer.len() > MAX_MESSAGE {
            buffer.clear();
        }
        messages
    }

    /// The socket closed: whatever it held is gone.
    pub fn closed(&self, socket: u64) {
        self.partial.lock().retain(|(s, _), _| *s != socket);
    }
}

/// An outgoing message the bot logs and acts on (`packet`): the game's `%xt%zm%...` strings,
/// what VibeSkua's SWF sends as `packet`.
pub fn sent_packet(message: &str) -> Option<&str> {
    message.starts_with("%xt%zm%").then_some(message)
}

/// A received message as the `pext` the host SWF sends (`{type, params: {type, dataObj}}`), or
/// `None` when it isn't an extension response.
pub fn extension_packet(message: &str) -> Option<Value> {
    let (kind, data) = if message.starts_with('%') && message.len() >= 2 {
        let inner = &message[1..message.len() - 1];
        let mut fields = inner.split('%');
        if fields.next() != Some("xt") {
            return None;
        }
        let data: Vec<Value> = fields.map(|f| Value::String(f.to_owned())).collect();
        ("str", Value::Array(data))
    } else if message.starts_with('{') {
        let parsed: Value = match serde_json::from_str(message) {
            Ok(parsed) => parsed,
            Err(e) => {
                let start: String = message.chars().take(300).collect();
                tracing::warn!("A packet from the server isn't JSON ({e}): {start}");
                return None;
            }
        };
        if parsed.get("t").and_then(Value::as_str) != Some("xt") {
            return None;
        }
        ("json", parsed.get("b")?.get("o")?.clone())
    } else {
        return None;
    };
    Some(json!({
        "type": "onExtensionResponse",
        "params": {"type": kind, "dataObj": data}
    }))
}

impl Bot {
    /// What a socket of the game's sent, or received once the client handled it (the player's
    /// [`crate::host::FlashHost::player_packets`]). Called on the player's thread while it's
    /// locked: no Flash calls, no blocking, as [`Bot::on_flash_call`].
    pub fn on_socket(self: &Arc<Self>, socket: u64, direction: Direction, data: &[u8]) {
        for message in self.socket_buffers.messages(socket, direction, data) {
            match direction {
                Direction::Sent => {
                    if let Some(packet) = sent_packet(&message) {
                        crate::events::player_packet_sent(self, packet);
                    }
                }
                Direction::Received => {
                    // The server compresses its longer messages (`Z`, zlib in Base64): a
                    // monster's drops, `moveToArea`, `getQuests`, ...; the client unpacks them.
                    let message = crate::capture::unpack(&message).unwrap_or(message);
                    if let Some(pext) = extension_packet(&message) {
                        crate::events::player_extension_packet(self, pext);
                    }
                }
            }
        }
    }

    /// A socket of the game's closed.
    pub fn on_socket_closed(&self, socket: u64) {
        self.socket_buffers.closed(socket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_split_on_null_bytes_across_reads() {
        let buffers = SocketBuffers::default();
        assert!(
            buffers
                .messages(1, Direction::Received, b"%xt%a%")
                .is_empty()
        );
        let done = buffers.messages(1, Direction::Received, b"-1%\0%xt%b%-1%\0%xt%c");
        assert_eq!(done, ["%xt%a%-1%", "%xt%b%-1%"]);
        assert!(
            buffers.messages(2, Direction::Received, b"x\0").len() == 1,
            "sockets apart"
        );
        assert_eq!(
            buffers.messages(1, Direction::Received, b"%-1%\0"),
            ["%xt%c%-1%"]
        );
        buffers.closed(1);
        assert!(buffers.partial.lock().keys().all(|(s, _)| *s != 1));
    }

    #[test]
    fn sent_and_received_ways_keep_their_own_buffers() {
        let buffers = SocketBuffers::default();
        buffers.messages(1, Direction::Sent, b"%xt%zm%a");
        assert_eq!(buffers.messages(1, Direction::Received, b"b\0"), ["b"]);
        assert_eq!(buffers.messages(1, Direction::Sent, b"%\0"), ["%xt%zm%a%"]);
    }

    #[test]
    fn only_game_strings_are_sent_packets() {
        assert_eq!(
            sent_packet("%xt%zm%moveToCell%1%Enter%Spawn%"),
            Some("%xt%zm%moveToCell%1%Enter%Spawn%")
        );
        assert_eq!(
            sent_packet("<msg t='sys'><body action='login'/></msg>"),
            None
        );
    }

    #[test]
    fn string_messages_become_their_fields_after_xt() {
        let pext = extension_packet("%xt%uotls%-1%Questwalker%afk:true%").unwrap();
        assert_eq!(
            pext,
            json!({"type": "onExtensionResponse",
                   "params": {"type": "str", "dataObj": ["uotls", "-1", "Questwalker", "afk:true"]}})
        );
    }

    #[test]
    fn json_messages_become_their_object() {
        let pext = extension_packet(
            r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"moveToArea","strMapName":"battleon"}}}"#,
        )
        .unwrap();
        assert_eq!(
            pext["params"],
            json!({"type": "json", "dataObj": {"cmd": "moveToArea", "strMapName": "battleon"}})
        );
    }

    #[test]
    fn other_messages_arent_extension_packets() {
        assert_eq!(
            extension_packet("<msg t='sys'><body action='apiOK'/></msg>"),
            None
        );
        assert_eq!(extension_packet(r#"{"t":"sys","b":{}}"#), None);
        assert_eq!(extension_packet("%xx%a%"), None);
        assert_eq!(extension_packet(""), None);
    }
}
