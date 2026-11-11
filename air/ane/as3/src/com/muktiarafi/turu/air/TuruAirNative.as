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
         * skua.swf's `ExternalInterface.call(name, ...)` for the engine, its arguments as a JSON
         * array. The engine's calls into skua come as status events "call" (level: JSON with
         * `id`, `name` and `args`), answered with `reply`.
         */
        public function flashCall(name:String, args:String):void {
            if (context != null) {
                context.call("flashCall", name, args);
            }
        }

        /** The answer to the engine's call `id`, and how long the callback took. */
        public function reply(id:Number, value:String, runMicros:int):void {
            if (context != null) {
                context.call("reply", id, value, runMicros);
            }
        }

        /** The SWF rewritten for AIR (`GAME`, `SKUA` or `ASSET`), or null if it can't run there. */
        public function patchSwf(swf:ByteArray, target:int):ByteArray {
            return context != null ? context.call("patchSwf", swf, target) as ByteArray : null;
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
        public static const SKUA:int = 1;
        /** Another SWF the game or skua loads (a background, an injected script). */
        public static const ASSET:int = 2;
    }
}
