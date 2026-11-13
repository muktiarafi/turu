package com.muktiarafi.turu.air {
    import flash.events.EventDispatcher;
    import flash.events.StatusEvent;
    import flash.external.ExtensionContext;
    import flash.utils.ByteArray;

    /**
     * Turu's AIR native extension, from AS3. `available` is false where the extension has no
     * native side (AIR on a desktop): the calls then do nothing.
     */
    public class TuruAirNative extends EventDispatcher {
        public static const ID:String = "com.muktiarafi.turu.air";

        private var context:ExtensionContext;

        public function TuruAirNative() {
            try {
                context = ExtensionContext.createExtensionContext(ID, null);
            } catch (e:Error) {
                context = null;
            }
            if (context != null) {
                context.addEventListener(StatusEvent.STATUS, dispatchEvent);
            }
        }

        public function get available():Boolean {
            return context != null;
        }

        /** Starts the foreground service, with `text` in its notification. */
        public function startService(text:String):Boolean {
            return context != null && context.call("startService", text) === true;
        }

        public function stopService():Boolean {
            return context != null && context.call("stopService") === true;
        }

        /** Starts Turu's engine (once per process); returns its data folder. */
        public function startEngine():String {
            return context != null ? context.call("startEngine") as String : null;
        }

        /**
         * Client `client`'s turu.swf's `ExternalInterface.call(name, ...)` for the engine, its
         * arguments as a JSON array (-1: the host SWF's own). The engine's calls into turu.swf come as status events "call" (level: JSON with
         * `id`, `name` and `args`), answered with `reply`.
         */
        public function flashCall(name:String, args:String, client:int = -1):void {
            if (context != null) {
                context.call("flashCall", name, args, client);
            }
        }

        /** The answer to the engine's call `id`, and how long the callback took. */
        public function reply(id:Number, value:String, runMicros:int):void {
            if (context != null) {
                context.call("reply", id, value, runMicros);
            }
        }

        /** The SWF rewritten for AIR (`GAME` or `ASSET`), or null if it can't run there. */
        public function patchSwf(swf:ByteArray, target:int):ByteArray {
            return context != null ? context.call("patchSwf", swf, target) as ByteArray : null;
        }

        /**
         * Downloads `url` (a game file) through Turu's disk cache. Answers with a status event
         * "fetched", level `"<id>"` (then [fetchResult] has the bytes) or `"<id> <why>"`.
         */
        public function fetch(id:Number, url:String):Boolean {
            if (context == null) {
                return false;
            }
            context.call("fetch", id, url);
            return true;
        }

        /** The bytes of download `id`, once; null if there are none. */
        public function fetchResult(id:Number):ByteArray {
            return context != null ? context.call("fetchResult", id) as ByteArray : null;
        }

        /**
         * The game's focused text field for the strip above the soft keyboard, as JSON (`text`,
         * `caret`, `selStart`, `selEnd`, `password`, `max`), or "" when there's none.
         */
        public function gameTyping(json:String):void {
            if (context != null) {
                context.call("gameTyping", json);
            }
        }

        public static const GAME:int = 0;
        /** Another SWF the game loads (a map, an asset, the game menu). */
        public static const ASSET:int = 2;
        /** Or'd with a target: the game clients (tabs) share the stage. */
        public static const SHARED_STAGE:int = 0x10;
    }
}
