package {
    /** Client 2's skua.swf's `ExternalInterface` ([TuruExternal], `turu_air_patch::Options::client`). */
    public class TuruExternal2 {
        public static function get available():Boolean {
            return TuruExternal.available;
        }

        public static function addCallback(name:String, fn:Function):void {
            TuruExternal.addCallbackFor(2, name, fn);
        }

        public static function call(name:String, ...rest):* {
            return TuruExternal.callFrom(2, name, rest);
        }
    }
}
