package {
    import com.muktiarafi.turu.air.TuruAirNative;
    import flash.desktop.NativeApplication;
    import flash.desktop.SystemIdleMode;
    import flash.display.DisplayObject;
    import flash.display.Loader;
    import flash.display.LoaderInfo;
    import flash.display.Sprite;
    import flash.display.StageAlign;
    import flash.display.StageScaleMode;
    import flash.events.MouseEvent;
    import flash.events.TimerEvent;
    import flash.text.TextFormat;
    import flash.utils.Timer;
    import flash.utils.getQualifiedClassName;
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
        private static const HOST_CLASSES:Array = [TuruExternal, TuruLoader, TuruStage, TuruExternalMute];

        /**
         * Spike (docs/AIR_ARMY_SPIKE.md): game clients on the one stage. The first is the engine's;
         * the others load after it, their skua talking to nobody ([TuruExternalMute]). Over 1, the
         * stage is shared ([TuruStage]) and a button at the top switches the client on screen.
         */
        private static const CLIENTS:int = 1;

        /** What [say] got before the engine started, sent to it once it has. */
        private var early:Array = [];
        private var engineStarted:Boolean = false;
        private var native:TuruAirNative;
        private var skua:Loader;
        /** Each client's skua loader, kept. */
        private var skuas:Array = [];
        /** skua.swf as read, patched once per client. */
        private var skuaBytes:ByteArray;
        /** Clients whose skua is loading or loaded. */
        private var started:int = 0;
        private var switcher:Sprite;
        private var switcherLabel:TextField;
        /** What was last sent of the game's focused text field (`gameTyping`). */
        private var typing:String = "";

        public function TuruAirHost() {
            mouseEnabled = false;
            mouseChildren = CLIENTS > 1;
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
            stage.addEventListener(Event.ADDED, keepGameFirst);
            if (CLIENTS > 1) {
                TuruStage.stage = stage;
                stage.addEventListener(Event.ADDED, watchClients);
                addSwitcher();
                var report:Timer = new Timer(30000);
                report.addEventListener(TimerEvent.TIMER, function(event:TimerEvent):void {
                    say("shared stage: " + TuruStage.games.length + " games, " + stage.numChildren +
                        " stage children, " + TuruStage.fallbacks + " rootOf fallbacks");
                });
                report.start();
            }
            addEventListener(Event.ENTER_FRAME, watchTyping);
        }

        /**
         * Keeps this host above whatever else is on the stage, so the game is stage child 0. The
         * game and skua find the game as `MovieClip(stage.getChildAt(0))` (CastBarMC, skua's
         * ModalMC, ...), and skua puts the game on the stage with `addChild` once it has removed
         * itself (child 0), which on its own would leave this host at 0 (a TypeError in the
         * game's setup, then every game response failing: no gear colours, no menus).
         */
        private function keepGameFirst(event:Event):void {
            if (event.target is DisplayObject && DisplayObject(event.target).parent == stage &&
                event.target != this && stage.getChildIndex(this) != stage.numChildren - 1) {
                stage.setChildIndex(this, stage.numChildren - 1);
            }
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
            skuaBytes = URLLoader(event.target).data;
            loadSkua();
        }

        /** The next client's skua.swf (the engine's first). */
        private function loadSkua():void {
            var client:int = started++;
            var target:int = TuruAirNative.SKUA;
            if (CLIENTS > 1) {
                target |= TuruAirNative.SHARED_STAGE;
                if (client > 0) {
                    target |= TuruAirNative.MUTE;
                }
            }
            var patched:ByteArray = native.patchSwf(skuaBytes, target);
            if (patched == null) {
                say("ERROR: skua.swf can't run under AIR");
                return;
            }
            say("loading client " + (client + 1) + " of " + CLIENTS);
            var context:LoaderContext = new LoaderContext(false, new ApplicationDomain(ApplicationDomain.currentDomain));
            context.allowCodeImport = true;
            skua = new Loader();
            skuas.push(skua);
            skua.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR, onUncaught);
            skua.contentLoaderInfo.addEventListener(Event.COMPLETE, onSkua);
            skua.contentLoaderInfo.addEventListener(IOErrorEvent.IO_ERROR, onFail);
            skua.loadBytes(patched, context);
        }

        /** skua at the stage's bottom: it replaces stage child 0 with the game once loaded. */
        private function onSkua(event:Event):void {
            var content:DisplayObject = LoaderInfo(event.target).content;
            if (CLIENTS > 1 && started > 1) {
                // Under the host, over the other clients' games, until it swaps itself for its game.
                stage.addChild(content);
            } else {
                stage.addChildAt(content, 0);
            }
            say("skua.swf loaded");
        }

        /**
         * Spike: a client's game reached the stage ([TuruLoader] listed it when it loaded). The
         * next client loads once the one before is on the stage.
         */
        private function watchClients(event:Event):void {
            var added:DisplayObject = event.target as DisplayObject;
            if (added == null || added.parent != stage || added == this) {
                return;
            }
            var client:int = TuruStage.games.indexOf(added);
            if (client < 0) {
                say("stage child: " + getQualifiedClassName(added) + " (" + stage.numChildren + " now)");
                return;
            }
            say("client " + (client + 1) + "'s game on the stage");
            // Its skua adds it before it's anyone's; settle the order once that's done.
            addEventListener(Event.ENTER_FRAME, function settle(event:Event):void {
                removeEventListener(Event.ENTER_FRAME, settle);
                TuruStage.activate(TuruStage.active);
                updateSwitcher();
            });
            if (client == started - 1 && started < CLIENTS) {
                loadSkua();
            }
        }

        private function addSwitcher():void {
            switcher = new Sprite();
            switcher.graphics.beginFill(0xD4A72C, 0.9);
            switcher.graphics.drawRoundRect(0, 0, 72, 26, 10, 10);
            switcher.graphics.endFill();
            switcher.x = 958 / 2 - 36;
            switcher.y = 2;
            switcher.buttonMode = true;
            switcherLabel = new TextField();
            switcherLabel.defaultTextFormat = new TextFormat("_sans", 14, 0x1A1400, true, null, null, null, null, "center");
            switcherLabel.width = 72;
            switcherLabel.height = 26;
            switcherLabel.y = 3;
            switcherLabel.selectable = false;
            switcherLabel.mouseEnabled = false;
            switcher.addChild(switcherLabel);
            switcher.addEventListener(MouseEvent.CLICK, function(event:MouseEvent):void {
                if (TuruStage.games.length < 2) {
                    return;
                }
                TuruStage.activate((TuruStage.active + 1) % TuruStage.games.length);
                updateSwitcher();
                say("client " + (TuruStage.active + 1) + " on screen; " + TuruStage.fallbacks + " rootOf fallbacks");
            });
            addChild(switcher);
            updateSwitcher();
        }

        private function updateSwitcher():void {
            if (switcherLabel != null) {
                switcherLabel.text = "Client " + (TuruStage.active + 1) + "/" + Math.max(1, TuruStage.games.length);
            }
        }
    }
}
