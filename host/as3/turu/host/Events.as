package turu.host {
import flash.display.Shape;
import flash.events.Event;
import flash.utils.getTimer;

/**
 * Tells Turu what the game doesn't send packets for (docs/HOST_SWF_PROPOSAL.md M5): once a
 * frame it looks at a few values and calls `event(name, json)` only when one changed.
 *
 * - `world` `{present}`: `world` came or went (login, logout, a lost connection).
 * - `mapLoaded` `{map}`: `world.mapLoadInProgress` went from true to false. It also empties
 *   your avatar's queue of undrawn spell effects then (`clearSpellFX`), and every 10 s on the
 *   same map (no event for that).
 * - `cellChanged` `{cell}`: `world.strFrame` changed.
 * - `arrived` `{x, y}`: the avatar moved, then stood still for two frames.
 * - `loading` `{shown}`: the loading screen (`mcConnDetail` on the stage) came up or went.
 * - `kicked` `{shown}`: the login screen's warning (`mcLogin.warning`, what `isKicked` reads)
 *   came up or went.
 *
 * - `frames` `{frames, avgMs, maxMs, slow}`: with `setFrameReport(seconds)`, every that many
 *   seconds, the gaps between frames (`slow`: over 50 ms). For Turu Mas, where AIR draws and
 *   Ruffle's own frame log doesn't apply (M7a).
 *
 * Turu only wakes its waits on them and still reads the game to decide, so a late or missed
 * event costs time, never an answer.
 */
public class Events {
    /** Frames the avatar stands still before it has arrived. */
    private static const STILL_FRAMES:int = 2;
    /**
     * How often `clearSpellFX` runs while you stay on one map. Turu: measured headless, the queue
     * gains ~2.7 entries per kill and the game's own animation doesn't keep up on one map (about
     * 16k Flash objects an hour at ~13 kills a minute; docs/STATUS.md, "Differences from
     * VibeSkua"). Why it doesn't keep up isn't known. Not run while `paused` (`eventsOff`).
     */
    private static const SPELL_FX_CLEAR_MS:int = 10000;

    private var host:Host;
    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    /** ENTER_FRAME reaches every display object, on the stage or not, on both hosts. */
    private var ticker:Shape = new Shape();

    private var world:Boolean = false;
    private var mapLoading:Boolean = false;
    private var cell:String = null;
    private var loading:Boolean = false;
    private var kicked:Boolean = false;
    private var x:Number = NaN;
    private var y:Number = NaN;
    private var moving:Boolean = false;
    private var still:int = 0;
    /** `getTimer()` when `clearSpellFX` last ran. */
    private var lastSpellFXClear:int = 0;

    /** The `eventsOff` lever (M7a): no looking and no events, to measure what they cost. */
    public var paused:Boolean = false;

    /** `setFrameReport`'s period in ms (0: off), and the gaps since the last report. */
    private var reportEvery:int = 0;
    private var reported:int = 0;
    private var lastFrame:int = 0;
    private var frames:int = 0;
    private var gapTotal:int = 0;
    private var gapMax:int = 0;
    private var slow:int = 0;

    public function Events(host:Host, gameOf:Function) {
        this.host = host;
        this.gameOf = gameOf;
        ticker.addEventListener(Event.ENTER_FRAME, onFrame);
    }

    public function exposeWith(expose:Function):void {
        expose("setFrameReport", setFrameReport);
    }

    private function setFrameReport(seconds:*):void {
        reportEvery = Math.max(0, int(Number(seconds) * 1000));
        reported = lastFrame = getTimer();
        frames = gapTotal = gapMax = slow = 0;
    }

    private function timeFrame():void {
        var now:int = getTimer();
        var gap:int = now - lastFrame;
        lastFrame = now;
        frames++;
        gapTotal += gap;
        gapMax = Math.max(gapMax, gap);
        if (gap > 50) {
            slow++;
        }
        // An int, so it wraps with `getTimer()` (see `lookAtWorld`).
        var sinceReport:int = now - reported;
        if (sinceReport >= reportEvery || sinceReport < 0) {
            emit("frames", {frames: frames, avgMs: frames > 0 ? Math.round(gapTotal / frames * 10) / 10 : 0,
                maxMs: gapMax, slow: slow});
            reported = now;
            frames = gapTotal = gapMax = slow = 0;
        }
    }

    private function emit(name:String, data:Object):void {
        host.call("event", [name, JSON.stringify(data)]);
    }

    private function onFrame(event:Event):void {
        if (reportEvery > 0) {
            timeFrame();
        }
        var game:* = gameOf();
        if (game == null || paused) {
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

    /**
     * Empties your avatar's queue of spell effects waiting to be drawn (`AvatarMC._spFXQueue`) on
     * a new map. `Game.doAnim` queues one, with the monsters it hit, whenever an effect is
     * already pending, and the game clears the queue only in `fClose` and in some branches of
     * `World.castSpellFX`, so entries outlive the map. Each one kept the left map's monster clips
     * and with them the shared domain of all its monster SWFs: a copy of every monster stayed
     * loaded per visit (x19 after 20 rounds of Greenguard East and West, at any frame rate).
     * What's dropped is only effects not yet drawn for monsters no longer there.
     *
     * Turu: also run every `SPELL_FX_CLEAR_MS` while you stay on one map: a 26-minute kill-only
     * probe on Greenguard West gained about 1,400 queued entries (and their `Object`/`Array`/string
     * garbage, ~16k Flash objects an hour) with no map change. What's dropped then is effects
     * queued in the last 10 s that weren't drawn yet, and it can include ones for monsters still
     * there: as far as was checked only the avatar's animation reads the queue, so it looks
     * visual only (not proven), and already stale.
     */
    private function clearSpellFX(w:*):void {
        try {
            var pMC:* = w.myAvatar != null ? w.myAvatar.pMC : null;
            if (pMC != null) {
                pMC.clearSpFXQueue();
            }
        } catch (e:Error) {
        }
    }

    private function lookAtScreens(game:*):void {
        var shown:Boolean = game.mcConnDetail != null && game.mcConnDetail.stage != null;
        if (shown != loading) {
            loading = shown;
            emit("loading", {shown: shown});
        }
        var warned:Boolean = false;
        try {
            warned = game.mcLogin != null && game.mcLogin.warning.visible == true;
        } catch (e:Error) {
        }
        if (warned != kicked) {
            kicked = warned;
            emit("kicked", {shown: warned});
        }
    }

    private function lookAtWorld(w:*):void {
        var inProgress:Boolean = w.mapLoadInProgress === true;
        if (mapLoading && !inProgress) {
            clearSpellFX(w);
            lastSpellFXClear = getTimer();
            emit("mapLoaded", {map: String(w.strMapName)});
        }
        mapLoading = inProgress;
        // Turu: and every SPELL_FX_CLEAR_MS on the same map, which a farm can stay on for hours.
        var now:int = getTimer();
        // `elapsed` is an int so the subtraction wraps back to the right gap when `getTimer()`
        // turns negative after ~24.8 days; a negative gap (the timer went back) clears too.
        var elapsed:int = now - lastSpellFXClear;
        if (!inProgress && (elapsed >= SPELL_FX_CLEAR_MS || elapsed < 0)) {
            lastSpellFXClear = now;
            clearSpellFX(w);
        }
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
