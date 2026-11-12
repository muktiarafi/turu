package {
    /** Client 3's skua.swf's `ExternalInterface` ([TuruExternal], `turu_air_patch::Options::client`). */
    public class TuruExternal3 {
        public static function get available():Boolean {
            return TuruExternal.available;
        }

        public static function addCallback(name:String, fn:Function):void {
            TuruExternal.addCallbackFor(3, name, fn);
        }

        public static function call(name:String, ...rest):* {
            return TuruExternal.callFrom(3, name, rest);
        }
    }
}
