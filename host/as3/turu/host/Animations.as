package turu.host {
import flash.display.DisplayObject;
import flash.display.DisplayObjectContainer;
import flash.display.MovieClip;
import flash.display.Shape;
import flash.events.Event;
import flash.utils.Dictionary;
import flash.utils.getTimer;

/**
 * Stops the animations of the map's art and of other players (the `stopAnimations` lever,
 * docs/HOST_SWF_PROPOSAL.md M7a/M7b): `setScope("others")` for other players' avatars, `"all"`
 * for them and the map's art, `""` to play again what it stopped. Idle in a crowded room with
 * the tab shown, a frame's game logic went from 0.67 to 0.26 ms. Stopping them automatically
 * where nobody sees them (M7b) saved nothing measurable, so only the lever uses it.
 *
 * A clip plays if its frame moves from one frame to the next (`isPlaying` is false for a
 * timeline that plays by itself, as in Flash Player): it notes the frames of the clips with more
 * than one frame, and the next frame stops the ones that moved. It looks again every 2 s and on
 * a new map, as players come and go and the game plays clips again (walking, attacking). Your
 * own avatar and monsters are never touched.
 */
public class Animations {
    private static const DEPTH:int = 12;
    private static const AGAIN_MS:int = 2000;

    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    private var scope:String = "";
    /** What it stopped (weak keys: a gone map or player lets go). */
    private var stopped:Dictionary = new Dictionary(true);
    /** Clip -> its frame when looked at; the next frame stops those that moved. */
    private var watched:Dictionary = new Dictionary(true);
    private var watching:Boolean = false;
    private var ticker:Shape = new Shape();
    private var lastLook:int = 0;
    private var lastMap:* = null;

    public function Animations(gameOf:Function) {
        this.gameOf = gameOf;
        ticker.addEventListener(Event.ENTER_FRAME, onFrame);
    }

    /** The scope now (`""`, `"others"`, `"all"`). */
    public function get current():String {
        return scope;
    }

    /** Sets the scope; answers how many clips it watches to stop, or played again. */
    public function setScope(value:*):String {
        var next:String = value == null ? "" : String(value);
        if (next != "others" && next != "all") {
            next = "";
        }
        var count:int = 0;
        if (next != scope) {
            // Narrower or off: play again what it stopped, then look afresh.
            count = playAll();
            scope = next;
        }
        if (scope != "") {
            count = look();
        }
        return String(count);
    }

    private function playAll():int {
        var count:int = 0;
        for (var key:* in stopped) {
            try {
                MovieClip(key).play();
                count++;
            } catch (e:Error) {
            }
        }
        stopped = new Dictionary(true);
        watched = new Dictionary(true);
        watching = false;
        return count;
    }

    private function onFrame(event:Event):void {
        if (scope == "") {
            return;
        }
        if (watching) {
            stopMoved();
            return;
        }
        var world:* = null;
        try {
            world = gameOf() != null ? gameOf().world : null;
        } catch (e:Error) {
        }
        var map:* = world != null ? world.map : null;
        if (map != lastMap || getTimer() - lastLook >= AGAIN_MS) {
            look();
        }
    }

    /** Notes the frames of the clips that could be playing; answers how many. */
    private function look():int {
        lastLook = getTimer();
        var world:* = null;
        try {
            world = gameOf() != null ? gameOf().world : null;
        } catch (e:Error) {
        }
        if (world == null) {
            return 0;
        }
        lastMap = world.map;
        var count:int = 0;
        var note:Function = function(d:DisplayObject):void {
            var clip:MovieClip = d as MovieClip;
            if (clip != null && clip.totalFrames > 1 && stopped[clip] === undefined) {
                watched[clip] = clip.currentFrame;
                count++;
            }
        };
        if (scope == "all" && world.map != null) {
            walk(world.map, 0, note);
        }
        try {
            var mine:* = world.myAvatar != null ? world.myAvatar.pMC : null;
            for each (var avatar:* in world.avatars) {
                if (avatar != null && avatar.pMC != null && avatar.pMC != mine) {
                    walk(avatar.pMC, 0, note);
                }
            }
        } catch (e:Error) {
        }
        watching = count > 0;
        return count;
    }

    private function stopMoved():void {
        watching = false;
        for (var key:* in watched) {
            try {
                var clip:MovieClip = MovieClip(key);
                if (clip.currentFrame != watched[key]) {
                    stopped[clip] = true;
                    clip.stop();
                }
            } catch (e:Error) {
            }
        }
        watched = new Dictionary(true);
    }

    /** `fn` on `d` and what's below it; an object that throws is passed over. */
    private static function walk(d:DisplayObject, depth:int, fn:Function):void {
        try {
            fn(d);
        } catch (e:Error) {
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
            }
            if (child != null) {
                walk(child, depth + 1, fn);
            }
        }
    }
}
}
