package turu.host {
import flash.system.ApplicationDomain;
import flash.utils.Dictionary;
import flash.utils.getQualifiedClassName;

/**
 * The calls that reach into the game by a dotted path (docs/VIBESKUA_SPEC.md, "Game object
 * access"): `world.myAvatar.objData.intGold` is `game.world.myAvatar.objData.intGold`. Values
 * come back as `JSON.stringify` of them; a path that throws answers nothing (the callback's
 * guard reports it).
 */
public class GameCalls {
    /** Split paths kept; past it the cache starts again (scripts can read any path). */
    private static const CACHED:int = 1000;

    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    private var cache:Dictionary = new Dictionary();
    private var cached:int = 0;

    public function GameCalls(gameOf:Function) {
        this.gameOf = gameOf;
    }

    /**
     * Registers the calls with `expose(name, fn, onThrow, quiet)`. Reads are quiet: a path that
     * doesn't resolve yet is an ordinary empty answer. Writes and calls report their throws.
     */
    public function exposeWith(expose:Function):void {
        expose("isTrue", isTrue, undefined, true);
        expose("isNull", isNull, undefined, true);
        expose("getGameObject", getGameObject, undefined, true);
        expose("getGameObjectS", getGameObjectS, undefined, true);
        expose("getGameObjectKey", getGameObjectKey, undefined, true);
        expose("getArrayObject", getArrayObject, undefined, true);
        expose("selectArrayObjects", selectArrayObjects, "", true);
        expose("selectArrayObjectsWhere", selectArrayObjectsWhere, "", true);
        expose("getMany", getMany, undefined, true);
        expose("setGameObject", setGameObject);
        expose("setGameObjectKey", setGameObjectKey);
        expose("setArrayObject", setArrayObject);
        expose("callGameFunction", callGameFunction);
        expose("callGameFunction0", callGameFunction0);
    }

    private function isTrue():String {
        return "true";
    }

    /** `"true"` when the value is null or the path doesn't resolve. */
    private function isNull(path:String):String {
        try {
            return String(walk(gameOf(), path) == null);
        } catch (e:Error) {
        }
        return "true";
    }

    private function getGameObject(path:String):String {
        return JSON.stringify(walk(gameOf(), path));
    }

    /** Rooted at the game's class: its statics (`serverIP`, `loginInfo.strUsername`). */
    private function getGameObjectS(path:String):String {
        return JSON.stringify(walk(gameClass(), path));
    }

    private function getGameObjectKey(path:String, key:String):String {
        return JSON.stringify(walk(gameOf(), path)[key]);
    }

    private function setGameObject(path:String, value:*):void {
        var parts:Array = pathParts(path);
        var last:int = parts.length - 1;
        walkParts(gameOf(), parts, last)[parts[last]] = value;
    }

    private function setGameObjectKey(path:String, key:String, value:*):void {
        walk(gameOf(), path)[key] = value;
    }

    private function getArrayObject(path:String, index:*):String {
        return JSON.stringify(walk(gameOf(), path)[index]);
    }

    private function setArrayObject(path:String, index:*, value:*):void {
        walk(gameOf(), path)[index] = value;
    }

    private function callGameFunction(path:String, ... args):String {
        var parts:Array = pathParts(path);
        var last:int = parts.length - 1;
        var owner:* = walkParts(gameOf(), parts, last);
        var fn:Function = owner[parts[last]];
        return JSON.stringify(fn.apply(owner, args));
    }

    private function callGameFunction0(path:String):String {
        return callGameFunction(path);
    }

    /** `[item[selector] for item in array]`, `selector` itself a dotted path. */
    private function selectArrayObjects(path:String, selector:String):String {
        var items:* = walk(gameOf(), path);
        if (!(items is Array)) {
            // VibeSkua's: "target is not an array", nothing.
            return "";
        }
        var picked:Array = [];
        var selectorParts:Array = pathParts(selector);
        for each (var item:* in items) {
            picked.push(walkParts(item, selectorParts, selectorParts.length));
        }
        return JSON.stringify(picked);
    }

    /**
     * The items of the array at `path` whose fields match all of `filter`'s
     * (`{sName: "Gold", bEquip: true}`), whole: one read instead of a column per field and an
     * item per match. A string matches trimmed, ignoring case and the game's `&lt;`, `&gt;`
     * and `&amp;`; a number by value (`"12"` is 12); a boolean as the game writes them (`1`,
     * `"1"`, `"true"`). Loose on purpose: the engine checks each item again.
     */
    private function selectArrayObjectsWhere(path:String, filter:*):String {
        var items:* = walk(gameOf(), path);
        if (!(items is Array)) {
            return "";
        }
        var keys:Array = [];
        for (var key:String in filter) {
            keys.push(key);
        }
        var picked:Array = [];
        for each (var item:* in items) {
            if (item != null && matchesAll(item, filter, keys)) {
                picked.push(item);
            }
        }
        return JSON.stringify(picked);
    }

    private static function matchesAll(item:*, filter:*, keys:Array):Boolean {
        for each (var key:String in keys) {
            var want:* = filter[key];
            var have:* = item[key];
            if (want is Boolean) {
                if (truthy(have) != want) {
                    return false;
                }
            } else if (want is Number) {
                if (have == null || Number(have) != Number(want)) {
                    return false;
                }
            } else if (looseName(have) != looseName(want)) {
                return false;
            }
        }
        return true;
    }

    private static function truthy(value:*):Boolean {
        if (value is Boolean) {
            return value;
        }
        if (value is Number) {
            return value != 0;
        }
        if (value is String) {
            var text:String = trim(value);
            return text == "1" || text.toLowerCase() == "true";
        }
        return false;
    }

    private static function looseName(value:*):String {
        if (value == null) {
            return "";
        }
        return trim(String(value)).split("&lt;").join("<").split("&gt;").join(">").split("&amp;").join("&")
            .toLowerCase();
    }

    private static function trim(text:String):String {
        return text.replace(/^\s+|\s+$/g, "");
    }

    /**
     * Several values in one read: `{key: path}` gives `{key: value}`. A path that doesn't
     * resolve, throws, or holds a value JSON can't write gives `null` for its key.
     */
    private function getMany(paths:*):String {
        var game:* = gameOf();
        var out:Object = {};
        for (var key:String in paths) {
            var value:* = null;
            try {
                value = walk(game, String(paths[key]));
            } catch (e:Error) {
            }
            out[key] = value === undefined ? null : value;
        }
        try {
            return JSON.stringify(out);
        } catch (e:Error) {
        }
        // One value JSON can't write (a cycle): find it rather than lose the rest.
        for (key in out) {
            try {
                JSON.stringify(out[key]);
            } catch (e:Error) {
                out[key] = null;
            }
        }
        return JSON.stringify(out);
    }

    /** The game's class, from the game's own domain (a child of its loader's). */
    private function gameClass():Class {
        var game:* = gameOf();
        var domain:ApplicationDomain = game.loaderInfo.applicationDomain;
        return Class(domain.getDefinition(getQualifiedClassName(game)));
    }

    private function walk(from:*, path:String):* {
        var parts:Array = pathParts(path);
        return walkParts(from, parts, parts.length);
    }

    /** `from` followed through the first `count` of `parts`. */
    private static function walkParts(from:*, parts:Array, count:int):* {
        var at:* = from;
        for (var i:int = 0; i < count; i++) {
            at = at[parts[i]];
        }
        return at;
    }

    private function pathParts(path:String):Array {
        var found:Array = cache[path];
        if (found == null) {
            if (cached >= CACHED) {
                cache = new Dictionary();
                cached = 0;
            }
            found = path == "" ? [] : path.split(".");
            cache[path] = found;
            cached++;
        }
        return found;
    }
}
}
