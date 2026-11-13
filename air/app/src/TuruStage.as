package {
    import flash.display.DisplayObject;
    import flash.display.DisplayObjectContainer;
    import flash.display.Loader;
    import flash.display.Sprite;
    import flash.display.Stage;
    import flash.events.KeyboardEvent;
    import flash.media.SoundChannel;
    import flash.media.SoundTransform;
    import flash.system.ApplicationDomain;
    import flash.utils.Dictionary;
    import flash.utils.getQualifiedClassName;

    /**
     * Turu Mas's game clients (tabs) on AIR's one stage (docs/AIR_ARMY_SPIKE.md), by client
     * number. The game, skua and the SWFs the game loads find their game as
     * `stage.getChildAt(0)`; `turu_air_patch::Options::shared_stage` makes each of those
     * [rootOf], which finds the game the asking object belongs to. The client on screen has its
     * game at stage child 0, so whatever still asks the stage directly gets the one shown; the
     * others are hidden, so AIR doesn't draw them.
     */
    public class TuruStage {
        public static var stage:Stage;
        /** Each client's skua (`skua::Main`), while it's open. */
        public static var mains:Array = [];
        /** Each client's game, once its skua has loaded it. */
        public static var games:Array = [];
        /** The loader of each client's game ([TuruLoader]). */
        public static var gameLoaders:Array = [];
        /** The client on screen. */
        public static var active:int = 0;
        /** [rootOf] calls that couldn't tell whose game they meant (they got the one on screen). */
        public static var fallbacks:int = 0;
        /** What fits the games to AIR's view (`TuruAirHost.fit`): their scale and left. */
        public static var fitScale:Number = 1;
        public static var fitX:Number = 0;
        /**
         * Each client's layer for what its code adds straight to the stage (popups: keybinds, the
         * character page, the drop blacklist, ...): just above its game, fitted and shown with it
         * ([stageAddChild]).
         */
        public static var overlays:Array = [];
        /** Sounds playing ([sound]): each `SoundChannel`'s client and its own volume. */
        private static var channels:Dictionary = new Dictionary(true);
        /** Key listeners' gates ([keys]), by the listener they gate. */
        private static var gates:Dictionary = new Dictionary();
        /** What hidden games' own `soundTransform` was ([activate] silences their timelines). */
        private static var gameVolumes:Array = [];
        /** What the hooks did, for the host's Debug log ([hookSummary]). */
        private static var keysHeld:int = 0;
        private static var soundsSilenced:int = 0;
        private static var popups:int = 0;
        /** Hooks that couldn't tell their tab, by kind ("sound", "keys", "popup"). */
        private static var unknown:Object = {};

        private static function lost(kind:String):void {
            unknown[kind] = int(unknown[kind]) + 1;
        }

        /** What the first few [assetsParent] calls that couldn't tell their tab saw, to log. */
        public static var lostDomains:Array = [];

        /** What `domain` holds of the tabs' classes, and what the tabs have so far. */
        private static function describe(domain:ApplicationDomain):String {
            var has:Array = [];
            for each (var name:String in ["Game", "turu.host::Main", "skua::Main", "TuruStage"]) {
                try {
                    if (domain.hasDefinition(name)) {
                        has.push(name);
                    }
                } catch (e:Error) {
                }
            }
            var tabs:Array = [];
            for (var i:int = 0; i < Math.max(mains.length, games.length); i++) {
                tabs.push(i + ": main " + (mains[i] != null ? getQualifiedClassName(mains[i]) : "-") +
                    ", game " + (games[i] != null) + ", loader " + (gameLoaders[i] != null));
            }
            return "domain has [" + has.join(", ") + "]; tabs " + tabs.join("; ");
        }

        /** What the shared stage's hooks have done so far, in a line. */
        public static function hookSummary():String {
            var lost:Array = [];
            for (var kind:String in unknown) {
                lost.push(unknown[kind] + " " + kind);
            }
            lost.sort();
            return keysHeld + " keys held back from hidden tabs, " + soundsSilenced +
                " sounds silenced, " + popups + " popups in a tab's layer; couldn't tell their tab: " +
                (lost.length > 0 ? lost.join(", ") : "none");
        }

        /** Scales and moves a client's game (or skua) to fit AIR's view. */
        public static function place(d:DisplayObject):void {
            if (d != null) {
                d.scaleX = fitScale;
                d.scaleY = fitScale;
                d.x = fitX;
                d.y = 0;
            }
        }

        /**
         * Puts the fit back on a game its own code reset to scale 1 at (0, 0): World Camera
         * zooms by scaling the game's root and, closing, sets it back to that (`onExit`,
         * `onReset`), which is right on one tab but undoes [place] with several. Each frame.
         */
        public static function keepFitted():void {
            if (fitScale == 1 && fitX == 0) {
                return;
            }
            var game:DisplayObject = games[active] as DisplayObject;
            if (game != null && game.scaleX == 1 && game.scaleY == 1 && game.x == 0 && game.y == 0) {
                place(game);
            }
        }

        /** [place]s every client's game, skua and overlay. */
        public static function placeAll():void {
            for each (var list:Array in [games, mains, overlays]) {
                for each (var d:DisplayObject in list) {
                    place(d);
                }
            }
        }

        /**
         * `x.stage.getChildAt(0)`: the ancestor of `x` that's a child of the stage (its client's
         * game). As the original, a TypeError (#1009) when `x` isn't on the stage.
         */
        public static function rootOf(x:*):DisplayObject {
            var d:DisplayObject = x as DisplayObject;
            if (d != null && !(d is Stage)) {
                if (d.stage == null) {
                    throw new TypeError("Error #1009: Cannot access a property or method of a null object reference.", 1009);
                }
                while (d.parent != null && !(d.parent is Stage)) {
                    d = d.parent;
                }
                if (d.parent is Stage) {
                    // A popup in a client's overlay belongs to that client's game.
                    var owner:int = overlays.indexOf(d);
                    if (owner >= 0 && games[owner] != null) {
                        return games[owner];
                    }
                    return d;
                }
            }
            fallbacks++;
            return stage != null && stage.numChildren > 0 ? stage.getChildAt(0) : null;
        }

        /**
         * skua's `stage.removeChildAt(0)`: at first skua itself (before it adds the game); when
         * it loads the game again (a relogin), the game it had put there.
         */
        public static function removeOwn(main:DisplayObject):void {
            if (main == null) {
                return;
            }
            if (main.parent != null) {
                main.parent.removeChild(main);
                return;
            }
            var client:int = mains.indexOf(main);
            var game:DisplayObject = client >= 0 ? games[client] as DisplayObject : null;
            if (game != null && game.parent != null) {
                game.parent.removeChild(game);
            }
        }

        /** Client `client`'s game, as its loader finished it ([TuruLoader]). */
        public static function setGame(client:int, game:DisplayObject, loader:Loader):void {
            place(game);
            games[client] = game;
            gameLoaders[client] = loader;
        }

        /** The client whose game `added` is, or -1. */
        public static function clientOf(added:DisplayObject):int {
            return games.indexOf(added);
        }

        /**
         * Shows client `client`'s game (at stage child 0, its overlay just above) and hides the
         * others; only its sounds play, and the keyboard's focus leaves a hidden game.
         */
        public static function activate(client:int):void {
            active = client;
            for (var i:int = 0; i < games.length; i++) {
                var game:DisplayObject = games[i] as DisplayObject;
                if (game == null || game.parent == null) {
                    continue;
                }
                var shown:Boolean = i == client;
                game.visible = shown;
                var overlay:DisplayObject = overlays[i] as DisplayObject;
                if (overlay != null) {
                    overlay.visible = shown;
                }
                if (shown) {
                    game.parent.setChildIndex(game, 0);
                    if (overlay != null && overlay.parent == game.parent) {
                        game.parent.setChildIndex(overlay, 1);
                    }
                }
                // Its timelines' sounds (maps, monsters, skills): through the game's own transform.
                var sprite:Sprite = game as Sprite;
                if (sprite != null && shown && gameVolumes[i] != null) {
                    sprite.soundTransform = gameVolumes[i] as SoundTransform;
                    gameVolumes[i] = null;
                } else if (sprite != null && !shown && gameVolumes[i] == null) {
                    gameVolumes[i] = sprite.soundTransform;
                    sprite.soundTransform = new SoundTransform(0);
                }
            }
            // Sounds the games' code plays ([sound]).
            for (var playing:Object in channels) {
                volume(playing as SoundChannel);
            }
            // A hidden game's text field would still take the keys.
            if (stage != null && stage.focus != null && clientOfObject(stage.focus) != client) {
                stage.focus = null;
            }
            // A skua still loading its game: shown only while its client is.
            for (var j:int = 0; j < mains.length; j++) {
                var main:DisplayObject = mains[j] as DisplayObject;
                if (main != null && main.parent != null) {
                    main.visible = j == client;
                }
            }
        }

        /** Forgets client `client` (its tab closed), and takes its overlay off the stage. */
        public static function remove(client:int):void {
            var overlay:DisplayObject = overlays[client] as DisplayObject;
            if (overlay != null && overlay.parent != null) {
                overlay.parent.removeChild(overlay);
            }
            overlays[client] = null;
            gameVolumes[client] = null;
            mains[client] = null;
            games[client] = null;
            gameLoaders[client] = null;
        }

        /**
         * The client whose code runs in `domain` (the code's own `ApplicationDomain.currentDomain`,
         * which the patched sites pass), or -1. Each client's skua has a domain of its own, its
         * game and maps load in it or below, so the `Game` class (or skua's main class, before the
         * game) found from there is that client's.
         */
        public static function clientOfDomain(domain:ApplicationDomain):int {
            if (domain == null) {
                return -1;
            }
            var i:int;
            try {
                if (domain.hasDefinition("Game")) {
                    var gameClass:Object = domain.getDefinition("Game");
                    for (i = 0; i < games.length; i++) {
                        if (games[i] != null && Object(games[i]).constructor == gameClass) {
                            return i;
                        }
                    }
                    // A game still starting (its constructor runs before its loader's `init`,
                    // when [setGame] learns it): by the loader [TuruLoader] registered.
                    for (i = 0; i < gameLoaders.length; i++) {
                        var loading:Loader = gameLoaders[i] as Loader;
                        if (games[i] == null && loading != null && loading.contentLoaderInfo.applicationDomain != null &&
                            loading.contentLoaderInfo.applicationDomain.hasDefinition("Game") &&
                            loading.contentLoaderInfo.applicationDomain.getDefinition("Game") == gameClass) {
                            return i;
                        }
                    }
                }
                for (i = 0; i < mains.length; i++) {
                    var main:Object = mains[i];
                    if (main == null) {
                        continue;
                    }
                    var name:String = getQualifiedClassName(main);
                    if (domain.hasDefinition(name) && domain.getDefinition(name) == main.constructor) {
                        return i;
                    }
                }
            } catch (e:Error) {
            }
            return -1;
        }

        /**
         * The parent for a `new ApplicationDomain()` in client code (`turu_air_patch` gives it
         * one): the domain of the client's skua. Without a parent the domain has only the
         * system's, so the SWFs loaded there (the game menu, the client's assets), patched to
         * call `TuruStage`, would fail with "TuruStage is not defined". Skua's domain has this
         * class (through the host's) and names the client ([clientOfDomain]) without the game's
         * classes, which would shadow the assets' own.
         */
        public static function assetsParent(domain:ApplicationDomain):ApplicationDomain {
            var client:int = clientOfDomain(domain);
            var main:DisplayObject = client >= 0 ? mains[client] as DisplayObject : null;
            if (main != null && main.loaderInfo != null && main.loaderInfo.applicationDomain != null) {
                return main.loaderInfo.applicationDomain;
            }
            // `TuruStage` is still found from the host's own; the tab isn't.
            lost("domain");
            if (lostDomains.length < 5) {
                lostDomains.push(describe(domain));
            }
            return ApplicationDomain.currentDomain;
        }

        /** The client whose game (or overlay, or skua) `x` is in, or -1. */
        public static function clientOfObject(x:Object):int {
            var d:DisplayObject = x as DisplayObject;
            if (d == null) {
                return -1;
            }
            while (d.parent != null && !(d.parent is Stage)) {
                d = d.parent;
            }
            for each (var list:Array in [games, overlays, mains]) {
                var i:int = list.indexOf(d);
                if (i >= 0) {
                    return i;
                }
            }
            return -1;
        }

        /**
         * `x.play(...)`'s result, from a client's code: a `SoundChannel` of a hidden client's is
         * silenced, and its volume follows the client's tab from then on ([activate]). Anything
         * else (a `MovieClip`'s `play()` gives nothing) passes through.
         */
        public static function sound(result:*, domain:ApplicationDomain):* {
            var channel:SoundChannel = result as SoundChannel;
            if (channel != null) {
                var client:int = clientOfDomain(domain);
                if (client >= 0) {
                    channels[channel] = {client: client, transform: channel.soundTransform};
                    volume(channel);
                    if (client != active) {
                        soundsSilenced++;
                    }
                } else {
                    lost("sound");
                }
            }
            return result;
        }

        private static function volume(channel:SoundChannel):void {
            var playing:Object = channels[channel];
            if (playing == null) {
                return;
            }
            if (playing.client == active) {
                channel.soundTransform = playing.transform as SoundTransform;
            } else {
                var muted:SoundTransform = channel.soundTransform;
                muted.volume = 0;
                channel.soundTransform = muted;
            }
        }

        /**
         * A key listener a client's code adds (`addEventListener(KeyboardEvent.KEY_DOWN, f)` and
         * `KEY_UP`, on the stage or anything else): `f` behind a gate that lets keys through only
         * while that client's tab is on screen. Every client's stage listeners hear every key, as
         * there's one stage. The same `f` gets the same gate, for [keysRemoved].
         */
        public static function keys(listener:*, domain:ApplicationDomain):* {
            var f:Function = listener as Function;
            if (f == null) {
                return listener;
            }
            var gate:Function = gates[f] as Function;
            if (gate != null) {
                return gate;
            }
            var client:int = clientOfDomain(domain);
            if (client < 0) {
                lost("keys");
                return listener;
            }
            gate = makeGate(f, client);
            gates[f] = gate;
            return gate;
        }

        /** `removeEventListener`'s side of [keys]: the gate `f` had, which is then forgotten. */
        public static function keysRemoved(listener:*, domain:ApplicationDomain):* {
            var f:Function = listener as Function;
            if (f == null) {
                return listener;
            }
            var gate:Function = gates[f] as Function;
            if (gate == null) {
                return listener;
            }
            delete gates[f];
            return gate;
        }

        private static function makeGate(f:Function, client:int):Function {
            return function(event:KeyboardEvent):void {
                if (client == active) {
                    f(event);
                } else {
                    keysHeld++;
                }
            };
        }

        /**
         * `parent.addChild(child)` near a `stage` in a client's code: on the stage itself, `child`
         * goes into that client's overlay instead, so it shows, hides and fits with its game.
         */
        public static function stageAddChild(parent:*, child:*, domain:ApplicationDomain):* {
            // Not the stage, or skua putting a game (or itself) there: as written.
            if (!(parent is Stage) || games.indexOf(child) >= 0 || mains.indexOf(child) >= 0) {
                return parent.addChild(child);
            }
            var client:int = clientOfDomain(domain);
            if (client < 0) {
                lost("popup");
                return parent.addChild(child);
            }
            // Its game isn't on the stage yet: nothing to sit above.
            var layer:DisplayObjectContainer = overlayFor(client);
            if (layer == null) {
                return parent.addChild(child);
            }
            popups++;
            return layer.addChild(child);
        }

        /** `parent.removeChild(child)`, for a child [stageAddChild] put in an overlay. */
        public static function stageRemoveChild(parent:*, child:*, domain:ApplicationDomain):* {
            var d:DisplayObject = child as DisplayObject;
            if (parent is Stage && d != null && d.parent != null && d.parent != parent &&
                overlays.indexOf(d.parent) >= 0) {
                return d.parent.removeChild(d);
            }
            return parent.removeChild(child);
        }

        /** `parent.getChildByName(name)`: on the stage, the client's overlay first. */
        public static function stageChildByName(parent:*, name:*, domain:ApplicationDomain):* {
            if (parent is Stage) {
                var client:int = clientOfDomain(domain);
                var overlay:DisplayObjectContainer = client >= 0 ? overlays[client] as DisplayObjectContainer : null;
                var found:DisplayObject = overlay != null ? overlay.getChildByName(String(name)) : null;
                if (found != null) {
                    return found;
                }
            }
            return parent.getChildByName(name);
        }

        /** Client `client`'s overlay, made just above its game when first needed; null without a game. */
        private static function overlayFor(client:int):DisplayObjectContainer {
            if (client < 0 || stage == null) {
                return null;
            }
            var game:DisplayObject = games[client] as DisplayObject;
            if (game == null || game.parent != stage) {
                return null;
            }
            var overlay:Sprite = overlays[client] as Sprite;
            if (overlay == null) {
                overlay = new Sprite();
                overlay.name = "turuOverlay" + client;
                overlays[client] = overlay;
            }
            if (overlay.parent != stage) {
                place(overlay);
                stage.addChildAt(overlay, stage.getChildIndex(game) + 1);
                overlay.visible = client == active;
            }
            return overlay;
        }
    }
}
