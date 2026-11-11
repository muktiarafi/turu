package com.muktiarafi.turu.air {
    import flash.events.EventDispatcher;
    import flash.events.StatusEvent;
    import flash.external.ExtensionContext;

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

        /**
         * Times `calls` round trips Rust -> AS3 -> Rust. Each comes as a status event "call" (level:
         * the id) to answer with `reply`; the summary as "done".
         */
        public function bench(calls:int):void {
            if (context != null) {
                context.call("bench", calls);
            }
        }

        public function reply(id:int, value:String):void {
            if (context != null) {
                context.call("reply", id, value);
            }
        }
    }
}
