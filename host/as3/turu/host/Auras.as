package turu.host {
import flash.display.BitmapData;
import flash.display.DisplayObject;
import flash.display.MovieClip;
import flash.geom.Matrix;
import flash.geom.Rectangle;
import flash.utils.ByteArray;
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
        expose("GetAuraIcon", getAuraIcon, "", true);
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

    /**
     * Turu's (the Target Auras HUD): the aura icon `link` (a packet's `icon`: the last of a
     * comma list, as the game's aura HUD takes it) drawn on its first frame, fitted and centred
     * in `size` pixels square, as `"<size>:<Base64 of its ARGB pixels>"`; "" while the game
     * hasn't loaded it.
     */
    private function getAuraIcon(link:*, size:*):String {
        var name:String = String(link);
        var comma:int = name.lastIndexOf(",");
        if (comma > -1) {
            name = name.substr(comma + 1);
        }
        if (name == "" || name == "undefined" || name == "null") {
            return "";
        }
        var found:Class = classOf(name);
        var visual:DisplayObject = found != null ? new found() as DisplayObject : null;
        if (visual == null) {
            return "";
        }
        if (visual is MovieClip) {
            MovieClip(visual).stop();
        }
        var px:int = Math.max(8, Math.min(128, int(size)));
        var bounds:Rectangle = visual.getBounds(visual);
        if (bounds.width <= 0 || bounds.height <= 0) {
            return "";
        }
        var scale:Number = px / Math.max(bounds.width, bounds.height);
        var fit:Matrix = new Matrix(scale, 0, 0, scale,
            (px - bounds.width * scale) / 2 - bounds.x * scale,
            (px - bounds.height * scale) / 2 - bounds.y * scale);
        var bitmap:BitmapData = new BitmapData(px, px, true, 0);
        var pixels:ByteArray;
        try {
            bitmap.draw(visual, fit, null, null, null, true);
            pixels = bitmap.getPixels(bitmap.rect);
        } finally {
            bitmap.dispose();
        }
        return px + ":" + base64(pixels);
    }

    /**
     * The class `name` from where the game's `world.getClass` looks (its own domain, the assets,
     * the map, the players' class files), without its side effects: that one remembers a miss
     * for the rest of the room and says so in chat, which an icon asked for before its class
     * file loaded would leave the game's own HUD without. A name it already missed stays missed.
     */
    private function classOf(name:String):Class {
        var w:* = world;
        try {
            if (w.classMissCache != null && w.classMissCache[name]) {
                return null;
            }
        } catch (e:*) {
        }
        var domains:Array = [];
        try {
            domains.push(gameOf().loaderInfo.applicationDomain);
        } catch (e:*) {
        }
        try {
            domains.push(w.rootClass.assetsDomain);
        } catch (e:*) {
        }
        try {
            domains.push(w.loaderD);
        } catch (e:*) {
        }
        try {
            var stats:* = w.playerDomainsCache.getStats();
            if (stats != null && stats.entries != null) {
                for each (var entry:* in stats.entries) {
                    var context:* = w.playerDomainsCache.getContext(entry);
                    if (context != null) {
                        domains.push(context.applicationDomain);
                    }
                }
            }
        } catch (e:*) {
        }
        try {
            for (var key:String in w.playerDomains) {
                domains.push(w.playerDomains[key].loaderD);
            }
        } catch (e:*) {
        }
        for each (var domain:* in domains) {
            try {
                if (domain != null && domain.hasDefinition(name)) {
                    return domain.getDefinition(name) as Class;
                }
            } catch (e:*) {
            }
        }
        return null;
    }

    private static const BASE64:String = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    /** `bytes` in Base64, padded. */
    private static function base64(bytes:ByteArray):String {
        var out:Array = [];
        var length:uint = bytes.length;
        for (var i:uint = 0; i < length; i += 3) {
            var b0:uint = bytes[i];
            var b1:uint = i + 1 < length ? bytes[i + 1] : 0;
            var b2:uint = i + 2 < length ? bytes[i + 2] : 0;
            var n:uint = (b0 << 16) | (b1 << 8) | b2;
            out.push(BASE64.charAt((n >> 18) & 63), BASE64.charAt((n >> 12) & 63),
                i + 1 < length ? BASE64.charAt((n >> 6) & 63) : "=",
                i + 2 < length ? BASE64.charAt(n & 63) : "=");
        }
        return out.join("");
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
