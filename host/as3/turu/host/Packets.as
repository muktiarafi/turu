package turu.host {

/**
 * The game's packets for the bot, from its SmartFox client (`game.sfc`):
 *
 * - `pext`: every extension response, as JSON (`params.type`, `params.dataObj`).
 * - `packet`: every packet the client sends, read from SmartFox's debug messages, so `debug`
 *   is kept on (the game's own packet logger turns it off). SmartFox's client is sealed, so its
 *   sends can't be wrapped from here; a hook in the client replaces this later
 *   (docs/HOST_SWF_PROPOSAL.md, R5).
 * - `packetFromServer`: the other debug messages, only after `setPacketEcho(true)`.
 * - `connectionLost`.
 *
 * The client may make a new SmartFox client; [frame] follows it.
 */
public class Packets {
    private static const SENT:String = "[Sending - STR]: ";
    private static const SENT_GAME:String = "[Sending - STR]: %xt%zm%";
    private static const PREFIXES:Array = ["[Sending - STR]: ", "[ RECEIVED ]: ", "[Sending]: "];
    private static const LENGTH:RegExp = /, \(len: \d+\)$/;
    /** SmartFox ends a sent message's debug line with its line break. */
    private static const LINE_END:RegExp = /[\r\n]+$/;

    private var host:Host;
    /** The SmartFox client listened to. */
    private var sfc:*;
    private var echo:Boolean = false;

    public function Packets(host:Host) {
        this.host = host;
    }

    /** `setPacketEcho(on)`: whether `packetFromServer` is sent. */
    public function setEcho(on:*):void {
        echo = on === true || on == "true";
    }

    /** Each frame: listen to the game's SmartFox client, and keep its debug messages on. */
    public function frame(game:*):void {
        var now:* = game.sfc;
        if (now == null) {
            return;
        }
        if (now !== sfc) {
            listen(now);
        }
        if (!now.debug) {
            now.debug = true;
        }
    }

    private function listen(next:*):void {
        if (sfc != null) {
            sfc.removeEventListener("onExtensionResponse", onExtension);
            sfc.removeEventListener("onConnectionLost", onConnectionLost);
            sfc.removeEventListener("onDebugMessage", onDebugMessage);
        }
        sfc = next;
        sfc.addEventListener("onExtensionResponse", onExtension);
        sfc.addEventListener("onConnectionLost", onConnectionLost);
        sfc.addEventListener("onDebugMessage", onDebugMessage);
    }

    private function onExtension(event:Object):void {
        host.call("pext", [JSON.stringify({type: event.type, params: event.params})]);
    }

    private function onConnectionLost(event:Object):void {
        host.call("connectionLost", []);
    }

    private function onDebugMessage(event:Object):void {
        var message:String = String(event.params.message);
        if (message.indexOf(SENT_GAME) == 0) {
            host.call("packet", [message.substr(SENT.length).replace(LINE_END, "")]);
        } else if (echo) {
            for each (var prefix:String in PREFIXES) {
                if (message.indexOf(prefix) == 0) {
                    message = message.substr(prefix.length);
                    break;
                }
            }
            host.call("packetFromServer", [message.replace(LENGTH, "")]);
        }
    }
}
}
