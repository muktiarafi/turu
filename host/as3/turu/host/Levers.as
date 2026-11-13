package turu.host {
import flash.display.DisplayObject;
import flash.display.DisplayObjectContainer;
import flash.display.MovieClip;
import flash.display.Shape;
import flash.events.Event;
import flash.utils.Dictionary;
import flash.utils.getQualifiedClassName;

/**
 * Ways to cut what the game asks of the player, each off until turned on, to measure
 * (docs/HOST_SWF_PROPOSAL.md M7a): `setLever(name, on)`. They change what the game shows, never
 * what it does, and turning one off puts back what it changed.
 *
 * - `stopAnimations`: stops the clips that play in the map's art and on other players, so the
 *   player's dormant marks can skip them. A clip plays if its frame moves from one frame to the
 *   next: `isPlaying` is false for a timeline that plays by itself.
 * - `mouseOff`: no hit tests in the map's art and on other players (`mouseChildren`).
 * - `stripFilters`: removes the filters (glows, shadows) of the map's art and other players.
 * - `lowQuality`: `stage.quality` low.
 * - `cacheStatic`: `cacheAsBitmap` on the map's top layers that have no animation.
 * - `eventsOff`: pauses the event watcher ([Events]), to measure what it costs.
 *
 * What a lever touches is looked for again when it's turned on and on each `refresh` (a new map,
 * players coming in), and remembered so it can be put back.
 */
public class Levers {
    /** How deep a walk of the map's art goes. */
    private static const DEPTH:int = 12;

    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    private var on:Object = {};
    /** What each lever changed: object -> what it was (weak keys: a gone map lets go). */
    private var stopped:Dictionary = new Dictionary(true);
    private var mouse:Dictionary = new Dictionary(true);
    private var filters:Dictionary = new Dictionary(true);
    private var cached:Dictionary = new Dictionary(true);
    private var quality:String = null;
    /** Clips with more than one frame -> their frame when `stopAnimations` looked: those that
     * moved by the next frame are playing (`isPlaying` is false for a timeline that plays by
     * itself, as in Flash Player). */
    private var candidates:Dictionary = new Dictionary(true);
    private var ticker:Shape = new Shape();
    /** For `census`: each clip's frame at the last census. */
    private var censusFrames:Dictionary = new Dictionary(true);

    /** What the last `setLever` changed, and the objects it couldn't look at (they threw). */
    private var changed:int = 0;
    private var failed:int = 0;

    private var events:Events;

    public function Levers(gameOf:Function, events:Events) {
        this.gameOf = gameOf;
        this.events = events;
    }

    public function exposeWith(expose:Function):void {
        expose("setLever", setLever);
    }

    /** Turns a lever on or off; `refresh` re-applies the ones that are on. */
    /**
     * Answers `{changed, failed}`: how many objects it changed (for `stopAnimations`, how many
     * clips it watches for a frame, stopping those that move), and how many it couldn't look at.
     * `census` changes nothing and answers where the clips that play are instead.
     */
    private function setLever(name:*, enabled:*):String {
        var lever:String = String(name);
        changed = failed = 0;
        if (lever == "census") {
            return census();
        }
        if (lever == "refresh") {
            for (var key:String in on) {
                if (on[key]) {
                    apply(key, true);
                }
            }
        } else {
            on[lever] = enabled == true;
            apply(lever, enabled == true);
        }
        return JSON.stringify({changed: changed, failed: failed});
    }

    /**
     * The game's clips that play, grouped by where they are: the class names of their first
     * three ancestors under the game (`World > MovieClip > ...`), the 12 biggest groups.
     */
    private function census():String {
        var game:* = gameOf();
        if (game == null) {
            return "{}";
        }
        var groups:Object = {};
        var total:int = 0;
        var visit:Function = function(d:DisplayObject, path:Array):void {
            var clip:MovieClip = d as MovieClip;
            if (clip != null && clip.totalFrames > 1) {
                // Playing: its frame moved since the last census (or a script played it).
                var before:* = censusFrames[clip];
                if (clip.isPlaying || (before !== undefined && before != clip.currentFrame)) {
                    var key:String = path.slice(0, 3).join(" > ");
                    groups[key] = (groups[key] || 0) + 1;
                    total++;
                }
                censusFrames[clip] = clip.currentFrame;
            }
            var c:DisplayObjectContainer = d as DisplayObjectContainer;
            if (c == null || path.length >= 20) {
                return;
            }
            for (var i:int = 0; i < c.numChildren; i++) {
                try {
                    var child:DisplayObject = c.getChildAt(i);
                    var name:String = getQualifiedClassName(child);
                    if (child.name != null && child.name.indexOf("instance") != 0) {
                        name += "(" + child.name + ")";
                    }
                    visit(child, path.concat([name]));
                } catch (e:Error) {
                    failed++;
                }
            }
        };
        visit(game, []);
        var list:Array = [];
        for (var k:String in groups) {
            list.push({at: k, playing: groups[k]});
        }
        list.sortOn("playing", Array.NUMERIC | Array.DESCENDING);
        return JSON.stringify({playing: total, failed: failed, groups: list.slice(0, 12)});
    }

    private function apply(lever:String, enabled:Boolean):Boolean {
        var world:* = gameOf() != null ? gameOf().world : null;
        switch (lever) {
            case "eventsOff":
                events.paused = enabled;
                return true;
            case "lowQuality":
                return setQuality(enabled);
            case "stopAnimations":
                if (enabled) {
                    eachTarget(world, stopClips);
                } else {
                    for (var clip:* in stopped) {
                        MovieClip(clip).play();
                    }
                    stopped = new Dictionary(true);
                }
                return true;
            case "mouseOff":
                if (enabled) {
                    eachTarget(world, function(o:DisplayObject):void {
                        var c:DisplayObjectContainer = o as DisplayObjectContainer;
                        if (c != null && c.mouseChildren && mouse[c] === undefined) {
                            mouse[c] = true;
                            changed++;
                            c.mouseChildren = false;
                        }
                    });
                } else {
                    for (var c:* in mouse) {
                        DisplayObjectContainer(c).mouseChildren = true;
                    }
                    mouse = new Dictionary(true);
                }
                return true;
            case "stripFilters":
                if (enabled) {
                    eachTarget(world, function(o:DisplayObject):void {
                        walk(o, 0, function(d:DisplayObject):void {
                            if (d.filters.length > 0 && filters[d] === undefined) {
                                filters[d] = d.filters;
                                changed++;
                                d.filters = [];
                            }
                        });
                    });
                } else {
                    for (var f:* in filters) {
                        DisplayObject(f).filters = filters[f];
                    }
                    filters = new Dictionary(true);
                }
                return true;
            case "cacheStatic":
                if (enabled) {
                    cacheStatic(world);
                } else {
                    for (var d:* in cached) {
                        DisplayObject(d).cacheAsBitmap = cached[d];
                    }
                    cached = new Dictionary(true);
                }
                return true;
        }
        return false;
    }

    /** The map's art and every other player's avatar; never yours, never monsters. */
    private function eachTarget(world:*, fn:Function):void {
        if (world == null) {
            return;
        }
        try {
            if (world.map != null) {
                fn(world.map);
            }
        } catch (e:Error) {
        }
        try {
            var mine:* = world.myAvatar != null ? world.myAvatar.pMC : null;
            for each (var avatar:* in world.avatars) {
                if (avatar != null && avatar.pMC != null && avatar.pMC != mine) {
                    fn(avatar.pMC);
                }
            }
        } catch (e:Error) {
        }
    }

    /** Notes the frames of the clips that could be playing; the next frame stops those that moved. */
    private function stopClips(root:DisplayObject):void {
        walk(root, 0, function(d:DisplayObject):void {
            var clip:MovieClip = d as MovieClip;
            if (clip != null && clip.totalFrames > 1 && stopped[clip] === undefined) {
                candidates[clip] = clip.currentFrame;
                changed++;
            }
        });
        ticker.addEventListener(Event.ENTER_FRAME, stopMoved);
    }

    private function stopMoved(event:Event):void {
        ticker.removeEventListener(Event.ENTER_FRAME, stopMoved);
        if (!on["stopAnimations"]) {
            candidates = new Dictionary(true);
            return;
        }
        for (var key:* in candidates) {
            var clip:MovieClip = MovieClip(key);
            try {
                if (clip.currentFrame != candidates[key]) {
                    stopped[clip] = true;
                    clip.stop();
                }
            } catch (e:Error) {
            }
        }
        candidates = new Dictionary(true);
    }

    private function cacheStatic(world:*):void {
        if (world == null || world.map == null) {
            return;
        }
        var map:DisplayObjectContainer = world.map as DisplayObjectContainer;
        for (var i:int = 0; i < map.numChildren; i++) {
            var layer:DisplayObject = map.getChildAt(i);
            if (cached[layer] === undefined && !animates(layer)) {
                cached[layer] = layer.cacheAsBitmap;
                changed++;
                layer.cacheAsBitmap = true;
            }
        }
    }

    /** Whether anything in it has more than one frame. */
    private function animates(root:DisplayObject):Boolean {
        var found:Boolean = false;
        walk(root, 0, function(d:DisplayObject):void {
            var clip:MovieClip = d as MovieClip;
            if (clip != null && clip.totalFrames > 1) {
                found = true;
            }
        });
        return found;
    }

    private function setQuality(enabled:Boolean):Boolean {
        var game:* = gameOf();
        if (game == null || game.stage == null) {
            return false;
        }
        if (enabled) {
            if (quality == null) {
                quality = game.stage.quality;
            }
            game.stage.quality = "low";
        } else if (quality != null) {
            game.stage.quality = quality;
            quality = null;
        }
        return true;
    }

    /** `fn` on `d` and what's below it; an object that throws is counted and passed over. */
    private function walk(d:DisplayObject, depth:int, fn:Function):void {
        try {
            fn(d);
        } catch (e:Error) {
            failed++;
        }
        var c:DisplayObjectContainer = d as DisplayObjectContainer;
        if (c == null || depth >= DEPTH) {
            return;
        }
        for (var i:int = 0; i < c.numChildren; i++) {
            var child:DisplayObject = null;
            try {
                child = c.getChildAt(i);
            } catch (e:Error) {
                failed++;
            }
            if (child != null) {
                walk(child, depth + 1, fn);
            }
        }
    }
}
}
