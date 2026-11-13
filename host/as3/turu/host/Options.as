package turu.host {
import flash.utils.Dictionary;
import flash.utils.getTimer;

/**
 * The game-side options and modules (docs/VIBESKUA_SPEC.md, "World" and "Modules"):
 * `disableDeathAd`, `killLag`, `skipCutscenes`, and the modules `modEnable`/`modDisable` turn
 * on: Hide Players (other avatars hidden), Disable FX (avatars' effects off: the game plays one
 * only while an avatar's `pMC.spFX` is set) and Disable Collisions (`world.arrSolid`, the walls,
 * emptied). An enabled module is re-applied every 100 ms (new avatars, a new map), and turning it
 * off restores what it changed, which VibeSkua's Disable FX doesn't.
 */
public class Options {
    private static const EVERY_MS:int = 100;

    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    private var hidePlayers:Boolean = false;
    private var disableFX:Boolean = false;
    private var disableCollisions:Boolean = false;
    private var lastFrame:int = 0;
    /** Disable FX: each avatar MC's `spFX`, to give back. */
    private var savedFX:Dictionary = new Dictionary(true);
    /** Disable Collisions: the walls taken away, and the empty list put in their place. */
    private var savedSolid:Array;
    private var noSolid:Array = [];

    public function Options(gameOf:Function) {
        this.gameOf = gameOf;
    }

    /** Registers the calls with `expose(name, fn, onThrow, quiet)`. */
    public function exposeWith(expose:Function):void {
        expose("disableDeathAd", disableDeathAd);
        expose("killLag", killLag);
        expose("skipCutscenes", skipCutscenes);
        expose("modEnable", modEnable);
        expose("modDisable", modDisable);
    }

    private function get world():* {
        return gameOf().world;
    }

    private static function isOn(value:*):Boolean {
        return value === true || value == "true";
    }

    private function disableDeathAd(enable:*):void {
        var preference:* = gameOf().userPreference;
        preference.data.bDeathAd = !isOn(enable);
    }

    /** Lag Killer: the world isn't drawn. */
    private function killLag(enable:*):void {
        world.visible = !isOn(enable);
    }

    /** Takes the cutscene off (`mcExtSWF`'s children) and shows the game's interface again. */
    private function skipCutscenes():void {
        var game:* = gameOf();
        var scenes:* = game.mcExtSWF;
        while (scenes.numChildren > 0) {
            scenes.removeChildAt(0);
        }
        game.showInterface();
    }

    private function modEnable(name:*):void {
        toggle(String(name), true);
    }

    private function modDisable(name:*):void {
        toggle(String(name), false);
    }

    /**
     * A module on or off. The quest window's two (`QuestItemRates`, `QuestRequirementWiki`) and
     * names Turu doesn't know (VibeSkua's C# also asks for `OptimizePlayers`) do nothing.
     */
    private function toggle(name:String, on:Boolean):void {
        switch (name) {
            case "HidePlayers":
                hidePlayers = on;
                if (!on) {
                    showPlayers();
                }
                break;
            case "DisableFX":
                disableFX = on;
                if (!on) {
                    restoreFX();
                }
                break;
            case "DisableCollisions":
                disableCollisions = on;
                if (!on) {
                    restoreSolid();
                }
                break;
        }
        if (on) {
            apply();
        }
    }

    /** Every frame (from `Main`): re-applies the enabled modules every 100 ms. */
    public function frame():void {
        if (!hidePlayers && !disableFX && !disableCollisions) {
            return;
        }
        var now:int = getTimer();
        if (now - lastFrame < EVERY_MS) {
            return;
        }
        lastFrame = now;
        apply();
    }

    private function apply():void {
        var w:* = gameOf() != null ? world : null;
        if (w == null) {
            return;
        }
        if (hidePlayers || disableFX) {
            for each (var avatar:* in otherAvatars(w, disableFX)) {
                var mc:* = avatar.pMC;
                if (mc == null) {
                    continue;
                }
                if (hidePlayers && avatar !== w.myAvatar) {
                    mc.visible = false;
                }
                if (disableFX && mc.spFX != null) {
                    savedFX[mc] = mc.spFX;
                    mc.spFX = null;
                }
            }
        }
        if (disableCollisions && w.arrSolid !== noSolid) {
            savedSolid = w.arrSolid;
            noSolid = [];
            w.arrSolid = noSolid;
        }
    }

    /** The avatars on the map: the others, and yours too when `withMine`. */
    private static function otherAvatars(w:*, withMine:Boolean):Array {
        var found:Array = [];
        for each (var avatar:* in w.avatars) {
            if (avatar != null && (withMine || avatar !== w.myAvatar)) {
                found.push(avatar);
            }
        }
        return found;
    }

    private function showPlayers():void {
        var w:* = world;
        for each (var avatar:* in otherAvatars(w, false)) {
            if (avatar.pMC != null) {
                avatar.pMC.visible = true;
            }
        }
    }

    private function restoreFX():void {
        for (var mc:* in savedFX) {
            if (mc.spFX == null) {
                mc.spFX = savedFX[mc];
            }
        }
        savedFX = new Dictionary(true);
    }

    private function restoreSolid():void {
        var w:* = world;
        if (w.arrSolid === noSolid && savedSolid != null) {
            w.arrSolid = savedSolid;
        }
        savedSolid = null;
    }
}
}
