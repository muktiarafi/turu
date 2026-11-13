package {
    import flash.display.DisplayObject;
    import flash.display.Loader;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;

    /**
     * What Turu's own host SWF (`turu.swf`, `host/`; its `AirHost`) asks of Turu Mas, for its tab
     * `client` (docs/HOST_SWF_PROPOSAL.md, R4). `turu.swf` finds this class by name in its
     * parent domain, so one unpatched copy serves every tab: VibeSkua's SWF needs a patched copy
     * per tab instead (`TuruExternal<n>`, `TuruLoader<n>`).
     */
    public class TuruHost {
        /** Client `client`'s callback `name`, which the engine's calls reach. */
        public static function addCallback(client:int, name:String, fn:Function):void {
            TuruExternal.addCallbackFor(client, name, fn);
        }

        /**
         * Tells client `client`'s bot of an event. `args` go as one array, as `ExternalInterface`
         * sends them on the Ruffle build (`events.rs` unwraps it).
         */
        public static function call(client:int, name:String, args:Array):void {
            TuruExternal.callFrom(client, name, [args]);
        }

        /**
         * A loader for client `client`, loading `url` as [TuruLoader] does: through Turu's disk
         * cache, the game's client patched for AIR and known as this client's; into `domain`
         * when given (turu.swf passes one under its own, so [TuruStage] finds the tab).
         */
        public static function load(client:int, url:String, domain:ApplicationDomain = null):Loader {
            var loader:TuruLoader = new TuruLoader(client);
            loader.load(new URLRequest(url), domain != null ? new LoaderContext(false, domain) : null);
            return loader;
        }

        /**
         * Client `client`'s game takes its host SWF's (`main`) place on the stage; the host
         * places it and shows the client on screen ([TuruStage], `TuruAirHost.watchGames`).
         */
        public static function showGame(client:int, main:DisplayObject, game:DisplayObject):void {
            TuruStage.removeOwn(main);
            TuruStage.stage.addChild(game);
        }

        /** Whether client `client`'s tab is the one on screen. */
        public static function isShown(client:int):Boolean {
            return TuruStage.active == client;
        }
    }
}
