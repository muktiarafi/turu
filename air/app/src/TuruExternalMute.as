package {
    import flash.utils.setTimeout;

    /**
     * skua's `ExternalInterface` in a client the engine doesn't drive (spike: several clients on
     * one stage, docs/AIR_ARMY_SPIKE.md): sends nothing, except that it answers skua's
     * `requestLoadGame` itself with `loadClient`, as the engine does for its own client.
     *
     * Its callbacks are static, so they're the last muted client's: enough for one more client.
     */
    public class TuruExternalMute {
        private static var callbacks:Object = {};

        public static function get available():Boolean {
            return true;
        }

        public static function addCallback(name:String, fn:Function):void {
            callbacks[name] = fn;
        }

        public static function call(name:String, ...rest):* {
            if (name == "requestLoadGame") {
                // Not from inside skua's own call.
                setTimeout(function():void {
                    var load:Function = callbacks["loadClient"];
                    if (load != null) {
                        load();
                    }
                }, 0);
            }
            return null;
        }
    }
}
