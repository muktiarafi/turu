package turu.host {
import flash.display.DisplayObject;
import flash.display.Loader;

/**
 * What `turu.swf` needs from whatever runs it: the bot's calls in and out, loading a SWF, and
 * putting the game on screen. [ExternalHost] is Ruffle's (desktop and Ruffle Android), through
 * `ExternalInterface`; [AirHost] is Turu Mas's, through the AIR host's `TuruHost`, one of
 * several tabs on AIR's one stage (docs/HOST_SWF_PROPOSAL.md, R4).
 */
public class Host {
    public function Host() {
    }

    /** Lets the bot call `fn` by `name`. */
    public function addCallback(name:String, fn:Function):void {
        throw new Error("Host.addCallback");
    }

    /**
     * Tells the bot of an event. `args` arrive as one array (VibeSkua's `Externalizer.call`
     * convention, kept; `events.rs` unwraps it).
     */
    public function call(name:String, args:Array):void {
        throw new Error("Host.call");
    }

    /** A loader the host made, loading `url` (on AIR: through Turu's cache, patched for AIR). */
    public function load(url:String):Loader {
        throw new Error("Host.load");
    }

    /** Puts `game` on screen in place of `main` (the host SWF's root) and any earlier game. */
    public function show(main:DisplayObject, game:DisplayObject):void {
        throw new Error("Host.show");
    }

    /** Whether this tab is on screen: keys are its own only then. */
    public function isShown():Boolean {
        return true;
    }

    /** Whether the stage is this SWF's alone, to set its frame rate, scale and alignment. */
    public function ownsStage():Boolean {
        return true;
    }
}
}
