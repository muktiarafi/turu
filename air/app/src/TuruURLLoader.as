package {
    import com.muktiarafi.turu.air.TuruAirNative;
    import flash.events.Event;
    import flash.events.IOErrorEvent;
    import flash.events.ProgressEvent;
    import flash.events.StatusEvent;
    import flash.net.URLLoader;
    import flash.net.URLLoaderDataFormat;
    import flash.net.URLRequest;
    import flash.net.URLRequestMethod;
    import flash.utils.ByteArray;

    /**
     * A `URLLoader` that fetches the game's files through Turu's disk cache: AIR on Android keeps
     * none, so without it every start downloaded the game again (15 MB) and every map join its
     * map, monsters and gear. `turu_air_patch` makes the client's `AirLoader` build these, and
     * [TuruLoader] uses one too.
     *
     * Only a binary `GET` of a file under `/gamefiles/` goes to the engine
     * (`turu_frontend::game_files::fetch`, AQW's cache rules); anything else, and anything the
     * engine couldn't fetch, is a plain `URLLoader`'s download. The events are a `URLLoader`'s:
     * `open`, one `progress`, then `complete` or `ioError`, after `load` returns.
     */
    public class TuruURLLoader extends URLLoader {
        /** Loaders waiting for the engine, by download id. */
        private static var waiting:Object = {};
        private static var nextId:Number = 1;
        private static var listening:Boolean = false;

        /** This loader's download, or 0. */
        private var id:Number = 0;
        private var request:URLRequest;

        public function TuruURLLoader(request:URLRequest = null) {
            super(null);
            if (request != null) {
                load(request);
            }
        }

        override public function load(request:URLRequest):void {
            forget();
            var native:TuruAirNative = TuruExternal.native;
            if (native == null || !cached(request)) {
                super.load(request);
                return;
            }
            if (!listening) {
                listening = true;
                native.addEventListener(StatusEvent.STATUS, onStatus);
            }
            this.request = request;
            id = nextId++;
            waiting[id] = this;
            if (!native.fetch(id, request.url)) {
                forget();
                super.load(request);
            }
        }

        override public function close():void {
            if (id != 0) {
                forget();
                return;
            }
            super.close();
        }

        /** What the engine's cache may answer: a file of the game's, asked for as bytes. */
        private function cached(request:URLRequest):Boolean {
            var url:String = request.url;
            return dataFormat == URLLoaderDataFormat.BINARY &&
                (request.method == null || request.method == URLRequestMethod.GET) &&
                request.data == null &&
                (request.requestHeaders == null || request.requestHeaders.length == 0) &&
                url != null && /^https?:\/\/[^\/]+\/.*\/gamefiles\//i.test(url);
        }

        private function forget():void {
            if (id != 0) {
                delete waiting[id];
                id = 0;
            }
            request = null;
        }

        private static function onStatus(event:StatusEvent):void {
            if (event.code != "fetched") {
                return;
            }
            var space:int = event.level.indexOf(" ");
            var id:Number = Number(space < 0 ? event.level : event.level.substr(0, space));
            var error:String = space < 0 ? null : event.level.substr(space + 1);
            var loader:TuruURLLoader = waiting[id] as TuruURLLoader;
            // Taken even when nobody waits any more (closed), so the engine lets it go.
            var bytes:ByteArray = error == null ? TuruExternal.native.fetchResult(id) : null;
            if (loader == null || loader.id != id) {
                return;
            }
            var request:URLRequest = loader.request;
            loader.forget();
            if (bytes == null) {
                // The engine couldn't: AIR's own download, as before Turu cached anything.
                trace("[TuruURLLoader] " + (error != null ? error : "no bytes") + "; AIR downloads " + request.url);
                loader.download(request);
                return;
            }
            loader.data = bytes;
            loader.bytesLoaded = bytes.length;
            loader.bytesTotal = bytes.length;
            loader.dispatchEvent(new Event(Event.OPEN));
            loader.dispatchEvent(new ProgressEvent(ProgressEvent.PROGRESS, false, false, bytes.length, bytes.length));
            loader.dispatchEvent(new Event(Event.COMPLETE));
        }

        private function download(request:URLRequest):void {
            try {
                super.load(request);
            } catch (e:Error) {
                dispatchEvent(new IOErrorEvent(IOErrorEvent.IO_ERROR, false, false, e.message, e.errorID));
            }
        }
    }
}
