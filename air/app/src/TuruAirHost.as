package {
    import com.muktiarafi.turu.air.TuruAirNative;
    import flash.desktop.NativeApplication;
    import flash.desktop.SystemIdleMode;
    import flash.display.DisplayObject;
    import flash.display.Loader;
    import flash.display.Sprite;
    import flash.display.StageAlign;
    import flash.display.StageScaleMode;
    import flash.events.ErrorEvent;
    import flash.events.Event;
    import flash.events.IOErrorEvent;
    import flash.events.UncaughtErrorEvent;
    import flash.media.SoundMixer;
    import flash.media.SoundTransform;
    import flash.text.TextFieldType;
    import flash.net.URLLoader;
    import flash.net.URLLoaderDataFormat;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.system.Security;
    import flash.text.TextField;
    import flash.text.TextFormat;
    import flash.utils.ByteArray;
    import flash.utils.setTimeout;

    /**
     * Turu's host under Adobe AIR (docs/AIR_PROPOSAL.md): starts the foreground service and
     * Turu's engine (the native extension), then loads VibeSkua's skua.swf as the Ruffle build
     * does, patched by the engine for AIR (`turu_air_patch`): its `ExternalInterface` is
     * [TuruExternal] and its loaders [TuruLoader], so the engine drives it as it drives skua in
     * Ruffle. A log strip at the bottom shows what happened at the start (release builds' trace goes
     * nowhere), and errors after. Turu's windows are drawn over all this by the engine (the
     * extension's overlay view).
     *
     * [TuruExternal] and [TuruLoader] are compiled into this SWF; skua, in a child application
     * domain, finds them by name.
     */
    [SWF(frameRate="24", backgroundColor="#000000", width="958", height="550")]
    public class TuruAirHost extends Sprite {
        private static const URL:String = "https://game.aq.com/game/";

        // Compiled in for skua (found by name, never named here otherwise).
        private static const HOST_CLASSES:Array = [TuruExternal, TuruLoader];

        private var log:TextField;
        private var native:TuruAirNative;
        private var skua:Loader;
        /** What was last sent of the game's focused text field (`gameTyping`). */
        private var typing:String = "";

        public function TuruAirHost() {
            mouseEnabled = false;
            mouseChildren = false;
            log = new TextField();
            log.defaultTextFormat = new TextFormat("_sans", 11, 0x00ff00);
            log.background = true;
            log.backgroundColor = 0x000000;
            log.alpha = 0.8;
            log.width = 958;
            log.height = 90;
            log.y = 550 - 90;
            log.multiline = true;
            log.wordWrap = true;
            addChild(log);
            say("sandbox " + Security.sandboxType);
            loaderInfo.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR, onUncaught);

            // Keep running off screen (AIR suspends a deactivated app otherwise; the service keeps
            // Android from freezing the process).
            NativeApplication.nativeApplication.executeInBackground = true;
            NativeApplication.nativeApplication.systemIdleMode = SystemIdleMode.KEEP_AWAKE;
            NativeApplication.nativeApplication.addEventListener(Event.ACTIVATE, onActivity);
            NativeApplication.nativeApplication.addEventListener(Event.DEACTIVATE, onActivity);

            native = new TuruAirNative();
            if (!native.available) {
                say("ERROR: Turu's native extension is missing");
                return;
            }
            say("service started: " + native.startService("Running"));
            TuruExternal.start(native);
            TuruLoader.gameParams = {sFilePath: URL + "gamefiles/", isWeb: false};
            say("engine started, data folder " + native.startEngine());
            // The main SWF is on the stage already; added later only when loaded by another.
            if (stage != null) {
                onStage(null);
            } else {
                addEventListener(Event.ADDED_TO_STAGE, onStage);
            }

            var bytes:URLLoader = new URLLoader();
            bytes.dataFormat = URLLoaderDataFormat.BINARY;
            bytes.addEventListener(Event.COMPLETE, onSkuaBytes);
            bytes.addEventListener(IOErrorEvent.IO_ERROR, onFail);
            bytes.load(new URLRequest("app:/skua.swf"));
        }

        private function onStage(event:Event = null):void {
            stage.scaleMode = StageScaleMode.SHOW_ALL;
            stage.align = StageAlign.TOP;
            // Stay above what skua puts on the stage (it swaps itself for the game).
            stage.addEventListener(Event.ADDED, keepOnTop);
            addEventListener(Event.ENTER_FRAME, watchTyping);
        }

        private function keepOnTop(event:Event):void {
            var added:DisplayObject = event.target as DisplayObject;
            if (added != null && added.parent == stage && stage.getChildIndex(this) != stage.numChildren - 1) {
                stage.setChildIndex(this, stage.numChildren - 1);
            }
        }

        private function say(text:String):void {
            trace("[TuruAir] " + text);
            if (text.indexOf("ERROR") == 0 || text.indexOf("UNCAUGHT") == 0) {
                log.visible = true;
            }
            log.appendText(text + "\n");
            log.scrollV = log.maxScrollV;
        }

        /** Silent off screen, as Turu's Ruffle build (the game keeps running there). */
        private function onActivity(event:Event):void {
            say(new Date().toTimeString().substr(0, 8) + " " + event.type);
            SoundMixer.soundTransform = new SoundTransform(event.type == Event.ACTIVATE ? 1 : 0);
        }

        /**
         * The game's focused text field, for the engine's strip above the soft keyboard (the
         * keyboard covers the field; AIR doesn't pan the stage: `softKeyboardBehavior` none).
         */
        private function watchTyping(event:Event):void {
            var field:TextField = stage.focus as TextField;
            var now:String = "";
            if (field != null && field.type == TextFieldType.INPUT) {
                now = JSON.stringify({
                    text: field.text,
                    caret: field.caretIndex,
                    selStart: field.selectionBeginIndex,
                    selEnd: field.selectionEndIndex,
                    password: field.displayAsPassword,
                    max: field.maxChars
                });
            }
            if (now != typing) {
                typing = now;
                native.gameTyping(now);
            }
        }

        private function onFail(event:ErrorEvent):void {
            say("ERROR " + event.type + ": " + event.text);
        }

        private function onUncaught(event:UncaughtErrorEvent):void {
            var error:* = event.error;
            say("UNCAUGHT " + (error is Error ? Error(error).errorID + " " + Error(error).message + "\n" + Error(error).getStackTrace() : String(error)));
        }

        private function onSkuaBytes(event:Event):void {
            var patched:ByteArray = native.patchSwf(URLLoader(event.target).data, TuruAirNative.SKUA);
            if (patched == null) {
                say("ERROR: skua.swf can't run under AIR (see logcat, tag turu)");
                return;
            }
            var context:LoaderContext = new LoaderContext(false, new ApplicationDomain(ApplicationDomain.currentDomain));
            context.allowCodeImport = true;
            skua = new Loader();
            skua.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR, onUncaught);
            skua.contentLoaderInfo.addEventListener(Event.COMPLETE, onSkua);
            skua.contentLoaderInfo.addEventListener(IOErrorEvent.IO_ERROR, onFail);
            skua.loadBytes(patched, context);
        }

        /** skua at the stage's bottom: it replaces stage child 0 with the game once loaded. */
        private function onSkua(event:Event):void {
            stage.addChildAt(skua.content, 0);
            say("skua.swf loaded");
            // Turu's windows take over (the engine's overlay): the strip would cover the game's bar.
            setTimeout(function():void {
                log.visible = false;
            }, 20000);
        }
    }
}
