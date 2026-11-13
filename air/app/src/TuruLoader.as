package {
    import com.muktiarafi.turu.air.TuruAirNative;
    import flash.display.Loader;
    import flash.events.ErrorEvent;
    import flash.events.Event;
    import flash.events.IOErrorEvent;
    import flash.events.SecurityErrorEvent;
    import flash.net.URLLoader;
    import flash.net.URLLoaderDataFormat;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.utils.ByteArray;

    /**
     * skua.swf's `Loader` under AIR (`turu_air_patch` makes each of skua's `new Loader()` one of
     * these). A SWF loaded from the web would land in a remote sandbox, where skua can't reach
     * the game's classes, so this fetches the bytes, has the engine patch a SWF for AIR, and loads
     * it with `loadBytes` and `allowCodeImport`, as the client's own `AirLoader` does. Before
     * the game's client reaches the stage (`init`, before skua's `complete`), its `params` get
     * [gameParams]: the client reads `sFilePath` when it's added to the stage.
     */
    public class TuruLoader extends Loader {
        /** What the client's `params` get (the host sets them). */
        public static var gameParams:Object = {};

        private var url:String;
        /** The game client whose skua made this loader ([TuruLoader0] ...), or -1. */
        protected var client:int = -1;

        /** `forClient`: the client it loads for (`TuruHost.load`); -1 for skua's own. */
        public function TuruLoader(forClient:int = -1) {
            super();
            client = forClient;
            contentLoaderInfo.addEventListener(Event.INIT, onInit, false, int.MAX_VALUE);
        }

        override public function load(request:URLRequest, context:LoaderContext = null):void {
            url = request.url;
            // Through Turu's disk cache (the game's client, ~3 MB, at every start otherwise).
            var fetch:URLLoader = new TuruURLLoader();
            fetch.dataFormat = URLLoaderDataFormat.BINARY;
            fetch.addEventListener(Event.COMPLETE, function(event:Event):void {
                var bytes:ByteArray = fetch.data;
                if (isSwf(bytes)) {
                    var target:int = isClient() ? TuruAirNative.GAME : TuruAirNative.ASSET;
                    if (TuruStage.stage != null) {
                        target |= TuruAirNative.SHARED_STAGE;
                    }
                    var patched:ByteArray = TuruExternal.native != null ? TuruExternal.native.patchSwf(bytes, target) : null;
                    if (patched == null) {
                        fail("can't run under AIR: " + url);
                        return;
                    }
                    bytes = patched;
                }
                var loading:LoaderContext = context != null ? context : new LoaderContext(false, new ApplicationDomain(ApplicationDomain.currentDomain));
                loading.allowCodeImport = true;
                if (isClient() && client >= 0) {
                    // Known as this client's from the start: the game's constructor already
                    // plays sounds and adds listeners (TuruStage.clientOfDomain).
                    TuruStage.gameLoaders[client] = this;
                }
                loadBytes(bytes, loading);
            });
            fetch.addEventListener(IOErrorEvent.IO_ERROR, function(event:ErrorEvent):void {
                fail(event.text);
            });
            fetch.addEventListener(SecurityErrorEvent.SECURITY_ERROR, function(event:ErrorEvent):void {
                fail(event.text);
            });
            fetch.load(request);
        }

        /** AQW's client: gamefiles/Game<version>.swf. */
        private function isClient():Boolean {
            return url != null && /\/gamefiles\/Game[^\/]*\.swf/i.test(url);
        }

        private static function isSwf(bytes:ByteArray):Boolean {
            if (bytes == null || bytes.length < 3) {
                return false;
            }
            var signature:String = String.fromCharCode(bytes[0], bytes[1], bytes[2]);
            return signature == "FWS" || signature == "CWS" || signature == "ZWS";
        }

        private function onInit(event:Event):void {
            if (!isClient()) {
                return;
            }
            if (client >= 0) {
                TuruStage.setGame(client, content, this);
            }
            try {
                var params:Object = Object(content).params;
                for (var key:String in gameParams) {
                    params[key] = gameParams[key];
                }
            } catch (e:Error) {
            }
        }

        private function fail(text:String):void {
            trace("[TuruLoader] " + text);
            contentLoaderInfo.dispatchEvent(new IOErrorEvent(IOErrorEvent.IO_ERROR, false, false, text));
        }
    }
}
