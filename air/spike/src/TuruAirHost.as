package {
    import com.muktiarafi.turu.air.TuruAirNative;
    import flash.desktop.NativeApplication;
    import flash.desktop.SystemIdleMode;
    import flash.display.Loader;
    import flash.display.LoaderInfo;
    import flash.display.Sprite;
    import flash.display.StageAlign;
    import flash.display.StageScaleMode;
    import flash.events.ErrorEvent;
    import flash.events.Event;
    import flash.events.IOErrorEvent;
    import flash.events.SecurityErrorEvent;
    import flash.events.StatusEvent;
    import flash.events.UncaughtErrorEvent;
    import flash.net.URLLoader;
    import flash.net.URLLoaderDataFormat;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.system.Security;
    import flash.text.TextField;
    import flash.text.TextFormat;
    import flash.utils.ByteArray;
    import flash.utils.getTimer;
    import flash.utils.setTimeout;

    /**
     * Spike A0 (docs/AIR_PROPOSAL.md): loads AQW's client, patched by `turu_air_patch` and
     * bundled as app:/Game-air.swf, the way skua-air.swf would: `loadBytes` with
     * `allowCodeImport`, params set before the game reaches the stage. No bot yet: the bridge
     * benchmark (Rust -> AS3 -> Rust through the extension) runs once the game is up, and again
     * (shorter) each time the app goes to the background.
     * A log strip at the bottom shows what happened (release builds' trace goes nowhere).
     */
    [SWF(frameRate="24", backgroundColor="#000000", width="958", height="550")]
    public class TuruAirHost extends Sprite {
        private static const URL:String = "https://game.aq.com/game/";

        private var log:TextField;
        private var vars:Object;
        private var loader:Loader;
        private var started:int;
        private var native:TuruAirNative;
        private var game:*;

        public function TuruAirHost() {
            log = new TextField();
            log.defaultTextFormat = new TextFormat("_sans", 11, 0x00ff00);
            log.background = true;
            log.backgroundColor = 0x000000;
            log.alpha = 0.8;
            log.mouseEnabled = false;
            log.width = 958;
            log.height = 110;
            log.y = 550 - 110;
            log.multiline = true;
            log.wordWrap = true;
            addChild(log);
            say("sandbox " + Security.sandboxType);
            // Keep running off screen (AIR suspends a deactivated app otherwise).
            NativeApplication.nativeApplication.executeInBackground = true;
            NativeApplication.nativeApplication.systemIdleMode = SystemIdleMode.KEEP_AWAKE;
            native = new TuruAirNative();
            native.addEventListener(StatusEvent.STATUS, onNative);
            say("native extension " + (native.available ? "loaded" : "missing") + ", service started: " + native.startService("Spike: AQW under AIR"));
            NativeApplication.nativeApplication.addEventListener(Event.ACTIVATE, onActivity);
            NativeApplication.nativeApplication.addEventListener(Event.DEACTIVATE, onActivity);
            loaderInfo.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR, onUncaught);

            var version:URLLoader = new URLLoader();
            version.addEventListener(Event.COMPLETE, onVersion);
            version.addEventListener(IOErrorEvent.IO_ERROR, onFail);
            version.addEventListener(SecurityErrorEvent.SECURITY_ERROR, onFail);
            version.load(new URLRequest(URL + "api/data/gameversion"));
        }

        private function say(text:String):void {
            trace("[TuruAir] " + text);
            log.appendText(text + "\n");
            log.scrollV = log.maxScrollV;
        }

        private function onActivity(event:Event):void {
            say(new Date().toTimeString().substr(0, 8) + " " + event.type);
            if (event.type == Event.DEACTIVATE && game != null) {
                say("bench in the background: 100 calls");
                native.bench(100);
            }
        }

        /** The bridge: a call to answer from the game (as getGameObject would), or a summary. */
        private function onNative(event:StatusEvent):void {
            if (event.code == "call") {
                var value:String = JSON.stringify({
                    world: game != null && game.world != null,
                    connected: game != null && game.sfc != null && game.sfc.isConnected
                });
                native.reply(int(event.level), value);
            } else if (event.code == "done") {
                say(new Date().toTimeString().substr(0, 8) + " bench (" + stage.frameRate + " fps): " + event.level);
            } else {
                say("native " + event.code + ": " + event.level);
            }
        }

        private function onFail(event:ErrorEvent):void {
            say("ERROR " + event.type + ": " + event.text);
        }

        private function onUncaught(event:UncaughtErrorEvent):void {
            var error:* = event.error;
            say("UNCAUGHT " + (error is Error ? Error(error).errorID + " " + Error(error).message + "\n" + Error(error).getStackTrace() : String(error)));
        }

        private function onVersion(event:Event):void {
            vars = JSON.parse(URLLoader(event.target).data);
            say("live client " + vars.sFile + " (" + vars.sVersion + "), loading app:/Game-air.swf");
            var bytes:URLLoader = new URLLoader();
            bytes.dataFormat = URLLoaderDataFormat.BINARY;
            bytes.addEventListener(Event.COMPLETE, onBytes);
            bytes.addEventListener(IOErrorEvent.IO_ERROR, onFail);
            bytes.load(new URLRequest("app:/Game-air.swf"));
        }

        private function onBytes(event:Event):void {
            var data:ByteArray = URLLoader(event.target).data;
            say("patched client: " + data.length + " bytes");
            var context:LoaderContext = new LoaderContext(false, new ApplicationDomain(ApplicationDomain.currentDomain));
            context.allowCodeImport = true;
            loader = new Loader();
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR, onUncaught);
            loader.contentLoaderInfo.addEventListener(Event.INIT, onGame);
            loader.contentLoaderInfo.addEventListener(IOErrorEvent.IO_ERROR, onFail);
            started = getTimer();
            loader.loadBytes(data, context);
        }

        private function onGame(event:Event):void {
            say("client constructed in " + (getTimer() - started) + " ms");
            var game:* = LoaderInfo(event.target).content;
            // Before the stage: Game.onAddedToStage reads sFilePath (else loaderInfo.url, app:/).
            game.params.vars = vars;
            game.params.sURL = URL;
            game.params.sFilePath = URL + "gamefiles/";
            game.params.sBG = vars.sBG;
            game.params.sTitle = vars.sTitle;
            game.params.isEU = false;
            game.params.isWeb = false;
            game.params.loginURL = URL + "api/login/now";
            stage.scaleMode = StageScaleMode.SHOW_ALL;
            stage.align = StageAlign.TOP;
            stage.addChildAt(game, 0);
            this.game = game;
            setTimeout(function():void {
                say("bench: 1000 calls");
                native.bench(1000);
            }, 8000);
            say("client on stage: serverFilePath " + game.constructor.serverFilePath + ", serverGamePath " + game.constructor.serverGamePath);
        }
    }
}
