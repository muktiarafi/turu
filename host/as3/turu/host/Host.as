package turu.host {
import flash.display.DisplayObject;
import flash.display.Loader;

/**
 * What `turu.swf` needs from whatever runs it: the bot's calls in and out, loading a SWF, and
 * putting the game on screen. [ExternalHost] is Ruffle's (desktop and Ruffle Android), through
 * `ExternalInterface`; Turu Mas (AIR) gets its own in R4 (docs/HOST_SWF_PROPOSAL.md).
 */
public class Host {
    public function Host() {
    }

    /** Lets the bot call `fn` by `name`. */
    public function addCallback(name:String, fn:Function):void {
        throw new Error("Host.addCallback");
    }

    /**
     * Tells the bot of an event. `args` arrive as one array, as VibeSkua's `Externalizer.call`
     * sends them (`events.rs` unwraps it).
     */
    public function call(name:String, args:Array):void {
        throw new Error("Host.call");
    }

    /** Starts `loader` loading `url`, in the host SWF's domain (the game shares its classes). */
    public function load(loader:Loader, url:String):void {
        throw new Error("Host.load");
    }

    /** Puts `game` on screen in place of `main` (the host SWF's root) and any earlier game. */
    public function show(main:DisplayObject, game:DisplayObject):void {
        throw new Error("Host.show");
    }
}
}
