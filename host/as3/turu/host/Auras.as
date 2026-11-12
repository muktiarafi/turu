package turu.host {
import flash.utils.getTimer;

/**
 * Auras on you and on your target (docs/VIBESKUA_SPEC.md, "Auras"). A subject is `"Self"` (you:
 * `world.uoTree[your name]`) or `"Target"` (your target's `dataLeaf`). A leaf's `auras` are what
 * the server sent (`nam`, `val`, `t`, `cat`, `dur`, `ts`, ...); its `hudAuras` are what the
 * game's aura HUD shows (`nam`, `n`, `dur`, `end`, `persist`, `icon`, `desc`).
 *
 * Not carried over: VibeSkua's timer that rebuilds the aura arrays every 5 s. Turu drops
 * expired snapshots itself (`AuraSnapshot::expired`).
 */
public class Auras {
    /** Returns the game, or null before it loads. */
    private var gameOf:Function;

    public function Auras(gameOf:Function) {
        this.gameOf = gameOf;
    }

    /** Registers the calls with `expose(name, fn, onThrow, quiet)`; all are quiet reads. */
    public function exposeWith(expose:Function):void {
        expose("GetPlayerAura", getPlayerAura, "[]", true);
        expose("GetMonsterAuraByID", getMonsterAuraByID, "[]", true);
        expose("GetMonsterAuraByName", getMonsterAuraByName, "[]", true);
        expose("GetAuraSnapshots", getAuraSnapshots, undefined, true);
        expose("GetAurasValue", getAurasValue, "0", true);
        expose("HasAnyActiveAura", hasAnyActiveAura, "false", true);
    }

    private function get world():* {
        return gameOf().world;
    }

    /** The leaf whose auras `subject` means; null when there's none. */
    private function leafOf(subject:*):* {
        if (String(subject) == "Self") {
            var name:String = String(world.myAvatar.objData.strUsername).toLowerCase();
            return world.uoTree[name];
        }
        var target:* = world.myAvatar.target;
        return target != null ? target.dataLeaf : null;
    }

    private static function aurasOf(leaf:*):Array {
        var auras:* = leaf != null ? leaf.auras : null;
        return auras is Array ? auras : [];
    }

    private function getPlayerAura(name:*):String {
        return JSON.stringify(aurasOf(world.uoTree[String(name).toLowerCase()]));
    }

    private function getMonsterAuraByID(mapId:*):String {
        return JSON.stringify(aurasOf(world.monTree[mapId]));
    }

    private function getMonsterAuraByName(name:*):String {
        var wanted:String = String(name).toLowerCase();
        for each (var monster:* in world.getMonstersByCell(world.strFrame)) {
            if (monster != null && String(monster.objData.strMonName).toLowerCase() == wanted) {
                return JSON.stringify(aurasOf(monster.dataLeaf));
            }
        }
        return "[]";
    }

    /**
     * The HUD's auras, with the seconds each has left, 0 once run out (`end` is on the player's
     * clock, `getTimer`), in the order they came (`seq`). `hudAuras` is keyed by the aura's name. The
     * keys go in VibeSkua's order, which `JSON.stringify` keeps.
     */
    private function getAuraSnapshots(subject:*):String {
        var leaf:* = leafOf(subject);
        if (leaf == null || leaf.hudAuras == null) {
            return "[]";
        }
        var now:int = getTimer();
        var out:Array = [];
        for (var name:String in leaf.hudAuras) {
            var aura:* = leaf.hudAuras[name];
            if (aura == null) {
                continue;
            }
            var snapshot:Object = {};
            snapshot.dur = aura.dur;
            snapshot.nam = name;
            snapshot.remaining = Math.max(0, (Number(aura.end) - now) / 1000);
            snapshot.persist = Boolean(aura.persist);
            snapshot.desc = aura.desc != null ? String(aura.desc) : "";
            snapshot.seq = aura.seq;
            snapshot.icon = aura.icon;
            snapshot.n = aura.n;
            out.push(snapshot);
        }
        out.sortOn("seq", Array.NUMERIC);
        return JSON.stringify(out);
    }

    /** The value of `subject`'s aura named `name` (any case), 0 without it. */
    private function getAurasValue(subject:*, name:*):String {
        var wanted:String = String(name).toLowerCase();
        for each (var aura:* in aurasOf(leafOf(subject))) {
            if (aura != null && String(aura.nam).toLowerCase() == wanted) {
                return String(aura.val != null ? aura.val : 1);
            }
        }
        return "0";
    }

    /** Whether `subject` has any of the auras named in `names` (comma-separated, any case). */
    private function hasAnyActiveAura(subject:*, names:*):String {
        var wanted:Object = {};
        for each (var one:String in String(names).toLowerCase().split(",")) {
            wanted[one] = true;
        }
        for each (var aura:* in aurasOf(leafOf(subject))) {
            if (aura != null && wanted[String(aura.nam).toLowerCase()] === true) {
                return "true";
            }
        }
        return "false";
    }
}
}
