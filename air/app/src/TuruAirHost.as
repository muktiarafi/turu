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
    import flash.events.ErrorEvent;
    import flash.events.Event;
    import flash.events.IOErrorEvent;
    import flash.events.KeyboardEvent;
    import flash.events.StatusEvent;
    import flash.events.TimerEvent;
    import flash.events.UncaughtErrorEvent;
    import flash.media.SoundMixer;
    import flash.media.SoundTransform;
    import flash.net.URLLoader;
    import flash.net.URLLoaderDataFormat;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.system.Security;
    import flash.text.TextField;
    import flash.text.TextFieldType;
    import flash.ui.Keyboard;
    import flash.utils.ByteArray;
    import flash.utils.Timer;
    import flash.utils.getQualifiedClassName;

    /**
     * Turu's host under Adobe AIR (docs/AIR_PROPOSAL.md): starts the foreground service and
     * Turu's engine (the native extension), then loads Turu's host SWF, turu.swf, as the Ruffle
     * build does, once per tab, as it is (it reaches this host through [TuruHost]). It draws nothing
     * itself: its start and its errors go to Turu's Debug log ([say]), and Turu's windows are drawn
     * over the game by the engine (the extension's overlay view).
     *
     * Tabs (docs/AIR_ARMY_SPIKE.md): each tab is a game client, numbered 0 to 3, all on AIR's one
     * stage ([TuruStage]). Client `n`'s turu.swf gets `n` in its `LoaderContext`'s parameters
     * (`turuClient`) and names it to [TuruHost], so its calls reach its tab's bot and its game is
     * known as its own. Client 0 loads at start; the engine opens, shows and closes the others (status events "tab",
     * `air/ane/rust/src/clients.rs`).
     *
     * The host classes are compiled into this SWF; turu.swf, in a child application domain, finds
     * them by name.
     */
    [SWF(frameRate="24", backgroundColor="#000000", width="958", height="550")]
    public class TuruAirHost extends Sprite {
        private static const URL:String = "https://game.aq.com/game/";

        // Compiled in for turu.swf (found by name, never named here otherwise).
        private static const HOST_CLASSES:Array = [TuruExternal, TuruLoader, TuruStage, TuruHost];

        /** What [say] got before the engine started, sent to it once it has. */
        private var early:Array = [];
        private var engineStarted:Boolean = false;
        private var native:TuruAirNative;
        /** Each client's turu.swf loader, while it's open. */
        private var hosts:Array = [];
        /** turu.swf as read, the same for every client; null until read. */
        private var hostBytes:ByteArray;
        /** Clients the engine opened before turu.swf was read. */
        private var waiting:Array = [];
        /** What was last sent of the game's focused text field (`gameTyping`). */
        private var typing:String = "";
        /** [TuruStage.hookSummary] when last logged. */
        private var hooksSaid:String = "";
        /** [TuruStage.fallbacks] when last logged. */
        private var fallbacksSaid:int = 0;
        /** AIR's view's size in pixels (status event "layout"), until the stage is there. */
        private var viewSize:Array;
        /** [fit] is reading AIR's own size. */
        private var measuring:Boolean = false;

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
            // Before the engine starts: it may reopen tabs at once.
            native.addEventListener(StatusEvent.STATUS, onTab);
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
            bytes.addEventListener(Event.COMPLETE, onHostBytes);
            bytes.addEventListener(IOErrorEvent.IO_ERROR, onFail);
            bytes.load(new URLRequest("app:/turu.swf"));
        }

        private function onStage(event:Event = null):void {
            stage.scaleMode = StageScaleMode.SHOW_ALL;
            stage.align = StageAlign.TOP;
            TuruStage.stage = stage;
            stage.addEventListener(Event.ADDED, keepHostOnTop);
            stage.addEventListener(Event.ADDED, watchGames);
            addEventListener(Event.ENTER_FRAME, watchTyping);
            // [fit]'s measuring resize isn't the games' to see.
            stage.addEventListener(Event.RESIZE, function(event:Event):void {
                if (measuring) {
                    event.stopImmediatePropagation();
                }
            }, false, int.MAX_VALUE);
            if (viewSize != null) {
                fit(viewSize[0], viewSize[1]);
            }
            addEventListener(Event.ENTER_FRAME, function(event:Event):void {
                TuruStage.keepFitted();
            });
            // A `stage.getChildAt(0)` that couldn't tell whose game it meant is a bug to look at;
            // what the shared stage's hooks did (keys held back, sounds silenced, popups) shows
            // they work.
            var check:Timer = new Timer(60000);
            check.addEventListener(TimerEvent.TIMER, function(event:TimerEvent):void {
                if (TuruStage.fallbacks != fallbacksSaid) {
                    fallbacksSaid = TuruStage.fallbacks;
                    say("tabs: " + fallbacksSaid + " stage.getChildAt(0) calls couldn't tell their tab");
                }
                var hooks:String = TuruStage.hookSummary();
                if (hooks != hooksSaid) {
                    hooksSaid = hooks;
                    say("tabs: " + hooks);
                }
                while (TuruStage.lostDomains.length > 0) {
                    say("tabs: a domain without its tab: " + TuruStage.lostDomains.shift());
                }
            });
            check.start();
        }

        /**
         * Keeps this host above whatever else is on the stage. The games stay below it: the one on
         * screen at child 0 ([TuruStage.activate]), for any `stage.getChildAt(0)` left unpatched.
         */
        private function keepHostOnTop(event:Event):void {
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

        /**
         * Silent off screen, as Turu's Ruffle build (the game keeps running there). Back on screen,
         * the tab's game is redrawn ([redrawShown]).
         */
        private function onActivity(event:Event):void {
            say(new Date().toTimeString().substr(0, 8) + " " + event.type);
            SoundMixer.soundTransform = new SoundTransform(event.type == Event.ACTIVATE ? 1 : 0);
            dumpStage(event.type);
            if (event.type == Event.ACTIVATE) {
                redrawShown();
            }
        }

        /**
         * With several tabs, the game on screen stayed black after the app came back (Home and
         * back, a file dialog's picker), its display list just as before, until another tab was
         * picked and then it again: likely AIR not drawing the scaled game anew. So a few frames
         * after the app is back (its surface may come after the event), what picking the tab shows
         * (its game and its popups' layer, or its host SWF while the game loads) is hidden for a
         * frame and shown again, the tab on screen staying the same.
         */
        private function redrawShown():void {
            var frames:int = 0;
            addEventListener(Event.ENTER_FRAME, function later(e:Event):void {
                frames++;
                if (frames == 3) {
                    var client:int = TuruStage.active;
                    for each (var shown:DisplayObject in [TuruStage.games[client], TuruStage.overlays[client],
                            TuruStage.mains[client]]) {
                        if (shown != null && shown.parent != null) {
                            shown.visible = false;
                        }
                    }
                } else if (frames == 4) {
                    removeEventListener(Event.ENTER_FRAME, later);
                    TuruStage.activate(TuruStage.active);
                    dumpStage("redrawn");
                }
            });
        }

        /**
         * The stage's children as the Debug log's line `stage <when>: ...`: whose each is (game,
         * host SWF or overlay of client n), its visibility, alpha, scale, place and size, with the
         * fit and the stage's size: what a tab that stays black after the app comes back differs in.
         */
        private function dumpStage(when:String):void {
            if (stage == null) {
                return;
            }
            var parts:Array = [];
            for (var i:int = 0; i < stage.numChildren; i++) {
                var d:DisplayObject = stage.getChildAt(i);
                var who:String = d == this ? "host" : "?";
                var n:int = TuruStage.games.indexOf(d);
                if (n >= 0) {
                    who = "game" + n;
                } else if ((n = TuruStage.mains.indexOf(d)) >= 0) {
                    who = "main" + n;
                } else if ((n = TuruStage.overlays.indexOf(d)) >= 0) {
                    who = "layer" + n;
                }
                parts.push(i + ":" + who + (d.visible ? "" : " hidden") + " a" + d.alpha.toFixed(2) +
                    " s" + d.scaleX.toFixed(3) + "/" + d.scaleY.toFixed(3) + " @" + d.x.toFixed(0) + "," +
                    d.y.toFixed(0) + " " + d.width.toFixed(0) + "x" + d.height.toFixed(0));
            }
            say("stage " + when + ": active " + TuruStage.active + ", fit " + TuruStage.fitScale.toFixed(3) +
                " @" + TuruStage.fitX.toFixed(1) + ", stage " + stage.stageWidth + "x" + stage.stageHeight +
                " " + stage.scaleMode + "; " + parts.join(" | "));
        }

        /**
         * The game's focused text field, for the native field above the soft keyboard (Kotlin's
         * `GameField`: the keyboard covers the game's field; AIR doesn't pan the stage:
         * `softKeyboardBehavior` none).
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

        /**
         * Typing in the native field above the keyboard (Kotlin's `GameField`), for the game's focused
         * text field: `{"op": "set", "text", "start", "end"}` (its text and selection now: only the
         * changed part is replaced, as typing does), `{"op": "enter"}` (the keyboard's Send: Enter, as
         * a key press) or `{"op": "close"}` (Back: done typing, the field loses the focus).
         */
        private function editTyping(level:String):void {
            var field:TextField = stage.focus as TextField;
            if (field == null || field.type != TextFieldType.INPUT) {
                return;
            }
            var request:Object;
            try {
                request = JSON.parse(level);
            } catch (e:Error) {
                say("ERROR: typing request " + level);
                return;
            }
            switch (request.op) {
                case "set":
                    var old:String = field.text;
                    var now:String = String(request.text);
                    if (old != now) {
                        var prefix:int = 0;
                        while (prefix < old.length && prefix < now.length && old.charCodeAt(prefix) == now.charCodeAt(prefix)) {
                            prefix++;
                        }
                        var suffix:int = 0;
                        while (suffix < old.length - prefix && suffix < now.length - prefix &&
                            old.charCodeAt(old.length - 1 - suffix) == now.charCodeAt(now.length - 1 - suffix)) {
                            suffix++;
                        }
                        field.replaceText(prefix, old.length - suffix, now.substring(prefix, now.length - suffix));
                        field.dispatchEvent(new Event(Event.CHANGE, true));
                    }
                    field.setSelection(int(request.start), int(request.end));
                    break;
                case "enter":
                    field.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_DOWN, true, false, Keyboard.ENTER, Keyboard.ENTER));
                    field.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_UP, true, false, Keyboard.ENTER, Keyboard.ENTER));
                    break;
                case "close":
                    stage.focus = null;
                    break;
            }
            watchTyping(null);
        }

        private function onFail(event:ErrorEvent):void {
            say("ERROR " + event.type + ": " + event.text);
        }

        private function onUncaught(event:UncaughtErrorEvent):void {
            var error:* = event.error;
            say("UNCAUGHT " + (error is Error ? Error(error).errorID + " " + Error(error).message + "\n" + Error(error).getStackTrace() : String(error)));
        }

        /**
         * The engine's tab requests (`tab`: `{"op": "open" | "select" | "close", "client": n}`),
         * and AIR's view's size (`layout`: "width height", from Kotlin).
         */
        private function onTab(event:StatusEvent):void {
            if (event.code == "typing") {
                editTyping(event.level);
                return;
            }
            if (event.code == "layout") {
                var size:Array = event.level.split(" ");
                viewSize = [Number(size[0]), Number(size[1])];
                if (stage != null) {
                    fit(viewSize[0], viewSize[1]);
                }
                return;
            }
            if (event.code != "tab") {
                return;
            }
            var request:Object;
            try {
                request = JSON.parse(event.level);
            } catch (e:Error) {
                say("ERROR: tab request " + event.level);
                return;
            }
            var client:int = int(request.client);
            switch (request.op) {
                case "open":
                    if (hostBytes == null) {
                        waiting.push(client);
                    } else {
                        open(client);
                    }
                    break;
                case "select":
                    TuruStage.activate(client);
                    dumpStage("select " + client);
                    break;
                case "close":
                    close(client);
                    break;
            }
        }

        private function onHostBytes(event:Event):void {
            hostBytes = URLLoader(event.target).data;
            open(0);
            for each (var client:int in waiting) {
                open(client);
            }
            waiting = [];
        }

        /**
         * Fits the games to AIR's view, `width` x `height` pixels. AIR scales the stage (show all,
         * aligned to the top) to its own idea of its size, which stays the screen's when its view
         * gets shorter (it takes that for a soft keyboard): the tab strip's margin. Then the games
         * would run off the view's bottom and right, so each is scaled and moved to where the
         * stage would have put it in the view ([TuruStage.place]). With the sizes the same,
         * nothing moves.
         */
        private function fit(width:Number, height:Number):void {
            if (width <= 0 || height <= 0) {
                return;
            }
            measuring = true;
            stage.scaleMode = StageScaleMode.NO_SCALE;
            var airWidth:Number = stage.stageWidth;
            var airHeight:Number = stage.stageHeight;
            stage.scaleMode = StageScaleMode.SHOW_ALL;
            measuring = false;
            var w:Number = loaderInfo.width;
            var h:Number = loaderInfo.height;
            var airScale:Number = Math.min(airWidth / w, airHeight / h);
            var viewScale:Number = Math.min(width / w, height / h);
            var airLeft:Number = (airWidth - w * airScale) / 2;
            var viewLeft:Number = (width - w * viewScale) / 2;
            TuruStage.fitScale = viewScale / airScale;
            TuruStage.fitX = (viewLeft - airLeft) / airScale;
            TuruStage.placeAll();
            say("view " + width + "x" + height + ", AIR's stage " + airWidth + "x" + airHeight +
                ": games at " + TuruStage.fitScale.toFixed(3) + "x, " + TuruStage.fitX.toFixed(1));
        }

        /** Loads client `client`'s turu.swf, which then loads its game (asked by its tab's bot). */
        private function open(client:int):void {
            if (hosts[client] != null) {
                say("ERROR: client " + client + " is open already");
                return;
            }
            say("opening client " + client);
            var context:LoaderContext = new LoaderContext(false, new ApplicationDomain(ApplicationDomain.currentDomain));
            context.allowCodeImport = true;
            context.parameters = {turuClient: String(client)};
            var host:Loader = new Loader();
            hosts[client] = host;
            host.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR, onUncaught);
            host.contentLoaderInfo.addEventListener(Event.COMPLETE, function(event:Event):void {
                if (hosts[client] != host) {
                    return; // Closed while it loaded.
                }
                var main:DisplayObject = LoaderInfo(event.target).content;
                TuruStage.mains[client] = main;
                // Under the host, until turu.swf swaps itself for its game.
                TuruStage.place(main);
                stage.addChild(main);
                TuruStage.activate(TuruStage.active);
                say("client " + client + "'s turu.swf loaded");
            });
            host.contentLoaderInfo.addEventListener(IOErrorEvent.IO_ERROR, onFail);
            host.loadBytes(hostBytes, context);
        }

        /**
         * Closes client `client` (its tab closed; the engine has stopped its bot): the game's
         * connection, its display objects, and its SWFs with what they run (`unloadAndStop`).
         */
        private function close(client:int):void {
            var game:Object = TuruStage.games[client];
            if (game != null) {
                try {
                    game.sfc.disconnect();
                } catch (e:Error) {
                }
            }
            for each (var shown:DisplayObject in [TuruStage.games[client], TuruStage.mains[client]]) {
                if (shown != null && shown.parent != null) {
                    shown.parent.removeChild(shown);
                }
            }
            for each (var loader:Loader in [TuruStage.gameLoaders[client], hosts[client]]) {
                if (loader != null) {
                    try {
                        loader.unloadAndStop(true);
                    } catch (e:Error) {
                    }
                }
            }
            TuruStage.remove(client);
            TuruExternal.forget(client);
            hosts[client] = null;
            say("closed client " + client);
        }

        /**
         * A client's game reached the stage ([TuruLoader] knew it as that client's): its turu.swf adds
         * it on top, before it's anyone's; the next frame puts the one on screen back in front.
         */
        private function watchGames(event:Event):void {
            var added:DisplayObject = event.target as DisplayObject;
            if (added == null || added.parent != stage || added == this) {
                return;
            }
            var client:int = TuruStage.clientOf(added);
            if (client < 0) {
                return;
            }
            say("client " + client + "'s game on the stage");
            addEventListener(Event.ENTER_FRAME, function settle(event:Event):void {
                removeEventListener(Event.ENTER_FRAME, settle);
                TuruStage.activate(TuruStage.active);
            });
        }
    }
}
