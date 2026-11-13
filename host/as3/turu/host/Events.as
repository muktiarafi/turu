package turu.host {
import flash.display.Shape;
import flash.events.Event;

/**
 * Tells Turu what the game doesn't send packets for (docs/HOST_SWF_PROPOSAL.md M5): once a
 * frame it looks at a few values and calls `event(name, json)` only when one changed.
 *
 * - `world` `{present}`: `world` came or went (login, logout, a lost connection).
 * - `mapLoaded` `{map}`: `world.mapLoadInProgress` went from true to false.
 * - `cellChanged` `{cell}`: `world.strFrame` changed.
 * - `arrived` `{x, y}`: the avatar moved, then stood still for two frames.
 * - `loading` `{shown}`: the loading screen (`mcConnDetail` on the stage) came up or went.
 * - `cutscene` `{shown}`: a cutscene overlay (a child of `mcExtSWF`) came up or went.
 *
 * Turu only wakes its waits on them and still reads the game to decide, so a late or missed
 * event costs time, never an answer.
 */
public class Events {
    /** Frames the avatar stands still before it has arrived. */
    private static const STILL_FRAMES:int = 2;

    private var host:Host;
    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    /** ENTER_FRAME reaches every display object, on the stage or not, on both hosts. */
    private var ticker:Shape = new Shape();

    private var world:Boolean = false;
    private var mapLoading:Boolean = false;
    private var cell:String = null;
    private var loading:Boolean = false;
    private var cutscene:Boolean = false;
    private var x:Number = NaN;
    private var y:Number = NaN;
    private var moving:Boolean = false;
    private var still:int = 0;

    public function Events(host:Host, gameOf:Function) {
        this.host = host;
        this.gameOf = gameOf;
        ticker.addEventListener(Event.ENTER_FRAME, onFrame);
    }

    private function emit(name:String, data:Object):void {
        host.call("event", [name, JSON.stringify(data)]);
    }

    private function onFrame(event:Event):void {
        var game:* = gameOf();
        if (game == null) {
            return;
        }
        try {
            lookAtScreens(game);
        } catch (e:Error) {
        }
        var w:* = null;
        try {
            w = game.world;
        } catch (e:Error) {
        }
        if ((w != null) != world) {
            world = w != null;
            cell = null;
            moving = false;
            emit("world", {present: world});
        }
        if (w == null) {
            return;
        }
        try {
            lookAtWorld(w);
        } catch (e:Error) {
        }
    }

    private function lookAtScreens(game:*):void {
        var shown:Boolean = game.mcConnDetail != null && game.mcConnDetail.stage != null;
        if (shown != loading) {
            loading = shown;
            emit("loading", {shown: shown});
        }
        var overlay:Boolean = game.mcExtSWF != null && game.mcExtSWF.numChildren > 0;
        if (overlay != cutscene) {
            cutscene = overlay;
            emit("cutscene", {shown: overlay});
        }
    }

    private function lookAtWorld(w:*):void {
        var inProgress:Boolean = w.mapLoadInProgress === true;
        if (mapLoading && !inProgress) {
            emit("mapLoaded", {map: String(w.strMapName)});
        }
        mapLoading = inProgress;
        var frame:String = w.strFrame == null ? null : String(w.strFrame);
        if (frame != cell) {
            cell = frame;
            emit("cellChanged", {cell: frame});
        }
        var pMC:* = w.myAvatar != null ? w.myAvatar.pMC : null;
        if (pMC == null) {
            moving = false;
            return;
        }
        var nowX:Number = Number(pMC.x);
        var nowY:Number = Number(pMC.y);
        if (nowX != x || nowY != y) {
            // The first sighting isn't a walk.
            moving = !isNaN(x);
            still = 0;
            x = nowX;
            y = nowY;
        } else if (moving && ++still >= STILL_FRAMES) {
            moving = false;
            emit("arrived", {x: Math.round(nowX), y: Math.round(nowY)});
        }
    }
}
}
