package {
    import flash.display.DisplayObject;
    import flash.display.Loader;
    import flash.display.Stage;

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

        /** Scales and moves a client's game (or skua) to fit AIR's view. */
        public static function place(d:DisplayObject):void {
            if (d != null) {
                d.scaleX = fitScale;
                d.scaleY = fitScale;
                d.x = fitX;
                d.y = 0;
            }
        }

        /** [place]s every client's game and skua. */
        public static function placeAll():void {
            for each (var list:Array in [games, mains]) {
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

        /** Shows client `client`'s game (at stage child 0) and hides the others. */
        public static function activate(client:int):void {
            active = client;
            for (var i:int = 0; i < games.length; i++) {
                var game:DisplayObject = games[i] as DisplayObject;
                if (game == null || game.parent == null) {
                    continue;
                }
                game.visible = i == client;
                if (i == client) {
                    game.parent.setChildIndex(game, 0);
                }
            }
            // A skua still loading its game: shown only while its client is.
            for (var j:int = 0; j < mains.length; j++) {
                var main:DisplayObject = mains[j] as DisplayObject;
                if (main != null && main.parent != null) {
                    main.visible = j == client;
                }
            }
        }

        /** Forgets client `client` (its tab closed). */
        public static function remove(client:int):void {
            mains[client] = null;
            games[client] = null;
            gameLoaders[client] = null;
        }
    }
}
