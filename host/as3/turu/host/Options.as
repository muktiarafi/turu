package turu.host {
import flash.utils.Dictionary;
import flash.utils.getTimer;

/**
 * The game-side options and modules (docs/VIBESKUA_SPEC.md, "World" and "Modules"):
 * `disableDeathAd`, `killLag`, `skipCutscenes`, and the modules `modEnable`/`modDisable` turn
 * on: Hide Players (other avatars hidden), Disable FX (avatars' effects off: the game plays one
 * only while an avatar's `pMC.spFX` is set), Disable Collisions (`world.arrSolid`, the walls,
 * emptied), and Turu's own Infinite Range, Untarget Lite (Lite's untarget-self and
 * untarget-dead on) and Skip Cutscenes (the overlay taken off, the world shown again unless Lag
 * Killer is on, and a cutscene the game waits to show cancelled), which the engine did on every
 * options pass before. An enabled module is re-applied every 100 ms (new avatars, a new map), and turning it
 * off restores what it changed, which VibeSkua's Disable FX doesn't.
 */
public class Options {
    private static const EVERY_MS:int = 100;

    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    private var host:Host;
    private var hidePlayers:Boolean = false;
    private var disableFX:Boolean = false;
    private var disableCollisions:Boolean = false;
    /** Infinite Range: every skill reaches across the map (the game resets ranges on a class
     * change, so it's kept applied). */
    private var infiniteRange:Boolean = false;
    /** Lite's "untarget self" and "untarget dead" kept on, as Turu always wants them. */
    private var untargetLite:Boolean = false;
    /** Skip Cutscenes, and Lag Killer as `killLag` last set it (a skipped cutscene leaves the
     * world hidden: it's shown again unless Lag Killer hides it). */
    private var skipCutscenesOn:Boolean = false;
    private var lagKiller:Boolean = false;
    private var lastFrame:int = 0;
    /** Disable FX: each avatar MC's `spFX`, to give back. */
    private var savedFX:Dictionary = new Dictionary(true);
    /** Disable Collisions: the walls taken away, and the empty list put in their place. */
    private var savedSolid:Array;
    private var noSolid:Array = [];

    public function Options(gameOf:Function, host:Host) {
        this.gameOf = gameOf;
        this.host = host;
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
        lagKiller = isOn(enable);
        world.visible = !lagKiller;
    }

    /**
     * Takes the cutscene off (`mcExtSWF`'s children) and shows the game's interface again. A
     * cutscene leaves the world hidden: with `showWorld` (Lag Killer off), a skipped one also
     * shows the world again (VibeSkua's stayed black until Lag Killer was toggled). Answers
     * whether there was one. The Skip Cutscenes module does this every 100 ms.
     */
    private function skipCutscenes(showWorld:* = false):String {
        var game:* = gameOf();
        var scenes:* = game.mcExtSWF;
        var skipped:Boolean = scenes.numChildren > 0;
        while (scenes.numChildren > 0) {
            scenes.removeChildAt(0);
        }
        game.showInterface();
        if (skipped && isOn(showWorld) && game.world != null) {
            game.world.visible = true;
        }
        return String(skipped);
    }

    /**
     * A map can ask for a cutscene (`world.cHandle.showCutscene`, lichwar's intro for one); while
     * it's pending the game holds back the monsters of the cells you enter until its overlay
     * appears, for up to 450 frames (~225 s at a hidden tab's 2 FPS). Skipping removes the
     * overlay, often before the game sees it, so those monsters stayed unattackable. This cancels
     * the pending cutscene (`cancel`, what its own end calls) and enters the cell again, which
     * places them (the game's cell settle gave up meanwhile).
     */
    private function cancelPendingCutscene(w:*):void {
        var handle:* = w.cHandle;
        if (handle == null || handle.isPending() !== true) {
            return;
        }
        handle.cancel();
        if (w.strFrame != null && w.strPad != null) {
            w.moveToCell(w.strFrame, w.strPad);
        }
        host.call("debug", ["Skip Cutscenes: cancelled the cutscene the game was waiting to show."]);
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
            case "InfiniteRange":
                infiniteRange = on;
                break;
            case "UntargetLite":
                untargetLite = on;
                break;
            case "SkipCutscenes":
                skipCutscenesOn = on;
                break;
        }
        if (on) {
            apply();
        }
    }

    /** Every frame (from `Main`): re-applies the enabled modules every 100 ms. */
    public function frame():void {
        if (!hidePlayers && !disableFX && !disableCollisions && !infiniteRange && !untargetLite &&
            !skipCutscenesOn) {
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
        if (skipCutscenesOn) {
            try {
                if (gameOf().mcExtSWF.numChildren > 0) {
                    skipCutscenes(!lagKiller);
                }
                cancelPendingCutscene(w);
            } catch (e:Error) {
            }
        }
        if (infiniteRange) {
            try {
                var active:* = w.actions.active;
                for (var i:int = 0; active != null && i < 6; i++) {
                    if (active[i] != null && active[i].range != 20000) {
                        active[i].range = 20000;
                    }
                }
            } catch (e:Error) {
            }
        }
        if (untargetLite) {
            try {
                var lite:* = gameOf().litePreference.data;
                if (lite.bUntargetSelf !== true) {
                    lite.bUntargetSelf = true;
                }
                if (lite.bUntargetDead !== true) {
                    lite.bUntargetDead = true;
                }
            } catch (e:Error) {
            }
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
