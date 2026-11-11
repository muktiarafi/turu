package {
    import flash.display.DisplayObject;
    import flash.display.Stage;

    /**
     * Several game clients on AIR's one stage (spike: docs/AIR_ARMY_SPIKE.md). The game, skua and
     * the SWFs the game loads find their game as `stage.getChildAt(0)`; with
     * `turu_air_patch::Options::shared_stage` each of those is [rootOf] instead, which finds the
     * game the asking object belongs to. The active client's game is kept at stage child 0, so
     * whatever still asks the stage directly gets the one on screen.
     */
    public class TuruStage {
        public static var stage:Stage;
        /** Each client's game, by client (null until its skua has put it on the stage). */
        public static var games:Array = [];
        /** The client on screen. */
        public static var active:int = 0;
        /** [rootOf] calls that couldn't tell whose game they meant (they got the active one). */
        public static var fallbacks:int = 0;

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

        /** skua's `stage.removeChildAt(0)`, which removes itself before it adds the game. */
        public static function removeOwn(main:DisplayObject):void {
            if (main != null && main.parent != null) {
                main.parent.removeChild(main);
            }
        }

        /** Shows client `index`'s game (at stage child 0) and hides the others. */
        public static function activate(index:int):void {
            active = index;
            for (var i:int = 0; i < games.length; i++) {
                var game:DisplayObject = games[i];
                if (game == null || game.parent == null) {
                    continue;
                }
                game.visible = i == index;
                if (i == index) {
                    game.parent.setChildIndex(game, 0);
                }
            }
        }
    }
}
