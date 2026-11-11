package {
    import com.muktiarafi.turu.air.TuruAirNative;
    import flash.desktop.NativeApplication;
    import flash.desktop.SystemIdleMode;
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
    import flash.utils.ByteArray;

    /**
     * Turu's host under Adobe AIR (docs/AIR_PROPOSAL.md): starts the foreground service and
     * Turu's engine (the native extension), then loads VibeSkua's skua.swf as the Ruffle build
     * does, patched by the engine for AIR (`turu_air_patch`): its `ExternalInterface` is
     * [TuruExternal] and its loaders [TuruLoader], so the engine drives it as it drives skua in
     * Ruffle. It draws nothing itself: its start and its errors go to Turu's Debug log ([say]), and
     * Turu's windows are drawn over the game by the engine (the extension's overlay view).
     *
     * [TuruExternal] and [TuruLoader] are compiled into this SWF; skua, in a child application
     * domain, finds them by name.
     */
    [SWF(frameRate="24", backgroundColor="#000000", width="958", height="550")]
    public class TuruAirHost extends Sprite {
        private static const URL:String = "https://game.aq.com/game/";

        // Compiled in for skua (found by name, never named here otherwise).
        private static const HOST_CLASSES:Array = [TuruExternal, TuruLoader];

        /** What [say] got before the engine started, sent to it once it has. */
        private var early:Array = [];
        private var engineStarted:Boolean = false;
        private var native:TuruAirNative;
        private var skua:Loader;
        /** What was last sent of the game's focused text field (`gameTyping`). */
        private var typing:String = "";

        public function TuruAirHost() {
            mouseEnabled = false;
            mouseChildren = false;
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
            var folder:String = native.startEngine();
            engineStarted = true;
            for each (var line:String in early) {
                say(line);
            }
            early = null;
            say("engine started, data folder " + folder);
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
            addEventListener(Event.ENTER_FRAME, watchTyping);
        }

        /**
         * A line for Turu's Debug log (Diagnostics > Logs) and logcat (tag turu), through the engine
         * (`hostLog`): release builds' trace goes nowhere. Kept until the engine has started.
         */
        private function say(text:String):void {
            trace("[TuruAir] " + text);
            if (!engineStarted) {
                early.push(text);
                return;
            }
            native.flashCall("hostLog", JSON.stringify([text]));
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
                say("ERROR: skua.swf can't run under AIR");
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
        }
    }
}
