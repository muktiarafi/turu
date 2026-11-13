package turu.host {
import flash.display.DisplayObject;
import flash.display.DisplayObjectContainer;
import flash.display.MovieClip;
import flash.utils.Dictionary;
import flash.utils.getQualifiedClassName;

/**
 * Switches for measuring what the game asks of the player (docs/HOST_SWF_PROPOSAL.md M7a),
 * turned on and off in turns by `TURU_LEVER_AB`: `setLever(name, on)`.
 *
 * - `stopAnimations`: [Animations] with the scope `all`, as for a tab nobody sees.
 * - `eventsOff`: pauses the event watcher ([Events]).
 * - `census` changes nothing: where the game's playing clips are.
 *
 * M7a also measured stripping filters, turning hit tests off, LOW quality and `cacheAsBitmap` on
 * the map's static layers: none paid (STATUS.md), so they went.
 */
public class Levers {
    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    private var events:Events;
    private var animations:Animations;
    /** For `census`: each clip's frame at the last census. */
    private var censusFrames:Dictionary = new Dictionary(true);
    private var failed:int = 0;

    public function Levers(gameOf:Function, events:Events, animations:Animations) {
        this.gameOf = gameOf;
        this.events = events;
        this.animations = animations;
    }

    public function exposeWith(expose:Function):void {
        expose("setLever", setLever);
    }

    /**
     * Answers `{changed}`: for `stopAnimations`, how many clips it watches to stop (or played
     * again); `census` answers where the clips that play are instead.
     */
    private function setLever(name:*, enabled:*):String {
        failed = 0;
        switch (String(name)) {
            case "census":
                return census();
            case "eventsOff":
                events.paused = enabled == true;
                return JSON.stringify({changed: 1});
            case "stopAnimations":
                return JSON.stringify({changed: int(animations.setScope(enabled == true ? "all" : ""))});
            case "refresh":
                // Animations looks again by itself.
                return JSON.stringify({changed: 0});
        }
        return JSON.stringify({changed: 0, unknown: String(name)});
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
}
}
