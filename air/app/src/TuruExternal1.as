package {
    /** Client 1's skua.swf's `ExternalInterface` ([TuruExternal], `turu_air_patch::Options::client`). */
    public class TuruExternal1 {
        public static function get available():Boolean {
            return TuruExternal.available;
        }

        public static function addCallback(name:String, fn:Function):void {
            TuruExternal.addCallbackFor(1, name, fn);
        }

        public static function call(name:String, ...rest):* {
            return TuruExternal.callFrom(1, name, rest);
        }
    }
}
