package {
    import com.muktiarafi.turu.air.TuruAirNative;
    import flash.events.StatusEvent;
    import flash.utils.getTimer;

    /**
     * skua.swf's `flash.external.ExternalInterface` under AIR, which has none on Android:
     * `turu_air_patch` makes skua's `ExternalInterface` this class, which has the same
     * `addCallback` and `call`, through Turu's native extension.
     *
     * Each game client (tab) has its own skua, patched to call `TuruExternal<n>` (`TuruExternal0`,
     * ...), which hand their calls here with the client's number; the engine's calls name the
     * client too, so each tab's bot talks to its own skua.
     *
     * * `addCallback(name, fn)` keeps skua's callbacks; the engine's calls into skua arrive as
     *   status events ("call", `{"id", "client", "name", "args"}`), run here on AIR's thread, and the result
     *   goes back with `reply`, stringified as the Ruffle host does (`ExternalInterface`'s rules:
     *   booleans "true"/"false", null/undefined "null", whole numbers without a point, other
     *   values JSON).
     * * `call(name, ...)` hands skua's calls to the engine, its arguments as JSON.
     */
    public class TuruExternal {
        public static var native:TuruAirNative;
        /** Each client's callbacks, by name. */
        private static var callbacks:Array = [];

        public static function get available():Boolean {
            return native != null && native.available;
        }

        public static function start(extension:TuruAirNative):void {
            native = extension;
            native.addEventListener(StatusEvent.STATUS, onStatus);
        }

        /** Client `client`'s skua's `ExternalInterface.addCallback`. */
        public static function addCallbackFor(client:int, name:String, fn:Function):void {
            if (callbacks[client] == null) {
                callbacks[client] = {};
            }
            callbacks[client][name] = fn;
        }

        /** Client `client`'s skua's `ExternalInterface.call` (-1: the host's own). */
        public static function callFrom(client:int, name:String, rest:Array):* {
            var args:String;
            try {
                args = JSON.stringify(rest);
            } catch (e:Error) {
                args = "[]";
            }
            if (native != null) {
                native.flashCall(name, args, client);
            }
            return null;
        }

        /** Forgets client `client`'s callbacks (its tab closed). */
        public static function forget(client:int):void {
            callbacks[client] = null;
        }

        private static function onStatus(event:StatusEvent):void {
            if (event.code != "call") {
                return;
            }
            var request:Object;
            try {
                request = JSON.parse(event.level);
            } catch (e:Error) {
                return;
            }
            var own:Object = callbacks[int(request.client)];
            var fn:Function = own != null ? own[request.name] : null;
            var started:int = getTimer();
            var result:* = null;
            if (fn != null) {
                try {
                    result = fn.apply(null, request.args);
                } catch (e:Error) {
                    result = null;
                }
            }
            native.reply(request.id, stringify(result), (getTimer() - started) * 1000);
        }

        private static function stringify(value:*):String {
            if (value === null || value === undefined) {
                return "null";
            }
            if (value is String) {
                return value;
            }
            if (value is Boolean) {
                return value ? "true" : "false";
            }
            if (value is Number) {
                var n:Number = value;
                return n % 1 == 0 && Math.abs(n) < 1e15 ? n.toFixed(0) : String(n);
            }
            try {
                return JSON.stringify(value);
            } catch (e:Error) {
            }
            return String(value);
        }
    }
}
